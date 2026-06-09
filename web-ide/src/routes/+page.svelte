<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import type { Component as ComponentType } from "svelte";
  import { Box, Component, Container, Database, File, FlaskConical, SquareFunction, User } from "@lucide/svelte";
  import { dev } from "$app/environment";
  import { base } from "$app/paths";
  import "../app.css";
  import { dependencyModules, docManifest, format as formatSource, ideEmitScene, ideLayoutScene, ideOutline, ideReferences, ideRename, ideSymbolScene, ideUniverse, initWasm, mountIde, renderDocSite, setIdeSource, type UniverseSnapshot } from "$lib/pds.js";
  import type { Module, Occurrence, References, RenameSelection } from "$lib/pds.js";
  import { fsSupported, scaffoldWorkspace, pickDirectory, emptySeed, openWorkspace, readWorkspace, readVendoredDeps, readDocPages, readFile, readFileAt, writeFile, fileHandleAt, writeSite, resolveDocAsset, fqnOf, createFile, createDir, deleteDir, movePath, deletePath, serializeManifest, isBinaryPath, MAX_OTHER_TEXT_BYTES } from "$lib/workspace.js";
  import { pins as pinStore } from "$lib/stores/pins.svelte.js";
  import { viewKey, getPins, serializeLayoutDoc } from "$lib/core/pins.js";
  import type { Workspace, WorkspaceFile, SiteFile } from "$lib/workspace.js";
  import type { Depth, SceneItem } from "$lib/sequence.js";
  import { reportError } from "$lib/errors.js";
  import { SAMPLES, sampleSeed } from "$lib/samples.js";
  import { getRecents, recordFolder, reopenFolder, forget } from "$lib/recents.js";
  import type { Recent } from "$lib/recents.js";
  import { encodeWorkspace, decodeWorkspace, bytesToBase64Url, base64UrlToBytes, MAX_HASH_BYTES } from "$lib/codec.js";
  import type { MountableWorkspace } from "$lib/codec.js";
  import { theme } from "$lib/theme.svelte.js";
  import { flowColor } from "$lib/flow-color.js";
  import { simpleName } from "$lib/graph-route.js";
  import * as nav from "$lib/core/navigation.js";
  import * as ops from "$lib/core/workspace-ops.js";
  import { keyOf, computeDirty, seedBaseline as advanceBaseline, classifyReload } from "$lib/core/dirty.js";
  import * as share from "$lib/core/share.js";
  import { pushState, replaceState } from "$app/navigation";

  import { router, parseHash, serializeRoute, type Route, type RouteBase } from "$lib/router.svelte.js";
  import * as docs from "$lib/core/docs.js";
  import * as model from "$lib/core/model.js";
  import { projectCanvas, canvasHint as canvasHintOf } from "$lib/core/canvas.js";
  import { notifications } from "$lib/stores/notifications.svelte.js";
  import { wasm } from "$lib/stores/wasm.svelte.js";
  import { sessionMount } from "$lib/stores/session.svelte.js";
  import { navigation } from "$lib/stores/navigation.svelte.js";
  import { wsStore } from "$lib/stores/workspace.svelte.js";
  import { selection } from "$lib/stores/selection.svelte.js";
  import { saveStore } from "$lib/stores/save.svelte.js";
  import { diagnostics } from "$lib/stores/diagnostics.svelte.js";
  import { ui } from "$lib/stores/ui.svelte.js";
  import { shareStore } from "$lib/stores/share.svelte.js";
  import { panelSizes, PANEL_MIN, PANEL_MAX, PROBLEMS_MIN, PROBLEMS_MAX } from "$lib/stores/panel-sizes.svelte.js";
  import Editor from "$lib/components/Editor.svelte";
  import FileTree from "$lib/components/FileTree.svelte";
  import TopBar from "$lib/components/shell/TopBar.svelte";
  import ActivityBar from "$lib/components/shell/ActivityBar.svelte";
  import RightRail from "$lib/components/shell/RightRail.svelte";
  import StructurePanel from "$lib/components/shell/StructurePanel.svelte";
  import BottomDock from "$lib/components/shell/BottomDock.svelte";
  import Splitter from "$lib/components/shell/Splitter.svelte";
  import StatusBar from "$lib/components/shell/StatusBar.svelte";
  import PerfMeter from "$lib/components/shell/PerfMeter.svelte";
  import CommandPalette from "$lib/components/shell/CommandPalette.svelte";
  import TabBar from "$lib/components/shell/TabBar.svelte";
  import DiagramPane from "$lib/components/DiagramPane.svelte";
  import ForceGraph from "$lib/components/ForceGraph.svelte";
  import ProblemsPane from "$lib/components/ProblemsPane.svelte";
  import Notifications from "$lib/components/Notifications.svelte";
  import ProjectPanel from "$lib/components/ProjectPanel.svelte";
  import NewProjectDialog from "$lib/components/NewProjectDialog.svelte";
  import Settings from "$lib/components/Settings.svelte";
  import { llm } from "$lib/llm.svelte.js";
  import PromptDialog from "$lib/components/PromptDialog.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import BuildNoticeDialog from "$lib/components/BuildNoticeDialog.svelte";
  import RenameDialog from "$lib/components/RenameDialog.svelte";

  // ── Page-local types ──────────────────────────────────────────────────────
  // Shared shapes live in the framework-agnostic core (`$lib/core/types`); the
  // page keeps only the view-specific `MountInput`. `WorkspaceModel` is aliased
  // to the page's historical `PageWorkspace` name.
  import type {
    StructureNode,
    Symbol,
    Problem,
    OpenFile,
    WorkspaceModel as PageWorkspace,
    LiveDocItem,
    LiveDocGroup,
    Loc,
    EditorApi,
    CanvasUsages,
    Dialog,
    NoteKind,
  } from "$lib/core/types.js";

  // The in-memory mount payload `mountWorkspace` consumes (sample / decoded).
  type MountInput = { workspace: PageWorkspace; landing?: string | null };

  // WASM readiness / version / init error — owned by the wasm store; the page
  // reads them through these derived aliases (keeping every call site unchanged).
  const ready = $derived(wasm.ready);
  const wasmError = $derived(wasm.error);
  let editorApi = $state<EditorApi | null>(null);
  // Timestamp of the last bare Shift keydown, for double-Shift "Search Everywhere".
  let lastShift = 0;

  // The selected item's view: its source ("code"), its interactive diagram
  // ("canvas"), or the workspace problem list ("problems"). The nav stays put;
  // only this content pane swaps.
  // Selection / view state is owned by the selection store; the view reads it
  // through these derived aliases and writes back via `selection.X = …`.
  const view = $derived(selection.view);
  const seqDepth = $derived(selection.seqDepth);
  const selected = $derived(selection.selected);
  const pendingGoto = $derived(selection.pendingGoto);

  // Navigation history is owned by the navigation store; the view keeps the impure
  // application (opening files, jumping the editor). `canBack`/`canForward` and
  // `recordLocation` are thin aliases so every call site is unchanged.
  const canBack = $derived(navigation.canBack);
  const canForward = $derived(navigation.canForward);
  const recordLocation = (loc: Loc) => navigation.record(loc);

  // Before a jump, record where the caret currently is so Back returns to the
  // starting point. Skips when the caret already sits at the history cursor.
  function recordOrigin() {
    const loc = editorApi?.location?.();
    if (!loc || !openFile?.fqn) return;
    navigation.recordIfMoved(nav.originLoc(openFile.fqn, loc.line, loc.col));
  }

  // Record the *current view's* scope as a Back origin, so a reveal/goto returns to
  // where it was launched from — the editor caret in code, the diagram scope in the
  // canvas, or the focused flow/node in the universe (not always the editor).
  function recordViewOrigin() {
    if (view === "space") recordSpaceScope(spaceTargetFqn);
    else if (view === "canvas") recordCanvasScope(selected?.fqn ?? null);
    else recordOrigin();
  }

  // Record a canvas scope (a drilled node, or `null` for the whole-model
  // overview) as a history entry, so Back returns to the previous diagram.
  function recordCanvasScope(fqn: string | null) {
    const hit = fqn ? nodeIndex.get(fqn) : null;
    recordLocation({
      view: "canvas",
      fqn: fqn ?? undefined,
      fileFqn: hit?.fileFqn ?? "",
      line: hit?.node.line ?? 0,
      col: hit?.node.col ?? 0,
      label: fqn ? nodeTitle(fqn) : "Overview",
    });
  }

  // Record a universe scope (a focused node, or an opened flow keyed by its entry
  // callable) as a history entry, so Back/Forward step through the 3D view alongside
  // code and canvas. A flow's entry callable is not a placed node — label it by leaf.
  function recordSpaceScope(fqn: string | null) {
    const hit = fqn ? nodeIndex.get(fqn) : null;
    recordLocation({
      view: "space",
      fqn: fqn ?? undefined,
      fileFqn: hit?.fileFqn ?? "",
      line: hit?.node.line ?? 0,
      col: hit?.node.col ?? 0,
      label: fqn ? (hit ? nodeTitle(fqn) : simpleName(fqn)) : "Universe",
    });
  }

  // Apply a location without recording it (back/forward, history-list click).
  // A canvas entry replays the diagram scope and stays on the canvas (no editor
  // jump); a code entry opens its file, re-scopes, and jumps the editor.
  // Sets state directly (never via selectNode/resetScope), so replay does not
  // re-enter the recording paths.
  function applyLocation(loc: Loc) {
    if (loc.view === "space") {
      applySpaceTarget(loc.fqn ?? null);
      if (loc.fqn) selectNode(loc.fqn, { goto: false, origin: false, record: false });
      return;
    }
    if (loc.view === "canvas") {
      selection.selected = loc.fqn
        ? { fqn: loc.fqn, line: loc.line, col: loc.col, fileFqn: loc.fileFqn }
        : null;
      const file = loc.fqn ? workspace?.files.find((f) => f.fqn === loc.fileFqn) : null;
      if (file && openFile?.fqn !== file.fqn) wsStore.openFile = file;
      selection.view = "canvas";
      return;
    }
    const file = workspace?.files.find((f) => f.fqn === loc.fileFqn);
    if (!file) return;
    if (openFile?.fqn !== file.fqn) wsStore.openFile = file;
    if (loc.fqn) selection.selected = { fqn: loc.fqn, line: loc.line, col: loc.col, fileFqn: loc.fileFqn };
    selection.view = "code";
    selection.pendingGoto = { line: loc.line, col: loc.col, fileFqn: loc.fileFqn };
  }

  function goBack() {
    const loc = navigation.back();
    if (loc) applyLocation(loc);
  }
  function goForward() {
    const loc = navigation.forward();
    if (loc) applyLocation(loc);
  }

  // Open a find-usages occurrence: jump to it and record it in history.
  function openUsage(occ: Occurrence) {
    recordOrigin();
    applyLocation({ fileFqn: occ.fqn, line: occ.line, col: occ.col });
    recordLocation({ fileFqn: occ.fqn, line: occ.line, col: occ.col, label: occ.text || `${occ.fqn}:${occ.line}` });
  }

  // Workspace state is owned by the workspace store (wsStore). The view reads it
  // through these derived aliases (every read site unchanged) and writes back via
  // `wsStore.X = …`. Defaults to the bundled sample (in-memory, handles null);
  // "Open folder" swaps in a real on-disk workspace whose files persist on edit.
  const workspace = $derived(wsStore.workspace);
  const openFile = $derived(wsStore.openFile);
  const moduleSources = $derived(wsStore.moduleSources);
  const docGroups = $derived(wsStore.docGroups);
  const docSources = $derived(wsStore.docSources);
  const docMeta = $derived(wsStore.docMeta);
  const otherSources = $derived(wsStore.otherSources);
  const manifestSource = $derived(wsStore.manifestSource);
  const manifestError = $derived(wsStore.manifestError);
  // Whether the live manifest declares a `[dependencies]` table — drives the
  // read-only "resolved by pds install" note.
  const manifestHasDeps = $derived(docs.manifestHasDeps(manifestSource));

  // The persisted baseline: the text last read from / written to disk, keyed the
  // same way as the live buffers (FQN for modules, path for docs). A file is
  // "dirty" when its live buffer differs from this baseline. Bundled samples have
  // no handle and never enter this map — they're session-only, not dirty.
  // Persisted baseline + save cue are owned by the save store; read via aliases,
  // written via `saveStore.X = …`. The debounce/FS methods stay in the view.
  const persisted = $derived(saveStore.persisted);
  const saveState = $derived(saveStore.saveState);
  let saveStateTimer: ReturnType<typeof setTimeout> | undefined;

  // Whether the workspace can persist to disk at all (a real opened folder, not a
  // bundled in-memory sample).
  const canPersist = $derived(!!workspace?.root);

  // Whether the open doc is folder-backed (a real on-disk page that can resolve
  // relative assets/links). A sample doc and the manifest are not.
  const isFolderBacked = $derived(!!(workspace?.root && openFile?.isDoc && openFile?.handle));

  // Resolve a relative doc link to a sibling page in the IDE: normalise it
  // against the open doc's dir and match a loaded doc page by path.
  function resolveDocLink(rel: string) {
    if (!openFile?.isDoc || !openFile.path) return;
    const item = docs.findDocByPath(docGroups, docs.resolveDocPath(openFile.path, rel));
    if (item) openDoc(item);
  }

  // Markdown live-preview options for the open doc. Folder docs resolve relative
  // images from disk and relative `.md` links to sibling pages; samples/manifest
  // get only the (inert) link resolver — relative images show a placeholder.
  const previewOpts = $derived(
    openFile?.isDoc
      ? {
          resolveLink: resolveDocLink,
          resolveAsset: isFolderBacked
            ? (rel: string) => resolveDocAsset(workspace?.root, openFile?.path ?? "", rel)
            : null,
        }
      : {},
  );

  // The set of dirty file keys: live buffer differs from the on-disk baseline.
  // Only files with a recorded baseline (i.e. backed by a handle) can be dirty;
  // sample buffers have no baseline and are reported as session-only instead.
  const manifestKey = $derived(workspace?.manifest?.path ?? null);
  const dirty = $derived(computeDirty(persisted, { manifestKey, manifestSource, moduleSources, docSources, otherSources }));
  const dirtyCount = $derived(dirty.size);

  // The Markdown reading width — owned (and persisted) by the ui store.
  const docWidth = $derived(ui.docWidth);
  const setDocWidth = (w: string) => ui.setDocWidth(w);

  // The Markdown syntax cheat-sheet shown from the doc toolbar's "?" button —
  // every flavour the live preview and `pds doc` render.
  const mdHelpOpen = $derived(ui.mdHelpOpen);
  const MD_SYNTAX = [
    { name: "Heading", syntax: "# H1  ·  ## H2  …  ###### H6" },
    { name: "Bold", syntax: "**bold**" },
    { name: "Italic", syntax: "*italic*" },
    { name: "Strikethrough", syntax: "~~struck~~" },
    { name: "Inline code", syntax: "`code`" },
    { name: "Link", syntax: "[text](https://…)" },
    { name: "Quote", syntax: "> quoted text" },
    { name: "Callout", syntax: "> [!NOTE]\n> note text\n(NOTE · TIP · IMPORTANT · WARNING · CAUTION)" },
    { name: "Bullet list", syntax: "- item\n  - nested" },
    { name: "Numbered list", syntax: "1. item" },
    { name: "Task list", syntax: "- [ ] todo\n- [x] done" },
    { name: "Code block", syntax: "```ts\nconst a = 1\n```" },
    { name: "Table", syntax: "| a | b |\n|---|---|\n| 1 | 2 |" },
    { name: "Divider", syntax: "---" },
  ];

  // The project panel (recent projects + examples + open folder): opens on start
  // and from the toolbar's project button. Never autoloads an architecture.
  const projectOpen = $derived(ui.projectOpen);
  // The New-project dialog (template picker), opened from the launcher.
  const newProjectOpen = $derived(ui.newProjectOpen);
  const structureOpen = $derived(ui.structureOpen);
  // The left Explorer (file tree) and the bottom Problems dock — tool-window
  // islands toggled from the nav rails.
  const explorerOpen = $derived(ui.explorerOpen);
  const problemsOpen = $derived(ui.problemsOpen);
  // Whether the keyboard-shortcuts settings modal is open (toolbar gear or the
  // bound shortcut). Shell-owned so it's reachable with or without a file open.
  const settingsOpen = $derived(ui.settingsOpen);
  // The tab Settings opens on. "keyboard" for the general openers; the AI status
  // chip seeds "ai" so its "click to configure" lands on the page it advertised.
  let settingsTab = $state<"keyboard" | "ai">("keyboard");
  const recents = $derived(ui.recents);
  // Only persisted projects (folders) are recents; in-memory samples re-open
  // from the catalogue, so they're never recorded — and legacy sample entries
  // are filtered out of the list.
  const refreshRecents = () => {
    ui.recents = getRecents().filter((r) => r.kind !== "sample");
  };

  const source = $derived(
    openFile?.isManifest
      ? manifestSource
      : openFile?.isDoc
        ? (docSources[openFile.path ?? ""] ?? "")
        : openFile?.isOther
          ? (otherSources[openFile.path ?? ""] ?? "")
          : openFile
            ? (moduleSources[openFile.fqn ?? ""] ?? "")
            : "",
  );

  // Every module as {fqn, source}, and the workspace diagnostics — both owned by
  // their stores; the view reads them through these aliases.
  const allModules = $derived(wsStore.allModules);
  const problems = $derived(diagnostics.problems);
  const errorCount = $derived(diagnostics.errorCount);
  // The open module's diagnostics (same workspace source as the problems pane),
  // for inline editor highlighting — so workspace-only diagnostics (FQN
  // qualification, cross-module visibility, §8) highlight at their position.
  const editorDiagnostics = $derived(
    diagnostics.results?.find((m) => m.fqn === openFile?.fqn)?.diagnostics ?? [],
  );

  // Every workspace file — modules, authored docs, and the manifest — as a single
  // list for one unified file tree (path-based, no per-type sections). `relPath`
  // is workspace-root-relative; `key` is the dirty/active key (FQN or path).
  type FileEntry = { key: string; kind: "module" | "doc" | "manifest" | "other"; relPath: string; label: string; fqn?: string; binary?: boolean };
  const fileEntries = $derived.by<FileEntry[]>(() => {
    if (!workspace) return [];
    const out: FileEntry[] = [];
    for (const f of workspace.files) {
      if (!f.fqn) continue;
      out.push({ key: f.fqn, kind: "module", relPath: rel(f.path ?? `${f.fqn}.pds`), label: f.fqn, fqn: f.fqn });
    }
    for (const g of docGroups) for (const it of g.items) out.push({ key: it.path, kind: "doc", relPath: rel(it.path), label: it.title });
    if (workspace.manifest) out.push({ key: workspace.manifest.path, kind: "manifest", relPath: rel(workspace.manifest.path), label: "pds.toml" });
    // Companion files: everything walked that isn't already shown as a module,
    // sidebar doc, or the manifest. De-dup by base-relative path so a
    // sidebar-listed `.md` stays a `doc` and isn't duplicated here.
    const shown = new Set(out.map((e) => e.relPath));
    for (const o of workspace.others ?? []) {
      const relPath = rel(o.path ?? "");
      if (!relPath || shown.has(relPath)) continue;
      shown.add(relPath);
      out.push({ key: relPath, kind: "other", relPath, label: relPath.split("/").at(-1) ?? relPath, binary: isBinaryPath(relPath) });
    }
    return out;
  });
  // The keys (FQN/path) of files that currently have an error, for the tree dot.
  const errorKeys = $derived(new Set(problems.filter((p) => p.severity === "error" && p.file).map((p) => p.file!)));
  const openKey = $derived(openFile ? (keyOf(openFile) ?? "") : "");

  const nodes = $derived.by<StructureNode[]>(() => {
    // `ideOutline()` reads the session's held state, which is not reactive — track
    // `allModules` (per-edit changes, the session already current via `setIdeSource`)
    // and `sessionMount.seq` (a structural re-mount) so the outline follows it.
    void allModules;
    void sessionMount.seq;
    if (!ready || !workspace) return [];
    try {
      return ideOutline() as unknown as StructureNode[];
    } catch {
      return [];
    }
  });
  // The whole structural model index — node lookup, per-file grouping, flows,
  // type map, collapse info — built once from the workspace outline (see
  // core/model). The view reads its fields via the aliases below; the longest-
  // prefix file grouping keeps cross-module go-to-definition resolving.
  const index = $derived(model.buildModelIndex(nodes, allModules.map((m) => m.fqn)));
  const structureByFile = $derived(index.structureByFile);
  const nodeIndex = $derived(index.nodeIndex);
  const symbols = $derived(index.symbols);
  // One icon per C4 level, for the breadcrumb (mirrors the structure tree).
  const KIND_ICON: Record<string, ComponentType> = {
    person: User,
    system: Box,
    container: Container,
    component: Component,
    data: Database,
    callable: SquareFunction,
    feature: FlaskConical,
  };
  const flowsByNode = $derived(index.flowsByNode);
  const nodeInfo = $derived(index.nodeInfo);
  const typeFqnByName = $derived(index.typeFqnByName);

  // The CANVAS diagram: the selected node's fitting view, or the whole-model
  // context overview when nothing is selected. The compiler picks the view; a
  // sequence scene is then collapsed to the chosen depth in the IDE.
  // The C4 layout tweaks (the canvas "Layout" control) — one config applied to
  // every diagram, persisted in the ui store.
  // The current diagram's pin key: the selected symbol's view, or the whole-model
  // overview. Derived from `selected` (not the scene) to avoid a layout cycle —
  // each diagram has a unique target FQN, so this keys the `.layout` document.
  const currentViewKey = $derived(viewKey(selected ? "view" : "context", selected?.fqn ?? null));
  // This view's manual placements — the single source every pin consumer reads
  // (reactive on the pin store, so a drag re-derives the canvas → re-layout with the
  // node fixed). The engine takes the list as `gridPins`; the canvas marks cards from
  // {@link pinnedFqns}.
  const currentPins = $derived(getPins(pinStore.doc, currentViewKey));
  // The global layout tweaks plus this view's pins.
  const canvasTweaks = $derived({ ...ui.layoutTweaks, gridPins: currentPins });
  const canvas = $derived(
    ready && workspace
      ? projectCanvas({
          selected,
          seqDepth,
          index,
          wasm: { symbolScene: ideSymbolScene, emitScene: ideEmitScene, layoutScene: ideLayoutScene },
          tweaks: canvasTweaks,
          onError: reportError,
        })
      : { scene: null, error: "" },
  );

  // Drag-to-pin: pin a node to a grid cell, then persist the `.layout` file. The
  // re-layout is automatic (canvasTweaks reads the pin store).
  function pinNode(fqn: string, row: number, col: number): void {
    pinStore.pin(currentViewKey, { fqn, row, col });
    void persistPins();
  }

  // The FQNs pinned in the current view — marks their cards and gates the Reset.
  const pinnedFqns = $derived(new Set(currentPins.map((p) => p.fqn)));

  // Clear one node's placement (the inline ✕ on a pinned card), then persist.
  function unpinNode(fqn: string): void {
    pinStore.unpin(currentViewKey, fqn);
    void persistPins();
  }

  // Reset this diagram's manual placements back to the auto-layout, then persist.
  function resetCurrentView(): void {
    pinStore.clear(currentViewKey);
    void persistPins();
  }

  // The 3D relationship graph view (activity-bar "3D graph"). Rebuilt from the held
  // workspace whenever that view is entered, the workspace switches, or the model is
  // edited — `allModules` is the per-edit / per-workspace signal. `spaceRev` bumps on
  // each rebuild so the (otherwise mount-once) ForceGraph remounts on fresh data.
  let spaceSnapshot = $state<UniverseSnapshot | null>(null);
  let spaceRev = $state(0);
  // The node the 3D view should fly to (set by "Show in 3D graph"); null = no focus.
  let spaceFocus = $state<string | null>(null);
  // A flow's participant chain to light end-to-end in the 3D view; null = no flow.
  let spacePath = $state<string[] | null>(null);
  // The flow's ordered call hops (caller→callee node ids + the call label), so the 3D
  // traffic follows the sequence and can name the current step; null = no flow.
  let spaceFlow = $state<{ from: string; to: string; label: string }[] | null>(null);
  // The selected flow's colour (hash of its fqn → palette), so each flow reads distinct.
  let spaceFlowColor = $state<string | null>(null);
  // The selected flow's name (its entry point's leaf), shown in the 3D timeline header.
  let spaceFlowName = $state<string | null>(null);
  // The fqn the 3D view is currently targeting (a flow's entry callable or a node),
  // so a reveal/goto launched from the universe can record it as the Back origin.
  let spaceTargetFqn = $state<string | null>(null);

  $effect(() => {
    void allModules; // track edits + workspace switches
    if (view !== "space" || !ready || !workspace) return;
    try {
      spaceSnapshot = ideUniverse();
      // untracked: reading spaceRev to increment it would make this effect depend on
      // its own write (effect_update_depth_exceeded).
      spaceRev = untrack(() => spaceRev) + 1;
    } catch (e) {
      spaceSnapshot = null;
      notify("error", "Could not build the 3D graph", String((e as Error)?.message ?? e));
    }
  });
  // Remount the graph on a rebuild *or* a theme change (it reads brand colours off the
  // active theme at mount, so a live theme switch needs a fresh build).
  const spaceKey = $derived(`${spaceRev}|${theme.resolved}`);

  // Reveal a node in the 3D graph: switch to that view. A flow (an entry-point with a
  // sequence) lights its whole chain and streams traffic along it in call order;
  // anything else flies to the node. Also records the shared selection so it persists
  // when you switch to the canvas or code views.
  function openUniverse(fqn: string | null): void {
    applySpaceTarget(fqn);
    if (fqn) selectNode(fqn, { goto: false, origin: false, record: false });
    recordSpaceScope(fqn);
  }
  // Set the 3D view's target — light a flow's chain (an entry-point fqn) or fly to a
  // node — and switch to the view. State only: no history, no shared selection (the
  // callers own those, so back/forward replay can reuse this without re-recording).
  function applySpaceTarget(fqn: string | null): void {
    const flow = fqn ? flowOf(fqn) : null;
    if (flow) { spacePath = flow.participants; spaceFlow = flow.hops; spaceFlowColor = flowColor(fqn!); spaceFlowName = simpleName(fqn!); spaceFocus = null; }
    else { spaceFocus = fqn; spacePath = null; spaceFlow = null; spaceFlowColor = null; spaceFlowName = null; }
    spaceTargetFqn = fqn;
    selection.view = "space";
  }
  // Clear the 3D graph's selection (highlight + flow) back to the resting view, and
  // the global node selection with it — deselecting in the universe is a deselect
  // everywhere (the structure panel, breadcrumb, canvas scope), not just here.
  function resetSpace(): void {
    spaceFocus = null;
    spacePath = null;
    spaceFlow = null;
    spaceFlowColor = null;
    spaceFlowName = null;
    spaceTargetFqn = null;
    selection.selected = null;
  }
  // The flow `fqn`'s sequence — its participant nodes and its ordered call hops — mapped
  // to 3D-graph node ids, or null if it isn't a flow. The sequence and the universe use
  // different FQN forms, so we bridge them by node simple-name (last `::` segment).
  function flowOf(fqn: string): { participants: string[]; hops: { from: string; to: string; label: string }[] } | null {
    // The view's snapshot is built lazily on entering the view, so when this is invoked
    // from the canvas it may not exist yet — resolve against a fresh build then.
    let snap = spaceSnapshot;
    if (!snap) {
      try { snap = ideUniverse(); } catch { return null; }
    }
    let scene: { participants?: { fqn: string }[]; items?: SceneItem[] };
    try {
      scene = ideSymbolScene(fqn) as typeof scene;
    } catch {
      return null; // not a projectable flow
    }
    if (!Array.isArray(scene.participants) || scene.participants.length <= 1) return null;
    const byName = new Map<string, string[]>();
    for (const n of snap.nodes) {
      const s = simpleName(n.id);
      (byName.get(s) ?? byName.set(s, []).get(s)!).push(n.id);
    }
    const mapId = (f: string) => {
      const m = byName.get(simpleName(f));
      return m && m.length === 1 ? m[0] : null; // unambiguous only
    };
    const participants = [...new Set(scene.participants.map((p) => mapId(p.fqn)).filter((x): x is string => !!x))];
    if (participants.length <= 1) return null;
    // Flatten the ordered call messages (recursing into loop/alt frames) into hops.
    const hops: { from: string; to: string; label: string }[] = [];
    const walk = (items: SceneItem[]) => {
      for (const it of items) {
        if (it.Message && it.Message.kind !== "return") {
          const a = mapId(it.Message.from), b = mapId(it.Message.to);
          if (a && b && a !== b) hops.push({ from: a, to: b, label: it.Message.label ?? "" });
        } else if (it.Frame) walk(it.Frame.body);
      }
    };
    walk(scene.items ?? []);
    return { participants, hops };
  }

  // Every flow in the model, for the 3D view's resting filaments: each entry point's
  // whole call chain, coloured by its start. Entry points are the callables that begin
  // a flow — a person's actions, plus anything explicitly triggered (scheduled / http /
  // event). Each becomes one or more stacked filaments via its hops.
  const spaceFlows = $derived.by(() => {
    if (selection.view !== "space" || !spaceSnapshot) return [];
    const personFqns = new Set(nodes.filter((n) => n.kind === "person").map((n) => n.fqn));
    const out: { fqn: string; color: string; hops: { from: string; to: string; label: string }[] }[] = [];
    for (const n of nodes) {
      if (n.kind !== "callable") continue;
      const isEntry = n.triggered || (n.parent != null && personFqns.has(n.parent));
      if (!isEntry) continue;
      const flow = flowOf(n.fqn);
      if (flow && flow.hops.length) out.push({ fqn: n.fqn, color: flowColor(n.fqn), hops: flow.hops });
    }
    return out;
  });

  // Unlocking only arms drag-to-pin; it pins nothing. A dragged card pins just itself
  // (the engine fixes pinned nodes and re-flows the rest), so only nodes you place
  // stay put. Locking keeps those manual placements.
  function toggleUnlock(next: boolean): void {
    pinStore.unlocked = next;
  }

  // Load the workspace's manual placements from `pds.layout`. Folder-backed only;
  // in-memory samples/shares keep pins session-only.
  async function loadPins(ws: PageWorkspace): Promise<void> {
    if (!ws.root) {
      pinStore.reset();
      return;
    }
    const path = ws.base ? `${ws.base}/pds.layout` : "pds.layout";
    const text = (await readFileAt(ws.root, path)) ?? "";
    pinStore.load(text, null); // the handle is created lazily on first write
  }

  // Write `pds.layout` to disk (creating it on first pin). No-op for in-memory
  // workspaces, which have no root to write to.
  async function persistPins(): Promise<void> {
    const ws = workspace;
    if (!ws?.root) return;
    const path = ws.base ? `${ws.base}/pds.layout` : "pds.layout";
    try {
      const handle = pinStore.handle ?? (await fileHandleAt(ws.root, path));
      pinStore.handle = handle;
      await writeFile(handle, serializeLayoutDoc(pinStore.doc));
    } catch (e) {
      notify("error", "Could not save layout", String((e as Error)?.message ?? e));
    }
  }
  const canvasHint = $derived(canvasHintOf(selected));

  // Canvas interaction mirrors the C4 graph: right-clicking a symbol opens its
  // actions menu (go-to-definition / find-usages). Find-usages lists references
  // in a popover anchored at the pointer.
  const canvasUsages = $derived(ui.canvasUsages); // { name, items, x, y }

  // The byte offset of a node's declaration in its module source.
  const nodeByteOffset = (fileFqn: string, line: number, col: number) =>
    model.nodeByteOffset(moduleSources[fileFqn] ?? "", line, col);

  const resolveNodeFqn = (fqn: string) => model.resolveNodeFqn(index, fqn);

  function showCanvasUsages(fqn: string, e: MouseEvent) {
    const target = resolveNodeFqn(fqn.startsWith("event:") ? fqn.slice(6) : fqn);
    const hit = target ? nodeIndex.get(target) : null;
    if (!hit) {
      notify("info", "No usages", "Not a resolvable symbol.");
      return;
    }
    let refs = null;
    try {
      refs = ideReferences(hit.fileFqn, nodeByteOffset(hit.fileFqn, hit.node.line, hit.node.col));
    } catch {
      refs = null;
    }
    if (!refs?.occurrences?.length) {
      notify("info", "No usages", `\`${hit.node.name}\` is not referenced.`);
      return;
    }
    ui.canvasUsages = { name: refs.fqn.split("::").at(-1) ?? "", items: refs.occurrences, x: e.clientX, y: e.clientY };
  }
  function pickCanvasUsage(occ: Occurrence) {
    ui.canvasUsages = null;
    openUsage(occ);
  }

  const nodeTitle = (fqn: string) => model.nodeTitle(index, fqn);

  const ownerNodeOf = (fqn: string) => model.ownerNodeOf(index, fqn);

  // Select a structure node: open its declaring file and remember it as the
  // current scope. `goto` (a nav click) also shows the code and jumps the editor
  // to the declaration; a canvas drill leaves the view alone. A member/field fqn
  // (`Owner::name`) isn't itself a node — fall back to its owner so GOTO on a
  // field opens its declaring type instead of no-opping (PDS-GOTO-002).
  function selectNode(fqn: string, { goto = false, origin = true, record = true }: { goto?: boolean; origin?: boolean; record?: boolean } = {}) {
    // Resolve the fqn to a structural node. A member/field fqn (`Owner::name`)
    // isn't itself a node — fall back to its owner so go-to-definition on a field
    // opens its declaring type instead of no-opping (PDS-GOTO-002).
    let targetFqn = fqn;
    let hit = nodeIndex.get(targetFqn);
    if (!hit) {
      const owner = ownerNodeOf(fqn);
      if (owner) {
        reportError("GOTO_MEMBER_FALLBACK", `${fqn} → ${owner}`);
        targetFqn = owner;
        hit = nodeIndex.get(owner);
      }
    }
    if (!hit) {
      reportError("GOTO_UNRESOLVED", fqn, { nodeCount: nodeIndex.size });
      return;
    }
    const file = workspace?.files.find((f) => f.fqn === hit!.fileFqn);
    if (!file?.fqn) {
      reportError("GOTO_FILE_MISSING", `${targetFqn} declared in ${hit.fileFqn}`);
      return;
    }
    const fileFqn = file.fqn;
    // Record the launching view's scope before the file/scope changes, so Back
    // returns there (the editor caret in code, the universe's flow/node in space).
    // `origin: false` suppresses it when the caller already recorded the origin.
    if (goto && view !== "canvas" && origin) recordViewOrigin();
    if (openFile?.fqn !== fileFqn) wsStore.openFile = file;
    selection.selected = { fqn: targetFqn, line: hit.node.line, col: hit.node.col, fileFqn };
    // A nav click jumps the editor to the declaration — but only when the canvas
    // isn't showing; on the canvas the new scope is the navigation, so stay put.
    if (goto && view !== "canvas") {
      selection.view = "code";
      selection.pendingGoto = { line: hit.node.line, col: hit.node.col, fileFqn };
      if (record) recordLocation({ fileFqn, line: hit.node.line, col: hit.node.col, fqn: targetFqn, label: nodeTitle(targetFqn) });
    } else if (record) {
      // A canvas drill or a universe selection: record the new scope in the active
      // view so Back returns to the previous diagram / flow.
      if (view === "space") recordSpaceScope(targetFqn);
      else recordCanvasScope(targetFqn);
    }
  }

  // Clicking a node in the canvas drills the selection into it (staying on the
  // canvas); synthetic initiators (client, scheduler, …) aren't declared nodes.
  const pickNode = (fqn: string) => selectNode(fqn);
  // "Go to definition" from the canvas context menu: leave the canvas for the
  // editor and jump to the node's declaration. Record the canvas scope we're
  // leaving as the origin (so Back returns to that diagram, not the editor's
  // last caret), then switch the view — selectNode's goto path stays put while
  // the canvas shows, so the view must flip first — and suppress its own origin.
  function openNodeInEditor(fqn: string) {
    if (view === "canvas") recordCanvasScope(selected?.fqn ?? null);
    selection.view = "code";
    selectNode(fqn, { goto: true, origin: false });
  }
  // Reset the canvas scope to the whole-model context, recording it so Back
  // returns to the previous diagram.
  const resetScope = () => {
    selection.selected = null;
    recordCanvasScope(null);
  };
  // Close the expanded boundary: pop up to the structural parent (the `for`
  // owner — system → container → component), or the whole-model context at the
  // top level. FQNs are flat within a module, so this follows `parent`, not `::`.
  function navigateUp() {
    if (!selected) return;
    const parent = nodeIndex.get(selected.fqn)?.node.parent;
    if (parent && nodeIndex.has(parent)) selectNode(parent);
    else resetScope();
  }

  // The structural ancestor chain (root system → … → the node) for the breadcrumb.
  const ancestry = (fqn: string) => model.ancestry(index, fqn);

  // Reveal a symbol's diagram on the canvas (editor hover popover, or the structure
  // panel's right-click). Record where you launched it from — code, canvas, or the
  // universe — so Back returns there; then switch to the canvas so selectNode records
  // the revealed scope as a canvas entry.
  function revealSymbol(fqn: string) {
    if (!nodeIndex.has(fqn)) return;
    recordViewOrigin();
    selection.view = "canvas";
    selectNode(fqn, { origin: false });
  }

  // Show a symbol in the 3D universe (the structure panel's right-click). Record the
  // launch view so Back returns there; openUniverse flies to the node (or lights the
  // chain for a flow), switches to the space view, and records the new space scope.
  function showInUniverse(fqn: string) {
    if (!nodeIndex.has(fqn)) return;
    recordViewOrigin();
    openUniverse(fqn);
  }

  // Apply a queued editor jump once the code view is mounted on the right file.
  // Deferred a tick so the editor has synced the (possibly just-switched) file's
  // text before we index into it — otherwise a cross-file jump lands in the old doc.
  $effect(() => {
    if (view === "code" && editorApi && pendingGoto && openFile?.fqn === pendingGoto.fileFqn) {
      const target = pendingGoto;
      selection.pendingGoto = null;
      tick().then(() => editorApi?.goto(target.line, target.col));
    }
  });

  async function boot() {
    theme.init(); // sync runtime theme state with the inline-head choice; watch OS
    if (!(await wasm.init())) return;
    refreshRecents();
    wasm.ready = true;
    // Disk-only: without the File System Access API there's nowhere to read or
    // write a project, so the render shows an unsupported notice and the launcher
    // never opens.
    if (!fsSupported) return;
    // The URL hash restores its workspace + location and skips the project panel;
    // otherwise open the panel on start (never autoload a model). Route history
    // writes through SvelteKit's shallow-routing API — raw history.replaceState
    // conflicts with its client router (re-runs effects → update-depth loop).
    router.configureWriter((url, replace) => (replace ? replaceState(url, {}) : pushState(url, {})));
    router.start();
    const restored = await restoreSession();
    ui.projectOpen = !restored;
    // Enable the live URL sync only after the restore (and its queued caret jump)
    // has settled, so the first sync doesn't overwrite the restored location.
    await tick();
    urlReady = true;
  }
  onMount(boot);

  // Keep the IDE-core session's module set in step with the workspace structure:
  // re-mount on ready, open, file add/remove/rename, and deps-resolved — but NOT on
  // a plain edit (those flow through `setIdeSource`). The structural signals (the
  // file-fqn list and the externals) are tracked; `allModules` is read `untrack`ed
  // so a source change does not retrigger a full re-mount.
  $effect(() => {
    if (!wasm.ready) return;
    const fileKey = (wsStore.workspace?.files ?? []).map((f) => f.fqn).join("\n");
    const externals = wsStore.externalModules;
    void fileKey;
    mountIde(
      untrack(() => wsStore.allModules),
      externals,
    );
    // Signal that the session now holds the new modules, so held-state deriveds
    // (the outline) recompute against it. `untrack` keeps this write off the
    // effect's own dependency set.
    untrack(() => sessionMount.bump());
  });

  // Register the PWA service worker in production only (dev skips it to keep
  // Vite's HMR/module loading uncontended — see svelte.config.js).
  onMount(() => {
    if (!dev && "serviceWorker" in navigator) {
      navigator.serviceWorker.register(`${base}/service-worker.js`, { type: "module" });
    }
  });

  // `FileSystemObserver` (Chromium 129+) is not in the TS DOM lib yet; the change
  // records are ignored — a change just triggers the same reconcile.
  type FileSystemObserverCtor = new (callback: () => void) => {
    observe(handle: FileSystemHandle, options?: { recursive?: boolean }): Promise<void>;
    disconnect(): void;
  };

  // Watch for changes made outside the IDE (an agent scaffolding/editing files).
  // `FileSystemObserver` (Chromium) fires on the underlying change — event-driven,
  // no disk churn. A visible-only poll is the fallback where it isn't available
  // (Firefox/Safari, older Chromium) and a backstop on focus. Both run the same
  // `reloadExternalChanges`, which re-reads content and reconciles files
  // created/deleted on disk; it no-ops without an open folder.
  let watching = $state(false); // true while a native FileSystemObserver is active

  onMount(() => {
    const timer = setInterval(() => {
      if (!watching && document.visibilityState === "visible") reloadExternalChanges();
    }, 2500);
    return () => clearInterval(timer);
  });

  // The native directory observer, (re)attached whenever the open workspace's root
  // changes. It re-walks only when the OS reports a change (debounced), so there's
  // no constant polling on capable browsers.
  $effect(() => {
    const root = workspace?.root;
    const Observer = (globalThis as unknown as { FileSystemObserver?: FileSystemObserverCtor })
      .FileSystemObserver;
    if (!root || !Observer) return;
    let observer: { disconnect(): void } | undefined;
    let debounce: ReturnType<typeof setTimeout> | undefined;
    const onchange = () => {
      clearTimeout(debounce);
      debounce = setTimeout(() => void reloadExternalChanges(), 150);
    };
    void (async () => {
      try {
        const obs = new Observer(onchange);
        await obs.observe(root, { recursive: true });
        observer = obs;
        watching = true;
      } catch {
        watching = false; // observe() can reject (unsupported recursive, permission)
      }
    })();
    return () => {
      watching = false;
      clearTimeout(debounce);
      observer?.disconnect();
    };
  });

  // Swap in a freshly-loaded workspace, resetting navigation to `landing`.
  async function mountWorkspace(ws: PageWorkspace, landing?: string | null) {
    wsStore.workspace = ws;
    void resolveDependencyModules(ws);
    void loadPins(ws);
    // An explicit landing FQN (meta.json) resolves to its module immediately;
    // otherwise tentatively open the first module and revisit once docs load.
    const explicit = landing ? ws.files.find((f) => f.fqn === landing) : null;
    wsStore.openFile = explicit ?? ws.files[0] ?? null;
    wsStore.openTabs = wsStore.openFile ? [wsStore.openFile] : [];
    selection.selected = null;
    selection.pendingGoto = null;
    selection.view = "code";
    navigation.reset();
    ui.projectOpen = false;
    wsStore.docGroups = [];
    wsStore.docSources = {};
    wsStore.docMeta = {};
    // Reset the dirty/save state for the new workspace; module baselines are
    // seeded by the opener (folder/recent) before mount, doc baselines on load.
    saveStore.saveState = "idle";
    clearTimeout(saveStateTimer);
    // Seed the editable manifest buffer; folder manifests also get an on-disk
    // baseline so the manifest row only shows dirty after a real edit.
    wsStore.manifestSource = ws.manifestToml ?? "";
    wsStore.manifestError = null;
    if (ws.root && ws.manifest) seedBaseline([{ key: ws.manifest.path, text: ws.manifestToml ?? "" }]);

    // Docs load async; await them so the initial open can prefer the docs
    // landing page. `loadWorkspaceDocs` returns null for a stale (superseded)
    // load — in which case a newer mount owns the open file, so leave it alone.
    const groups = await loadWorkspaceDocs(ws);
    if (groups === null) return; // a faster switch superseded this mount
    // With no explicit landing, prefer the first authored doc page (manifest
    // sidebar order); fall back to the first module already opened above. Don't
    // override a doc/manifest the user navigated to while docs loaded.
    if (!explicit && !openFile?.isDoc && !openFile?.isManifest && !openFile?.isOther) {
      const firstDoc = groups.flatMap((g) => g.items).find((it) => it?.path);
      if (firstDoc) openDoc(firstDoc);
    }
  }

  // Resolves the workspace's git dependencies (LANG.md §8.3) into externals for
  // the language services — read from `pds_modules/` + `pds.lock` once per mount
  // (dependencies change via the `pds` CLI, not in-editor). In-memory workspaces
  // (samples/shares) have no `root`, so they carry no dependencies. Guarded
  // against a faster mount superseding this async read.
  async function resolveDependencyModules(ws: PageWorkspace) {
    const seq = (depLoadSeq += 1);
    wsStore.dependencyModules = [];
    console.info("[pds-deps] start", { hasRoot: !!ws.root, base: ws.base ?? "" });
    if (!ws.root) {
      console.info("[pds-deps] no workspace root (in-memory sample) — no dependencies");
      return;
    }
    try {
      // The wasm resolver must be initialised before the first `dependency_modules`
      // call; mount can fire before init resolves, which would silently drop
      // externals for the whole session.
      await initWasm();
      const { lockToml, vendored } = await readVendoredDeps(ws.root, ws.base ?? "");
      console.info("[pds-deps] readVendoredDeps", {
        lockBytes: lockToml.length,
        vendoredCount: vendored.length,
        vendoredSample: vendored.slice(0, 3).map((v) => ({ slug: v.slug, fqn: v.fqn })),
      });
      if (seq !== depLoadSeq) {
        console.info("[pds-deps] workspace superseded — abort");
        return;
      }
      if (vendored.length === 0) {
        console.info("[pds-deps] no vendored files found under pds_modules — externals empty");
        return;
      }
      const resolved = dependencyModules(lockToml, vendored, []);
      console.info("[pds-deps] dependencyModules resolved", {
        count: resolved.length,
        fqns: resolved.map((m) => m.fqn),
      });
      wsStore.dependencyModules = resolved;
    } catch (e) {
      console.error("[pds-deps] FAILED", e);
      reportError("WORKSPACE_IO_FAILED", e instanceof Error ? e.message : String(e));
    }
  }

  // Parses the workspace `[doc]` manifest and loads its `[[doc.sidebar]]` pages
  // (from disk for a folder, from the bundled map for a sample) into the doc
  // state the FileTree and the site build read.
  let docLoadSeq = 0;
  // Supersession token for the async dependency read (mirrors `docLoadSeq`). A
  // monotonic counter, not an identity check: `wsStore.workspace` is a Svelte
  // `$state` proxy, so `wsStore.workspace !== ws` (proxy vs raw) is always true
  // and would discard every successful resolve.
  let depLoadSeq = 0;
  // Returns the loaded doc groups (`[{ title, items:[{title,path,handle}] }]`)
  // so the caller can prefer a docs landing page, `[]` when the workspace has no
  // (or malformed) manifest, or `null` when a faster mount superseded this load.
  async function loadWorkspaceDocs(ws: PageWorkspace): Promise<LiveDocGroup[] | null> {
    const seq = (docLoadSeq += 1);
    if (!ws.manifestToml) return [];
    let manifest;
    try {
      manifest = docManifest(ws.manifestToml);
    } catch {
      return []; // malformed pds.toml — the auto docs still build
    }
    const groups: { title: string; items: { title: string; path: string; content: string; handle?: FileSystemFileHandle | null }[] }[] =
      ws.root
        ? await readDocPages(ws.root, ws.base ?? "", manifest.sidebar)
        : sampleDocPages(manifest.sidebar, ws.docs ?? {});
    // A later workspace may have mounted while we awaited; ignore a stale load.
    if (seq !== docLoadSeq) return null;
    const sources: Record<string, string> = {};
    for (const g of groups) for (const it of g.items) sources[it.path] = it.content;
    wsStore.docSources = sources;
    wsStore.docMeta = { name: manifest.name, theme: manifest.theme };
    wsStore.docGroups = groups.map((g) => ({
      title: g.title,
      items: g.items.map(({ title, path, handle }) => ({ title, path, handle: handle ?? null })),
    }));
    // Seed the on-disk baseline for folder-backed doc pages (those with a handle)
    // so they start clean; sample doc pages stay session-only (no baseline).
    if (ws.root) {
      const docBaseline: { key: string; text: string }[] = [];
      for (const g of groups) for (const it of g.items) if (it.handle) docBaseline.push({ key: it.path, text: it.content });
      seedBaseline(docBaseline);
    }
    return docGroups;
  }

  // ---- shared file-set mutation (T9/T10/T11) -------------------------------
  // The single affordance through which the FileTree create/rename/move/delete
  // flows reshape the model. It reassigns `workspace.files` and applies seed/
  // rename/drop edits to `moduleSources`, re-seeding the dirty baseline so
  // seeded/renamed buffers start clean. Reassigning these reactive maps re-runs
  // the whole `$derived` resolution chain off `allModules` — no explicit
  // re-resolve call is needed.
  //
  //   seed   : { [fqn]: text } modules to add with a clean baseline.
  //   rename : { from, to } to move a module's source (+ baseline) to a new key.
  //   drop   : [fqn] modules to remove from sources (+ baseline).
  function applyFileSet(
    files: OpenFile[],
    {
      seed = {},
      rename = null,
      drop = [],
    }: { seed?: Record<string, string>; rename?: { from: string; to: string } | null; drop?: string[] } = {},
  ) {
    const sources = { ...moduleSources };
    const base = { ...persisted };
    if (rename && rename.from !== rename.to) {
      if (rename.from in sources) {
        sources[rename.to] = sources[rename.from];
        delete sources[rename.from];
      }
      if (rename.from in base) {
        base[rename.to] = base[rename.from];
        delete base[rename.from];
      }
    }
    for (const fqn of drop) {
      delete sources[fqn];
      delete base[fqn];
    }
    for (const [fqn, text] of Object.entries(seed)) {
      sources[fqn] = text;
      if (workspace?.root) base[fqn] = text; // folder-backed → on-disk baseline (clean)
    }
    wsStore.moduleSources = sources;
    saveStore.persisted = base;
    wsStore.workspace = { ...workspace!, files };
  }

  // Persist a programmatic `[[doc.sidebar]]` manifest change (T10): update the
  // live `workspace.manifestToml` + editable buffer, write `pds.toml` to disk
  // when folder-backed, re-seed its baseline, and re-resolve doc nav.
  async function persistManifest(toml: string) {
    const handle = workspace?.manifest?.handle;
    wsStore.workspace = { ...workspace!, manifestToml: toml };
    wsStore.manifestSource = toml;
    if (handle && workspace.manifest) {
      await writeFile(handle, toml);
      seedBaseline([{ key: workspace.manifest.path, text: toml }]);
    }
  }

  // One dialog drives every FileTree name prompt. `dialog` holds its config or
  // is null when closed; `confirmDialog` is the destructive-action confirm.
  const dialog = $derived(ui.dialog);
  const confirmDialog = $derived(ui.confirmDialog);

  // IDE commands surfaced in the palette's "Actions" tab. `keywords` widens the
  // fuzzy match beyond the visible label; the palette closes before each runs.
  type PaletteAction = { id: string; label: string; hint?: string; keywords?: string; run: () => void };
  const paletteActions = $derived.by<PaletteAction[]>(() => {
    const list: PaletteAction[] = [
      { id: "new-file", label: "New .pds file", keywords: "module create add", run: () => startNewFile() },
      { id: "new-folder", label: "New folder", keywords: "directory create", run: () => startNewFolder() },
      { id: "new-doc", label: "New doc page", keywords: "markdown md create", run: () => startNewDoc() },
      { id: "open-manifest", label: "Open pds.toml", keywords: "manifest config dependencies", run: openManifest },
      { id: "save", label: "Save file", hint: "⌘S", keywords: "write disk", run: () => saveActiveFile() },
      { id: "save-all", label: "Save all", hint: "⇧⌘S", keywords: "write disk", run: () => saveAll() },
      { id: "toggle-explorer", label: "Toggle Explorer", keywords: "files tree sidebar panel", run: () => (ui.explorerOpen = !ui.explorerOpen) },
      { id: "toggle-structure", label: "Toggle Structure", keywords: "outline symbols panel", run: () => (ui.structureOpen = !ui.structureOpen) },
      { id: "toggle-problems", label: "Toggle Problems", keywords: "diagnostics errors dock panel", run: () => (ui.problemsOpen = !ui.problemsOpen) },
      { id: "theme", label: theme.resolved === "dark" ? "Switch to light theme" : "Switch to dark theme", keywords: "appearance colour dark light", run: () => theme.set(theme.resolved === "dark" ? "light" : "dark") },
      { id: "settings", label: "Settings", keywords: "settings keybindings shortcuts preferences ai completion", run: () => (ui.settingsOpen = true) },
      { id: "switch-project", label: "Switch project…", keywords: "open recent examples launcher", run: () => (ui.projectOpen = true) },
      { id: "new-project", label: "New project…", keywords: "create template scaffold", run: () => (ui.newProjectOpen = true) },
    ];
    if (fsSupported) list.push({ id: "open-folder", label: "Open folder…", keywords: "disk filesystem import", run: () => openFolder() });
    return list;
  });

  // A minimal valid `.pds` module: a system shell with one container and one
  // behaviour, mirroring the new-workspace starter so it resolves clean.
  // ---- T9: new .pds file ---------------------------------------------------
  // Thin view wrappers over the pure `core/workspace-ops` helpers, supplying the
  // live workspace context (file set, base dir) the pure functions take as args.
  const pdsSkeleton = (fqn: string) => ops.pdsSkeleton(fqn);
  const normalizePdsPath = (name: string) => ops.normalizePdsPath(name);
  const withBase = (rel: string) => ops.withBase(workspace?.base, rel);
  const validateNewFile = (name: string) => ops.validateNewFile(name, workspace?.files ?? [], workspace?.base);

  function startNewFile(dir?: string) {
    if (!workspace) return;
    ui.dialog = {
      title: "New .pds file",
      label: "Module path",
      placeholder: "banking/core",
      value: dir ? `${dir}/` : "",
      confirmLabel: "Create",
      hint: "A .pds extension is added automatically. Use / for subfolders.",
      validate: validateNewFile,
      run: createNewFile,
    };
  }

  // ---- new folder (a real directory on disk) -------------------------------
  const normalizeDirPath = (name: string) => ops.normalizeDirPath(name);

  function validateNewFolder(name: string): string | null {
    const rel = normalizeDirPath(name);
    if (!rel) return "Enter a folder name.";
    if ((workspace?.dirs ?? []).includes(rel)) return "That folder already exists.";
    return null;
  }

  function startNewFolder(dir?: string) {
    if (!workspace) return;
    ui.dialog = {
      title: "New folder",
      label: "Folder path",
      placeholder: "banking/adapters",
      value: dir ? `${dir}/` : "",
      confirmLabel: "Create",
      hint: "Use / for nested folders.",
      validate: validateNewFolder,
      run: createNewFolder,
    };
  }

  async function createNewFolder(name: string) {
    const ws = workspace;
    if (!ws) return;
    const rel = normalizeDirPath(name);
    if (!rel) return;
    if (ws.root) {
      try {
        await createDir(ws.root, withBase(rel));
      } catch (e) {
        notify("error", "Couldn't create folder", String((e as Error)?.message ?? e));
        return;
      }
    }
    const dirs = Array.from(new Set([...(ws.dirs ?? []), rel])).sort();
    wsStore.workspace = { ...ws, dirs };
    notify("success", `Created ${rel}/`);
  }

  // Base-relative path of a module file ("" base → path as-is).
  const relPathOf = (p: string) =>
    workspace?.base && p.startsWith(`${workspace.base}/`) ? p.slice(workspace.base.length + 1) : p;

  // The modules and doc pages that live under a base-relative folder. Folder
  // rename/delete handle modules + the directory tree; doc pages (manifest-
  // registered) aren't individually movable here, so a folder holding them is
  // refused rather than half-rewritten.
  function folderContents(rel: string) {
    const prefix = `${rel}/`;
    const modules = (workspace?.files ?? []).filter((f) => f.path && relPathOf(f.path).startsWith(prefix));
    const docs = docGroups.flatMap((g) => g.items).filter((it) => it.path.startsWith(prefix));
    return { modules, docs };
  }

  // Consolidated reactive update for a folder edit: rekey moduleSources/baseline
  // for each rename, drop deleted modules, then publish files + dirs in one shot.
  // Mirrors applyFileSet but for the many renames/drops a folder op emits at once.
  function applyFolderEdit(
    files: OpenFile[],
    dirs: string[],
    renames: Array<{ from: string; to: string }>,
    drop: string[],
  ) {
    const sources = { ...moduleSources };
    const base = { ...persisted };
    for (const { from, to } of renames) {
      if (from === to) continue;
      if (from in sources) {
        sources[to] = sources[from];
        delete sources[from];
      }
      if (from in base) {
        base[to] = base[from];
        delete base[from];
      }
    }
    for (const fqn of drop) {
      delete sources[fqn];
      delete base[fqn];
    }
    wsStore.moduleSources = sources;
    saveStore.persisted = base;
    wsStore.workspace = { ...workspace!, files, dirs };
  }

  const remapDirs = (dirs: string[], oldRel: string, newRel: string) => ops.remapDirs(dirs, oldRel, newRel);
  const folderRenameClash = (oldRel: string, newRel: string) => ops.folderRenameClash(oldRel, newRel);

  function validateRenameFolder(oldRel: string, name: string): string | null {
    const newRel = normalizeDirPath(name);
    if (!newRel) return "Enter a folder name.";
    if (newRel === oldRel) return null;
    if (folderRenameClash(oldRel, newRel)) return "Choose a folder outside the one being renamed.";
    if ((workspace?.dirs ?? []).includes(newRel)) return "That folder already exists.";
    if ((workspace?.files ?? []).some((f) => f.path && relPathOf(f.path).startsWith(`${newRel}/`)))
      return "That folder already exists.";
    if (folderContents(oldRel).docs.length) return "Folder contains doc pages — move those from the manifest first.";
    return null;
  }

  function startRenameFolder(rel: string) {
    if (!workspace) return;
    ui.dialog = {
      title: "Rename folder",
      label: "Folder path",
      placeholder: "banking/adapters",
      value: rel,
      confirmLabel: "Rename",
      hint: "Moves every module inside; importers of the old FQNs may dangle.",
      validate: (name: string) => validateRenameFolder(rel, name),
      run: (name: string) => renameFolder(rel, name),
    };
  }

  async function renameFolder(oldRel: string, name: string) {
    const ws = workspace;
    if (!ws) return;
    const newRel = normalizeDirPath(name);
    if (!newRel || folderRenameClash(oldRel, newRel)) return;
    const { modules, docs } = folderContents(oldRel);
    if (docs.length) {
      notify("error", "Can't rename folder", "It contains doc pages — move those from the manifest first.");
      return;
    }
    // Each module's destination, preserving its sub-path under the folder.
    const moves = modules.map((f) => {
      const newPath = withBase(`${newRel}${relPathOf(f.path!).slice(oldRel.length)}`);
      return { file: f, oldPath: f.path!, newPath, oldFqn: f.fqn!, newFqn: fqnOf(newPath, ws.base ?? "") };
    });
    if (ws.root) {
      try {
        for (const m of moves) await movePath(ws.root, m.oldPath, m.newPath, moduleSources[m.oldFqn] ?? "");
        // preserve empty subfolders: recreate the mapped tree, then drop the old
        for (const d of ws.dirs ?? [])
          if (d === oldRel || d.startsWith(`${oldRel}/`)) await createDir(ws.root, withBase(`${newRel}${d.slice(oldRel.length)}`));
        await deleteDir(ws.root, withBase(oldRel));
      } catch (e) {
        notify("error", "Couldn't rename folder", String((e as Error)?.message ?? e));
        return;
      }
    }
    const moved = new Map(moves.map((m) => [m.file, m] as const));
    const files = ws.files
      .map((f) => {
        const m = moved.get(f);
        return m ? { path: m.newPath, fqn: m.newFqn, handle: f.handle } : f;
      })
      .sort((a, b) => (a.fqn ?? "").localeCompare(b.fqn ?? ""));
    applyFolderEdit(files, remapDirs(ws.dirs ?? [], oldRel, newRel), moves.map((m) => ({ from: m.oldFqn, to: m.newFqn })), []);
    if (openFile && !openFile.isDoc && !openFile.isManifest) {
      const m = moves.find((x) => x.oldFqn === openFile!.fqn);
      if (m) wsStore.openFile = { path: m.newPath, fqn: m.newFqn, handle: openFile.handle };
    }
    notify(
      "success",
      `Renamed to ${newRel}/`,
      modules.length ? `${modules.length} module(s) moved; importers of the old FQNs may dangle.` : undefined,
    );
  }

  function deleteFolder(rel: string) {
    if (!workspace) return;
    const { modules, docs } = folderContents(rel);
    if (docs.length) {
      notify("error", "Can't delete folder", "It contains doc pages — move those from the manifest first.");
      return;
    }
    const msg = modules.length
      ? `Delete ${rel}/ and ${modules.length} module${modules.length === 1 ? "" : "s"}? This removes them from disk and the model. Importers will dangle.`
      : `Delete the empty folder ${rel}/?`;
    ui.confirmDialog = { title: "Delete folder", message: msg, confirmLabel: "Delete", run: () => performDeleteFolder(rel, modules) };
  }

  async function performDeleteFolder(rel: string, modules: OpenFile[]) {
    const ws = workspace;
    if (!ws) return;
    if (ws.root) {
      try {
        await deleteDir(ws.root, withBase(rel));
      } catch (e) {
        notify("error", "Couldn't delete folder", String((e as Error)?.message ?? e));
        return;
      }
    }
    const gone = new Set(modules);
    const files = ws.files.filter((f) => !gone.has(f));
    const dirs = (ws.dirs ?? []).filter((d) => d !== rel && !d.startsWith(`${rel}/`));
    applyFolderEdit(files, dirs, [], modules.map((m) => m.fqn).filter((f): f is string => !!f));
    if (openFile && !openFile.isDoc && !openFile.isManifest && modules.some((m) => m.fqn === openFile!.fqn)) {
      if (files[0]) selectFile(files[0]);
      else wsStore.openFile = null;
    }
    notify("success", `Deleted ${rel}/`, modules.length ? `${modules.length} module(s) removed.` : undefined);
  }

  async function createNewFile(name: string) {
    const ws = workspace;
    if (!ws) return;
    const rel = normalizePdsPath(name);
    const path = withBase(rel);
    const fqn = fqnOf(path, ws.base ?? "");
    const skeleton = pdsSkeleton(fqn);
    let handle: FileSystemFileHandle | null = null;
    if (ws.root) {
      try {
        handle = await createFile(ws.root, path, skeleton);
      } catch (e) {
        notify("error", "Couldn't create file", String((e as Error)?.message ?? e));
        return;
      }
    }
    const newFile: OpenFile = { path, fqn, handle };
    const files = [...ws.files, newFile].sort((a, b) => (a.fqn ?? "").localeCompare(b.fqn ?? ""));
    applyFileSet(files, { seed: { [fqn]: skeleton } });
    selectFile(newFile);
    notify("success", `Created ${rel}`);
  }

  // ---- T10: new doc (.md + [[doc.sidebar]] registration) -------------------
  const slugify = (title: string) => ops.slugify(title);
  const docPathSet = () => ops.docPathSet(docGroups);
  const validateNewDoc = (title: string) => ops.validateNewDoc(title, docPathSet());

  function startNewDoc() {
    if (!workspace) return;
    ui.dialog = {
      title: "New doc page",
      label: "Page title",
      placeholder: "Release Notes",
      value: "",
      confirmLabel: "Create",
      hint: "Saved as docs/<slug>.md and added to the sidebar.",
      validate: validateNewDoc,
      run: createNewDoc,
    };
  }

  async function createNewDoc(title: string) {
    const ws = workspace;
    if (!ws) return;
    const path = `docs/${slugify(title)}.md`;
    const body = `# ${title}\n\nDescribe ${title} here.\n`;

    let handle: FileSystemFileHandle | null = null;
    if (ws.root) {
      try {
        handle = await createFile(ws.root, withBase(path), body);
      } catch (e) {
        notify("error", "Couldn't create doc", String((e as Error)?.message ?? e));
        return;
      }
    }

    // Register in the manifest: append to the first sidebar group, or a new
    // "Docs" group when none exist. Reuse the wasm manifest model, then
    // serialise back to TOML.
    let toml: string;
    try {
      const manifest = docManifest(ws.manifestToml ?? "");
      const sidebar = (manifest.sidebar ?? []).map((g) => ({ title: g.title, items: [...(g.items ?? [])] }));
      if (sidebar.length === 0) sidebar.push({ title: "Docs", items: [] });
      sidebar[0].items.push({ title, path });
      toml = serializeManifest(ws.manifestToml ?? "", { ...manifest, sidebar });
    } catch (e) {
      if (handle && ws.root) {
        try {
          await deletePath(ws.root, withBase(path)); // don't orphan an unregistered page
        } catch {}
      }
      notify("error", "Couldn't register doc", String((e as Error)?.message ?? e));
      return;
    }

    // Live: add to docGroups + docSources so the sidebar/preview update now.
    const item: LiveDocItem = { title, path, handle };
    wsStore.docSources = { ...docSources, [path]: body };
    if (ws.root && handle) seedBaseline([{ key: path, text: body }]);
    wsStore.docGroups = docGroups.length
      ? docGroups.map((g, i) => (i === 0 ? { ...g, items: [...g.items, item] } : g))
      : [{ title: "Docs", items: [item] }];

    try {
      await persistManifest(toml);
    } catch (e) {
      notify("error", "Saved the page, but couldn't write pds.toml", String((e as Error)?.message ?? e));
    }
    openDoc(item);
    notify("success", `Created ${path}`);
  }

  // ---- T11: rename / move / delete .pds ------------------------------------
  const validateRename = (file: OpenFile, name: string) =>
    ops.validateRename(file, name, workspace?.files ?? [], workspace?.base);

  function startRenameFile(file: OpenFile) {
    if (!workspace || !file.path) return;
    const rel = workspace.base ? file.path.slice(workspace.base.length + 1) : file.path;
    ui.dialog = {
      title: "Rename module",
      label: "Module path",
      placeholder: "banking/core",
      value: rel.replace(/\.pds$/, ""),
      confirmLabel: "Rename",
      hint: "Renaming changes the module FQN; importers of the old name may dangle.",
      validate: (name: string) => validateRename(file, name),
      run: (name: string) => renameFile(file, name),
    };
  }

  async function renameFile(file: OpenFile, name: string) {
    const newPath = withBase(normalizePdsPath(name));
    if (newPath !== file.path) await relocate(file, newPath);
  }

  // Drag-and-drop move: `destDir` is a base-relative directory ("" = root).
  async function moveFile({ file, destDir }: { file: OpenFile; destDir: string }) {
    const ws = workspace;
    if (!ws || !file.path) return;
    const leaf = file.path.split("/").pop() ?? "";
    const prefix = destDir ? withBase(destDir) : (ws.base ?? "");
    const newPath = prefix ? `${prefix}/${leaf}` : leaf;
    if (newPath === file.path) return; // same folder → no-op
    if ((ws.files ?? []).some((f) => f.path === newPath)) {
      notify("error", "Can't move file", "A file with that name already exists there.");
      return;
    }
    await relocate(file, newPath);
  }

  // Shared rename/move core: disk move (with rollback), then memory rekey.
  async function relocate(file: OpenFile, newPath: string) {
    const ws = workspace;
    if (!ws || !file.path || !file.fqn) return;
    const oldFqn = file.fqn;
    const newFqn = fqnOf(newPath, ws.base ?? "");
    const source = moduleSources[oldFqn] ?? "";
    let handle = file.handle ?? null;
    if (ws.root) {
      try {
        handle = await movePath(ws.root, file.path, newPath, source);
      } catch (e) {
        notify("error", "Couldn't move file", String((e as Error)?.message ?? e)); // disk unchanged → leave memory
        return;
      }
    }
    const updated: OpenFile = { path: newPath, fqn: newFqn, handle };
    const files = ws.files.map((f) => (f === file ? updated : f)).sort((a, b) => (a.fqn ?? "").localeCompare(b.fqn ?? ""));
    applyFileSet(files, { rename: { from: oldFqn, to: newFqn } });
    if (openFile && !openFile.isDoc && !openFile.isManifest && openFile.fqn === oldFqn) wsStore.openFile = updated;
    const importers = danglingImporters(newFqn, oldFqn);
    if (importers.length) notify("info", `Renamed to ${newFqn}`, `${importers.length} module(s) still import the old name.`);
    else notify("success", `Renamed to ${newFqn}`);
  }

  const danglingImporters = (newFqn: string, oldFqn: string) =>
    ops.danglingImporters(workspace?.files ?? [], moduleSources, newFqn, oldFqn);

  function requestDeleteFile(file: OpenFile) {
    if (!workspace) return;
    ui.confirmDialog = {
      title: "Delete module",
      message: `Delete ${file.fqn}? This removes the file from disk and the model. Importers will dangle.`,
      confirmLabel: "Delete",
      run: () => deleteFile(file),
    };
  }

  async function deleteFile(file: OpenFile) {
    const ws = workspace;
    if (!ws || !file.path || !file.fqn) return;
    if (ws.root) {
      try {
        await deletePath(ws.root, file.path);
      } catch (e) {
        notify("error", "Couldn't delete file", String((e as Error)?.message ?? e));
        return;
      }
    }
    const files = ws.files.filter((f) => f !== file);
    applyFileSet(files, { drop: [file.fqn] });
    if (openFile && !openFile.isDoc && !openFile.isManifest && openFile.fqn === file.fqn) {
      if (files[0]) selectFile(files[0]);
      else wsStore.openFile = null;
    }
    notify("success", `Deleted ${file.fqn}`);
  }

  // Re-open a recent project from its stored folder handle (falling back to the
  // picker if the handle is gone or permission is denied). Legacy sample recents
  // (from before examples became templates) no longer re-open — they're forgotten.
  async function openRecent(entry: Recent) {
    if (entry.kind === "sample") {
      forget(entry.key);
      refreshRecents();
      return;
    }
    const handle = await reopenFolder(entry.key);
    if (!handle) {
      openFolder();
      return;
    }
    try {
      const ws = await readWorkspace(handle);
      await adoptWorkspace(ws);
      notify("success", `Opened ${ws.name} · ${ws.files.length} modules`);
    } catch {
      openFolder();
    }
  }

  function forgetRecent(entry: Recent) {
    forget(entry.key);
    refreshRecents();
  }

  // Briefly show a "saved" cue after a successful write, then settle to idle.
  function markSaved() {
    saveStore.saveState = "saved";
    clearTimeout(saveStateTimer);
    saveStateTimer = setTimeout(() => (saveStore.saveState = "idle"), 1600);
  }

  // Seed the persisted baseline for a batch of files read from disk, so they
  // start clean. `entries` is `[{ key, text }]`.
  function seedBaseline(entries: { key: string; text: string }[]) {
    saveStore.persisted = advanceBaseline(persisted, entries);
  }

  // Write one buffer to disk and, on success, advance its baseline so it's no
  // longer dirty. Returns the write promise (already resolved for handle-less
  // samples). Failure keeps the baseline stale (still dirty) and surfaces it.
  async function persistFile(handle: FileSystemFileHandle | null | undefined, key: string, text: string) {
    if (!handle) return; // in-memory sample: session-only, no baseline to advance
    saveStore.saveState = "saving";
    try {
      await writeFile(handle, text);
      saveStore.persisted = { ...persisted, [key]: text };
      markSaved();
      // A saved manifest re-resolves the doc nav / name / theme.
      if (key === manifestKey) resolveManifest(text);
    } catch (e) {
      saveStore.saveState = "error";
      notify("error", "Could not save to disk", String((e as Error)?.message ?? e));
      throw e;
    }
  }

  // Pull in edits made outside the IDE: re-read every open file's handle and, for
  // any whose disk content diverged from our baseline, reload it — provided its
  // buffer is clean. A buffer with unsaved edits is left alone (the conflict is
  // surfaced once) so external changes never clobber in-flight work. Runs on
  // window focus and a modest visible-only timer; comparing content (not mtime)
  // means our own saves never look external.
  // Pick up `.pds` modules created or deleted in the workspace directory outside
  // the IDE — e.g. an agent scaffolding or removing files. Re-walks the directory
  // and reconciles the module set: new files are read in, deleted files are
  // dropped (a file with unsaved edits is kept so external deletion never loses
  // work). The mount `$effect` then re-mounts the session and `sessionMount`
  // re-derives the outline/diagnostics, so the tree and model update live. The
  // content of files that still exist is reloaded by the caller below.
  async function reconcileExternalStructure() {
    const ws = workspace;
    if (!ws?.root) return;
    let scanned: PageWorkspace;
    try {
      scanned = await readWorkspace(ws.root);
    } catch {
      return; // a transient walk failure — leave the structure as-is this tick
    }
    const diskFqns = new Set(scanned.files.map((f) => f.fqn));
    const known = ws.files.filter((f) => f.fqn);
    const knownFqns = new Set(known.map((f) => f.fqn));
    const created = scanned.files.filter((f) => !knownFqns.has(f.fqn));
    const isDirty = (fqn: string) => moduleSources[fqn] !== persisted[fqn];
    const removed = known.filter((f) => !diskFqns.has(f.fqn) && !isDirty(f.fqn!));
    if (created.length === 0 && removed.length === 0) return;

    const sources = { ...moduleSources };
    const baseline = { ...persisted };
    for (const f of created) {
      if (!f.fqn || !f.handle) continue;
      try {
        const text = await readFile(f.handle);
        sources[f.fqn] = text;
        baseline[f.fqn] = text;
      } catch {
        /* vanished between the walk and the read — ignore */
      }
    }
    for (const f of removed) {
      if (!f.fqn) continue;
      delete sources[f.fqn];
      delete baseline[f.fqn];
    }
    wsStore.moduleSources = sources;
    saveStore.persisted = baseline;

    // Adopt the scan's file/dir/other listing, but keep any locally-dirty file the
    // scan no longer sees so its unsaved buffer stays editable (and saveable).
    const removedFqns = new Set(removed.map((f) => f.fqn));
    const dirtyKept = known.filter((f) => !diskFqns.has(f.fqn) && !removedFqns.has(f.fqn));
    const files = [...scanned.files, ...dirtyKept].sort((a, b) => (a.fqn ?? "").localeCompare(b.fqn ?? ""));
    wsStore.workspace = { ...ws, files, dirs: scanned.dirs, others: scanned.others };

    // If the open module was deleted, fall back to a surviving file.
    if (openFile?.fqn && !files.some((f) => f.fqn === openFile.fqn)) {
      wsStore.openTabs = wsStore.openTabs.filter((t) => files.some((f) => keyOf(f) === keyOf(t)));
      wsStore.openFile = wsStore.openTabs[0] ?? files[0] ?? null;
    }

    if (created.length) notify("info", `${created.length} file${created.length === 1 ? "" : "s"} added on disk`);
    if (removed.length) notify("info", `${removed.length} file${removed.length === 1 ? "" : "s"} removed on disk`);
  }

  async function reloadExternalChanges() {
    if (!workspace?.root || saveState === "saving") return;
    await reconcileExternalStructure();
    const targets: { key: string; handle: FileSystemFileHandle; kind: "module" | "doc" | "manifest" | "other" }[] = [];
    for (const f of workspace.files) if (f.handle && f.fqn) targets.push({ key: f.fqn, handle: f.handle, kind: "module" });
    if (workspace.manifest?.handle && manifestKey)
      targets.push({ key: manifestKey, handle: workspace.manifest.handle, kind: "manifest" });
    for (const g of docGroups) for (const it of g.items) if (it.handle) targets.push({ key: it.path, handle: it.handle, kind: "doc" });
    // Only opened companions carry a baseline; the rest skip below (`base === undefined`).
    for (const o of workspace.others ?? []) if (o.handle) targets.push({ key: rel(o.path ?? ""), handle: o.handle, kind: "other" });

    let conflicts = 0;
    for (const { key, handle, kind } of targets) {
      const base = persisted[key];
      if (base === undefined) continue;
      let disk: string;
      try {
        disk = await readFile(handle);
      } catch {
        continue; // file removed or unreadable this tick — skip
      }
      const buffer = kind === "module" ? moduleSources[key] : kind === "doc" ? docSources[key] : kind === "other" ? otherSources[key] : manifestSource;
      const action = classifyReload(disk, base, buffer);
      if (action === "skip") continue;
      if (action === "conflict") {
        conflicts += 1;
        continue;
      }
      applyExternalReload(key, disk, kind);
    }
    if (conflicts > 0) notify("info", `${conflicts} file(s) changed on disk — your unsaved edits are kept`);
  }

  // Apply one externally-changed file: update its live buffer (which flows to the
  // editor and re-derives the model) and advance its baseline so it reads clean.
  // A module's new text is pushed into the session first (as an in-IDE edit is),
  // so the held state the outline/diagnostics query reflects the external change.
  function applyExternalReload(key: string, disk: string, kind: "module" | "doc" | "manifest" | "other") {
    if (kind === "module") {
      setIdeSource(key, disk);
      wsStore.moduleSources = { ...moduleSources, [key]: disk };
    }
    else if (kind === "doc") wsStore.docSources = { ...docSources, [key]: disk };
    else if (kind === "other") wsStore.otherSources = { ...otherSources, [key]: disk };
    else {
      wsStore.manifestSource = disk;
      resolveManifest(disk);
    }
    saveStore.persisted = { ...persisted, [key]: disk };
  }

  // Toast notifications — owned by the notifications store; thin view wrappers.
  const notify = (kind: NoteKind, title: string, body = "") => notifications.notify(kind, title, body);
  const dismissNote = (id: string | number) => notifications.dismiss(id);

  // One toast per AI-completion failure kind: the chip carries the ongoing
  // state, so repeated failures of the same kind stay quiet until the kind
  // changes (a success clears `lastError` and re-arms the toast).
  let toastedAiKind: string | null = null;
  $effect(() => {
    const err = llm.lastError;
    if (!err) {
      toastedAiKind = null;
      return;
    }
    if (err.kind === toastedAiKind) return;
    toastedAiKind = err.kind;
    notify("error", "AI completion failed", `${err.message}. Click the AI chip to fix it.`);
  });

  // LSP symbol rename. The editor's right-click hands us the byte offset of the
  // symbol; we resolve every occurrence (find-references) and open the preview
  // dialog. Applying rewrites the chosen modules' buffers (dirty until saved).
  let renamePrompt = $state<{ symbol: string; offset: number; occurrences: Occurrence[] } | null>(null);
  function requestRename(offset: number) {
    if (!openFile?.fqn) return;
    let refs: References | null = null;
    try {
      refs = ideReferences(openFile.fqn, offset);
    } catch {
      refs = null;
    }
    if (!refs || refs.occurrences.length === 0) {
      notify("info", "Cannot rename", "The cursor is not on a renameable symbol.");
      return;
    }
    renamePrompt = { symbol: refs.fqn.split("::").at(-1) ?? refs.fqn, offset, occurrences: refs.occurrences };
  }
  function applyRename(newName: string, selected: RenameSelection[]) {
    const prompt = renamePrompt;
    renamePrompt = null;
    if (!prompt || !openFile?.fqn) return;
    let edited: { fqn: string; source: string }[];
    try {
      edited = ideRename(openFile.fqn, prompt.offset, newName, selected);
    } catch (e) {
      notify("error", "Rename failed", String((e as Error)?.message ?? e));
      return;
    }
    if (edited.length === 0) return;
    const next = { ...moduleSources };
    for (const e of edited) next[e.fqn] = e.source;
    wsStore.moduleSources = next;
    notify("success", `Renamed ${selected.length} occurrence${selected.length === 1 ? "" : "s"} to ${newName}`);
  }

  // Save every dirty buffer to disk (File ▸ Save all): persist each dirty module
  // / doc / manifest by its handle.
  async function saveAll() {
    for (const key of [...dirty]) {
      let handle: FileSystemFileHandle | null | undefined;
      let text: string | undefined;
      if (key === manifestKey) {
        handle = workspace?.manifest?.handle;
        text = manifestSource;
      } else if (key in moduleSources) {
        handle = workspace?.files.find((f) => f.fqn === key)?.handle;
        text = moduleSources[key];
      } else {
        handle = docGroups.flatMap((g) => g.items).find((i) => i.path === key)?.handle;
        text = docSources[key];
      }
      if (handle && text !== undefined) await persistFile(handle, key, text).catch(() => {});
    }
  }

  // Manual save (Cmd/Ctrl-S): write the active file's current buffer to disk. A
  // clean file is a no-op cue.
  async function saveActiveFile() {
    if (!openFile?.handle) return;
    const key = keyOf(openFile);
    if (!key || !dirty.has(key)) {
      markSaved();
      return;
    }
    await persistFile(openFile.handle, key, source).catch(() => {});
  }

  function onEditorChange(value: string) {
    if (!openFile) return;
    if (openFile.isManifest) {
      wsStore.manifestSource = value;
      // A saved folder manifest re-resolves the doc nav; until then show only the
      // live parse cue. A session-only sample has no save, so re-resolve live.
      if (openFile.handle) validateManifest(value);
      else resolveManifest(value);
    } else if (openFile.isOther) {
      wsStore.otherSources = { ...otherSources, [openFile.path ?? ""]: value };
    } else if (openFile.isDoc) {
      wsStore.docSources = { ...docSources, [openFile.path ?? ""]: value };
    } else {
      // Push the edit into the IDE-core session before the reactive write, so the
      // diagnostics derived (re-run by the `moduleSources` change) queries current
      // state. No whole-workspace re-marshalling — one module.
      setIdeSource(openFile.fqn ?? "", value);
      wsStore.moduleSources = { ...moduleSources, [openFile.fqn ?? ""]: value };
    }
    // No autosave: edits stay in the in-memory buffer (and show as dirty on the
    // tab) until an explicit save — Cmd/Ctrl-S or File ▸ Save / Save all.
    // Keep the URL in step: refresh the embedded base (in-memory only) and the
    // caret, both debounced so a fast typist isn't re-encoding on every keystroke.
    recomputeWPayload();
    scheduleCaretSync();
  }

  // Live parse check for the inline error cue; doesn't touch the doc nav.
  function validateManifest(toml: string) {
    try {
      docManifest(toml);
      wsStore.manifestError = null;
    } catch (e) {
      wsStore.manifestError = String((e as Error)?.message ?? e);
    }
  }

  // Re-resolve the workspace doc nav / name / theme from the saved manifest. A
  // parse error keeps the last good doc nav; reuses the shared doc loader by
  // swapping the live manifest text onto the workspace.
  function resolveManifest(toml: string) {
    try {
      docManifest(toml); // throws on malformed TOML
      wsStore.manifestError = null;
    } catch (e) {
      wsStore.manifestError = String((e as Error)?.message ?? e);
      return; // keep the last good doc nav
    }
    if (workspace) loadWorkspaceDocs({ ...workspace, manifestToml: toml });
  }

  // Open the workspace manifest (`pds.toml`) as raw, editable TOML. A folder's
  // manifest persists to its handle; a sample's is session-only (no handle).
  function openManifest() {
    if (!workspace?.manifest) return;
    wsStore.openFile = {
      isManifest: true,
      path: workspace.manifest.path,
      title: "pds.toml",
      handle: workspace.manifest.handle ?? null,
    };
    selection.selected = null;
    selection.view = "code";
  }

  // Opening a file from the nav clears any node scope; it shows the source,
  // unless the canvas is up — then it stays on the canvas (whole-model context).
  function selectFile(file: OpenFile) {
    wsStore.openFile = file;
    selection.selected = null;
    if (view !== "canvas") selection.view = "code";
  }

  // Open an authored doc page (`[[doc.sidebar]]`) as raw Markdown in the editor.
  // Marked `isDoc` so the editor drops PseudoScript language features and edits
  // route to `docSources` (and save to the page's handle on a real folder).
  function openDoc(item: LiveDocItem) {
    wsStore.openFile = { isDoc: true, path: item.path, title: item.title, handle: item.handle ?? null };
    selection.selected = null;
    selection.view = "code";
  }

  // Open a companion file (any non-PDS file in the tree). Text files load their
  // content into `otherSources` lazily and open editable in the editor; binaries
  // show an inert placeholder, so they skip the read.
  async function openOther(key: string) {
    const o = (workspace?.others ?? []).find((x) => rel(x.path ?? "") === key);
    if (!o) return;
    const binary = isBinaryPath(key);
    const base = { isOther: true as const, binary, path: key, title: key.split("/").at(-1) ?? key, handle: o.handle ?? null };
    wsStore.openFile = base;
    selection.selected = null;
    selection.view = "code";
    if (!o.handle) return; // in-memory: nothing to read
    try {
      const file = await o.handle.getFile();
      // Stale-guard: a fast tab switch may have moved on before this resolved.
      if (keyOf(wsStore.openFile) === key) wsStore.openFile = { ...base, bytes: file.size };
      if (!binary && otherSources[key] === undefined) {
        const text = file.size > MAX_OTHER_TEXT_BYTES ? "" : await file.text();
        wsStore.otherSources = { ...wsStore.otherSources, [key]: text };
        // Baseline it so edits show dirty and Cmd-S persists to the handle.
        seedBaseline([{ key, text }]);
      }
    } catch {
      if (!binary) wsStore.otherSources = { ...wsStore.otherSources, [key]: "" };
    }
  }

  // The workspace-base-relative form of a base-prefixed path (the file tree's key
  // space): drops the manifest-base prefix so keys match `fileEntries` relPaths.
  function rel(p: string): string {
    const b = workspace?.base ?? "";
    return b && p.startsWith(`${b}/`) ? p.slice(b.length + 1) : p;
  }

  // A human-readable byte count for the binary-file placeholder.
  function formatBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    const units = ["KB", "MB", "GB"];
    let v = n / 1024;
    let i = 0;
    while (v >= 1024 && i < units.length - 1) {
      v /= 1024;
      i += 1;
    }
    return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
  }

  // Open a unified file-tree entry by its kind.
  function openEntry(e: { key: string; kind: "module" | "doc" | "manifest" | "other"; fqn?: string }) {
    if (e.kind === "manifest") return openManifest();
    if (e.kind === "other") return void openOther(e.key);
    if (e.kind === "doc") {
      const it = docGroups.flatMap((g) => g.items).find((x) => x.path === e.key);
      if (it) openDoc(it);
      return;
    }
    const f = workspace?.files.find((x) => x.fqn === e.fqn);
    if (f) selectFile(f);
  }
  const moduleByFqn = (fqn: string) => workspace?.files.find((f) => f.fqn === fqn) ?? null;

  // Open a module and jump the editor to a 1-based line (text-search result).
  function openAtLine(fqn: string, line: number) {
    const f = moduleByFqn(fqn);
    if (!f) return;
    selectFile(f);
    selection.pendingGoto = { line, col: 1, fileFqn: fqn };
  }

  // ── Editor tabs ────────────────────────────────────────────────────────────
  const openTabs = $derived(wsStore.openTabs);
  // The active file is always an open tab.
  $effect(() => {
    const f = openFile;
    if (!f) return;
    const k = keyOf(f);
    if (k && !wsStore.openTabs.some((t) => keyOf(t) === k)) wsStore.openTabs = [...wsStore.openTabs, f];
  });
  // Drop tabs whose file no longer exists (delete / rename / manifest change).
  $effect(() => {
    if (!workspace) return;
    const valid = new Set<string>();
    for (const x of workspace.files) {
      const k = keyOf(x);
      if (k) valid.add(k);
    }
    for (const g of docGroups) for (const it of g.items) valid.add(it.path);
    if (manifestKey) valid.add(manifestKey);
    for (const o of workspace.others ?? []) valid.add(rel(o.path ?? ""));
    const pruned = wsStore.openTabs.filter((t) => valid.has(keyOf(t) ?? ""));
    if (pruned.length !== wsStore.openTabs.length) wsStore.openTabs = pruned;
  });
  const tabList = $derived(
    openTabs.map((f) => ({
      key: keyOf(f) ?? "",
      label: f.isManifest ? "pds.toml" : f.isDoc || f.isOther ? (f.title ?? f.path ?? "") : (f.fqn ?? ""),
      kind: (f.isManifest ? "manifest" : f.isDoc ? "doc" : f.isOther ? "other" : "module") as "module" | "doc" | "manifest" | "other",
      active: keyOf(f) === keyOf(openFile),
      dirty: dirty.has(keyOf(f) ?? ""),
    })),
  );
  function selectTab(key: string) {
    const f = openTabs.find((t) => keyOf(t) === key);
    if (f) selectFile(f);
  }
  // Commit a new tab list. If the active file survived, leave it; otherwise pick
  // the survivor at `preferIndex` (the slot the closed/active tab vacated, falling
  // back to the last tab), or clear the editor when nothing remains.
  function applyTabs(remaining: OpenFile[], preferIndex: number) {
    const activeKey = keyOf(openFile);
    wsStore.openTabs = remaining;
    if (remaining.some((t) => keyOf(t) === activeKey)) return;
    const next = remaining[Math.min(preferIndex, remaining.length - 1)] ?? null;
    if (next) selectFile(next);
    else wsStore.openFile = null;
  }
  function closeTab(key: string) {
    const idx = openTabs.findIndex((t) => keyOf(t) === key);
    applyTabs(
      openTabs.filter((t) => keyOf(t) !== key),
      idx,
    );
  }
  function closeOthers(key: string) {
    const keep = openTabs.find((t) => keyOf(t) === key);
    if (!keep) return;
    wsStore.openTabs = [keep];
    selectFile(keep);
  }
  function closeToRight(key: string) {
    const idx = openTabs.findIndex((t) => keyOf(t) === key);
    if (idx < 0) return;
    applyTabs(openTabs.slice(0, idx + 1), idx);
  }
  function closeAll() {
    wsStore.openTabs = [];
    wsStore.openFile = null;
  }
  // Move `fromKey` to sit immediately before `toKey`. Active file is unchanged.
  function reorderTabs(fromKey: string, toKey: string) {
    if (fromKey === toKey) return;
    const moved = openTabs.find((t) => keyOf(t) === fromKey);
    if (!moved) return;
    const next = openTabs.filter((t) => keyOf(t) !== fromKey);
    const insertAt = next.findIndex((t) => keyOf(t) === toKey);
    if (insertAt < 0) return;
    next.splice(insertAt, 0, moved);
    wsStore.openTabs = next;
  }

  async function onProblemPick(d: Problem) {
    selection.view = "code";
    if (d.file && workspace && d.file !== openFile?.fqn) {
      const f = workspace.files.find((x) => x.fqn === d.file);
      if (f) selectFile(f);
    }
    await tick();
    editorApi?.goto(d.start_line, d.start_col);
    if (d.file) recordLocation({ fileFqn: d.file, line: d.start_line, col: d.start_col, label: d.message });
  }

  // Copy one or all problems as plain text (the pane formats; this owns the
  // clipboard edge + flash, mirroring onshare).
  async function onProblemsCopy(text: string, count: number) {
    try {
      await navigator.clipboard.writeText(text);
      notify("success", `Copied ${count} problem${count === 1 ? "" : "s"} to clipboard`);
    } catch {
      notify("error", "Couldn't copy to clipboard");
    }
  }

  // Read every module into the live buffer, seed a clean on-disk baseline, mount,
  // and record the folder in recents. The single adopt path shared by opening a
  // folder, scaffolding a new project, and re-opening a recent — every workspace
  // is disk-backed, so this always has real handles to read and persist against.
  async function adoptWorkspace(ws: PageWorkspace, landing?: string | null) {
    const sources: Record<string, string> = {};
    for (const file of ws.files) if (file.handle) sources[file.fqn ?? ""] = await readFile(file.handle);
    wsStore.moduleSources = sources;
    saveStore.persisted = { ...sources };
    await mountWorkspace(ws, landing ?? ws.files[0]?.fqn);
    if (ws.root) {
      // Label the recent with the manifest's `[doc].name` when present (so a list
      // of `<root>/model` folders reads as their project titles, not "model"); the
      // folder name (`ws.name` === `root.name`) becomes the subtitle. `docManifest`
      // throws on malformed TOML — fall back to the folder name.
      let display = ws.name;
      if (ws.manifestToml) {
        try {
          display = docManifest(ws.manifestToml).name?.trim() || ws.name;
        } catch {
          /* malformed manifest — keep the folder name */
        }
      }
      await recordFolder(display, ws.name, ws.root);
      refreshRecents();
    }
  }

  // Bootstrap a project from a template — the empty one-module starter, or an
  // example scaffolded onto disk. Writes the files into the caller-chosen `parent`
  // folder, then opens it like any folder. A failed write is a no-op.
  async function newProject(name: string, templateId: string, parent: FileSystemDirectoryHandle) {
    const tpl =
      templateId === "empty" ? { seed: emptySeed(name), landing: null as string | null } : sampleSeed(templateId);
    if (!tpl) return;
    try {
      const ws = await scaffoldWorkspace(name, tpl.seed, parent);
      await adoptWorkspace(ws, tpl.landing);
      notify("success", `Created ${ws.name}`);
    } catch {
      // write or permission failure — keep the current workspace
    }
  }

  // Prompt for the New-project target folder (the dialog stores the handle).
  // Returns null if the picker is cancelled, so the dialog keeps its prior choice.
  async function chooseProjectFolder(): Promise<FileSystemDirectoryHandle | null> {
    try {
      return await pickDirectory();
    } catch {
      return null;
    }
  }

  async function openFolder() {
    try {
      const ws = await openWorkspace();
      await adoptWorkspace(ws);
      notify("success", `Opened ${ws.name} · ${ws.files.length} modules`);
    } catch {
      // picker cancelled or permission denied — keep the current workspace
    }
  }

  // Tear down the current workspace and return to the launcher. Mirrors the reset
  // half of `mountWorkspace`, then clears the on-disk buffers/baselines so no stale
  // state leaks into the next project. Dirty buffers confirm before discarding.
  function closeProject() {
    const teardown = () => {
      wsStore.workspace = null;
      wsStore.openFile = null;
      wsStore.openTabs = [];
      wsStore.moduleSources = {};
      wsStore.docGroups = [];
      wsStore.docSources = {};
      wsStore.docMeta = {};
      wsStore.manifestSource = "";
      wsStore.manifestError = null;
      selection.selected = null;
      selection.pendingGoto = null;
      selection.view = "code";
      navigation.reset();
      saveStore.persisted = {};
      saveStore.saveState = "idle";
      clearTimeout(saveStateTimer);
      router.clear(); // back to the bare launcher URL
      ui.projectOpen = true;
    };
    if (dirtyCount > 0) {
      ui.confirmDialog = {
        title: "Close project",
        message: "Discard unsaved changes?",
        confirmLabel: "Discard",
        run: teardown,
      };
    } else {
      teardown();
    }
  }

  // New-project templates: the empty one-module starter, then every bundled
  // example (its files scaffold onto disk when chosen). The launcher's New-project
  // flow lists these; picking one runs `newProject(name, id)`.
  const templates = [
    { id: "empty", name: "Empty project", description: "A single module to build on.", moduleCount: 1 },
    ...SAMPLES.map((s) => ({ id: s.id, name: s.name, description: s.description, moduleCount: s.moduleCount })),
  ];

  async function onformat() {
    if (!openFile || openFile.isOther) return; // companion files have no formatter
    // Markdown docs reflow via Prettier (lazy-loaded); `.pds` modules use the
    // wasm formatter. The manifest (raw TOML) has no formatter — leave it.
    if (openFile.isManifest) return;
    if (openFile.isDoc) {
      try {
        const [{ format }, markdownPlugin] = await Promise.all([
          import("prettier/standalone"),
          import("prettier/plugins/markdown"),
        ]);
        const formatted = await format(source, {
          parser: "markdown",
          plugins: [markdownPlugin.default ?? markdownPlugin],
        });
        onEditorChange(formatted);
      } catch {
        notify("info", "Cannot format Markdown — check the document");
      }
      return;
    }
    try {
      onEditorChange(formatSource(source));
    } catch {
      notify("info", "Cannot format — fix syntax errors first");
    }
  }

  const building = $derived(ui.building);
  const buildNotice = $derived(ui.buildNotice); // the blocking example-vs-folder modal

  // Build the static documentation site (the browser equivalent of `pds doc`).
  // An opened folder builds straight to disk; the bundled example first opens a
  // blocking notice explaining it can only be previewed.
  function onbuilddocs() {
    if (!ready || !workspace || building) return;
    if (workspace.root) {
      runBuild();
    } else {
      ui.buildNotice = true;
    }
  }

  // Confirmed from the modal: build the example as a read-only preview.
  function confirmPreviewBuild() {
    ui.buildNotice = false;
    runBuild();
  }
  // From the modal: open a real folder to build to disk instead.
  function openFolderFromNotice() {
    ui.buildNotice = false;
    openFolder();
  }

  // Assembles the doc-site config from the doc state loaded on open: site
  // name/theme from `[doc]`, plus the `[[doc.sidebar]]` pages with their live
  // (possibly edited) Markdown content. Degrades to name + theme when no docs.
  const buildDocConfig = () =>
    docs.buildDocConfig({ name: docMeta.name ?? workspace?.name ?? "", theme: docMeta.theme ?? "dark", docGroups, docSources });

  const sampleDocPages = (
    sidebar: { title: string; items?: { title: string; path: string }[] }[] | null | undefined,
    docMap: Record<string, string>,
  ) => docs.sampleDocPages(sidebar, docMap);

  // Renders the site, then writes it to `target/doc/` (opened folder) or opens a
  // preview (example), reporting the outcome as a notification.
  async function runBuild() {
    const ws = workspace;
    if (!ws) return;
    ui.building = true;
    try {
      const config = buildDocConfig();
      const files = renderDocSite(config);
      if (ws.root) {
        const dir = await writeSite(ws.root, files);
        notify(
          "success",
          "Documentation built",
          `Wrote ${files.length} files to ${dir}/ in “${ws.name}”. Open ${dir}/index.html to view it.`,
        );
      } else {
        previewSite(files);
        notify("success", `Preview built (${files.length} files)`, "Opened a read-only preview in a new tab.");
      }
    } catch (e) {
      notify("error", "Documentation build failed", String((e as Error)?.message ?? e));
    } finally {
      ui.building = false;
    }
  }

  // No folder to write to (the bundled sample): open a self-contained preview in
  // a new tab. The built site is multi-page with relative cross-links, which
  // can't resolve from a single blob — so the host embeds every file and renders
  // pages into an iframe, swapping the page (assets inlined) when an internal
  // link is clicked. External links open out; in-page anchors work natively.
  function previewSite(files: SiteFile[]) {
    const byPath = Object.fromEntries(files.map((f) => [f.path, f.contents]));
    const url = URL.createObjectURL(new Blob([buildPreviewHost(byPath)], { type: "text/html" }));
    window.open(url, "_blank");
    setTimeout(() => URL.revokeObjectURL(url), 60_000);
  }

  // A standalone HTML shell that previews the in-memory site: it holds every file
  // and an iframe, inlines each page's `style.css`/`client.js`, and intercepts
  // internal links to navigate within the preview instead of leaving it.
  function buildPreviewHost(byPath: Record<string, string>) {
    const map = JSON.stringify(byPath).replace(/<\/script>/g, "<\\/script>");
    return `<!doctype html><html><head><meta charset="utf-8">
<title>Documentation preview</title>
<style>html,body{margin:0;height:100%;background:#0a0b0e}iframe{border:0;width:100%;height:100vh;display:block}</style>
</head><body><iframe id="f"></iframe><script>
const FILES = ${map};
const f = document.getElementById('f');
let current = 'index.html';
const resolve = (from, href) => new URL(href, new URL(from, 'http://h/')).pathname.replace(/^\\//, '');
function inline(path, html){
  return html
    .replace(/<link\\b[^>]*href="([^"]+\\.css)"[^>]*>/g, (m,h)=>'<style>'+(FILES[resolve(path,h)]||'')+'</style>')
    .replace(/<script\\b[^>]*src="([^"]+\\.js)"[^>]*><\\/script>/g, (m,s)=>'<script>'+(FILES[resolve(path,s)]||'')+'<\\/script>');
}
function show(path){ const html = FILES[path]; if(html==null) return; current = path; f.srcdoc = inline(path, html); }
f.addEventListener('load', () => {
  const d = f.contentDocument; if(!d) return;
  d.addEventListener('click', (e) => {
    const a = e.target.closest && e.target.closest('a[href]'); if(!a) return;
    const href = a.getAttribute('href');
    if(/^(https?:|mailto:)/.test(href)){ a.target='_blank'; a.rel='noreferrer'; return; }
    if(href.startsWith('#')) return;
    const target = resolve(current, href.split('#')[0]);
    if(FILES[target]!=null){ e.preventDefault(); show(target); }
  });
});
show('index.html');
<\/script></body></html>`;
  }

  // ── Share / import / export (client-only codec) ──────────────────────────
  // Snapshot the live workspace (current edits, manifest, docs) into the codec's
  // serialisable shape. Shared by the URL-hash share (T6) and the file export (T7).
  const snapshotWorkspace = () =>
    share.snapshotWorkspace({
      name: workspace?.name ?? "shared-workspace",
      files: workspace?.files ?? [],
      moduleSources,
      manifestSource,
      docGroups,
      docSources,
    });

  // Mount a decoded workspace (from a share link or imported file) in-memory,
  // session-only until "Save to folder" — exactly the sample-load path.
  async function mountDecoded({ workspace: ws, landing }: MountInput) {
    wsStore.moduleSources = share.mountedSources(ws.files as { fqn?: string; source?: string }[]);
    saveStore.persisted = {}; // imported/shared: no on-disk baseline, session-only
    await mountWorkspace(ws, landing);
    recomputeWPayload(); // session-only: embed the workspace in the URL so refresh restores it
  }

  const busyShare = $derived(shareStore.busy);

  // Share: encode the live workspace, base64url it into the URL hash, and copy
  // the link. Over the size guard, fall back to a file export instead.
  async function onshare() {
    if (!workspace || busyShare) return;
    shareStore.busy = true;
    try {
      const bytes = await encodeWorkspace(snapshotWorkspace());
      if (bytes.length > MAX_HASH_BYTES) {
        notify("info", "Workspace too large to share by link", "Exported it as a file instead.");
        await onexport();
        return;
      }
      // A share link always embeds the workspace (the recipient has no disk) and
      // carries the current location so they land where you are. Built via the
      // router serializer; not written to our own address bar (which keeps its
      // folder-reference URL) unless the clipboard write fails.
      const hash = serializeRoute({
        base: { kind: "w", value: bytesToBase64Url(bytes) },
        ...currentLocationFields(),
      });
      const url = `${location.origin}${location.pathname}${hash}`;
      try {
        await navigator.clipboard.writeText(url);
        notify("success", "Share link copied to clipboard");
      } catch {
        notify("info", "Share link is in the address bar");
        // `window.` qualified: the component's nav `history` $state shadows the global.
        window.history.replaceState(null, "", url);
      }
    } catch (e) {
      notify("error", "Could not create share link", String((e as Error)?.message ?? e));
    } finally {
      shareStore.busy = false;
    }
  }

  // Export: download the gzipped workspace as `<name>.pdsx`.
  async function onexport() {
    if (!workspace) return;
    try {
      const bytes = await encodeWorkspace(snapshotWorkspace());
      const url = URL.createObjectURL(new Blob([bytes as BlobPart], { type: "application/octet-stream" }));
      const a = document.createElement("a");
      a.href = url;
      a.download = `${workspace.name || "workspace"}.pdsx`;
      a.click();
      setTimeout(() => URL.revokeObjectURL(url), 10_000);
      notify("success", `Exported ${a.download}`);
    } catch (e) {
      notify("error", "Could not export workspace", String((e as Error)?.message ?? e));
    }
  }

  // Import: pick a `.pdsx`, decode it, and mount in-memory (session-only).
  function onimport() {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".pdsx,application/octet-stream";
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      try {
        const bytes = new Uint8Array(await file.arrayBuffer());
        mountDecoded(await decodeWorkspace(bytes));
        notify("success", `Imported ${file.name}`);
      } catch (e) {
        notify("error", "Could not import workspace", String((e as Error)?.message ?? e));
      }
    };
    input.click();
  }

  // ── Hash router: the live session in the URL ─────────────────────────────
  // The URL hash holds the workspace base (a folder reference, or an embedded
  // in-memory workspace) plus the current location, so a refresh restores where
  // you were. `router` (router.svelte.ts) owns reading/writing `location.hash`.

  // Gate the live URL sync until the initial restore has settled, so the boot
  // sync doesn't clobber the restored caret before `pendingGoto` applies.
  let urlReady = $state(false);
  // The embedded base64 payload for an in-memory workspace, refreshed (debounced)
  // off edits so a refresh restores unsaved work. Empty for folder workspaces
  // (their content lives on disk) and when the workspace exceeds MAX_HASH_BYTES.
  let wPayload = $state("");
  let wPayloadTimer: ReturnType<typeof setTimeout> | undefined;
  let caretSyncTimer: ReturnType<typeof setTimeout> | undefined;

  // Re-encode the in-memory workspace into the URL base (debounced). A folder
  // workspace is a no-op — its base is a stable folder reference.
  function recomputeWPayload() {
    clearTimeout(wPayloadTimer);
    wPayloadTimer = setTimeout(async () => {
      if (!workspace || workspace.root) return;
      try {
        const bytes = await encodeWorkspace(snapshotWorkspace());
        wPayload = bytes.length > MAX_HASH_BYTES ? "" : bytesToBase64Url(bytes);
      } catch {
        wPayload = "";
      }
    }, 600);
  }

  // The current route base: a `f.<folder name>` reference for a disk-backed
  // project, or a `w.<payload>` embed for an in-memory session.
  function currentBase(): RouteBase {
    if (!workspace) return { kind: null, value: "" };
    if (workspace.root) return { kind: "f", value: workspace.name };
    return wPayload ? { kind: "w", value: wPayload } : { kind: null, value: "" };
  }

  // The current location fields (view / file / node / depth) shared by the live
  // URL sync and the share link, so both encode the same place. In space view the
  // 3D target is authoritative (a flow's entry callable isn't a placed node);
  // elsewhere the shared node selection is. Depth only applies to the canvas.
  function currentLocationFields(): Pick<Route, "view" | "file" | "node" | "depth"> {
    return {
      view,
      file: openFile?.fqn || undefined,
      node: view === "space" ? (spaceTargetFqn ?? undefined) : (selected?.fqn || undefined),
      depth: view === "canvas" ? seqDepth : undefined,
    };
  }

  // Mirror the live session into the hash. Reads the caret straight off the
  // editor (no cursor event to react to). No-op until the workspace base is ready.
  function syncUrl() {
    if (!urlReady) return;
    const base = currentBase();
    if (base.kind === null) return; // nothing addressable yet (no ws / payload pending)
    const loc = view === "code" ? editorApi?.location?.() : null;
    router.navigate({ base, ...currentLocationFields(), line: loc?.line, col: loc?.col });
  }

  // Capture the caret (and a freshly-embedded base) shortly after an edit.
  function scheduleCaretSync() {
    clearTimeout(caretSyncTimer);
    caretSyncTimer = setTimeout(syncUrl, 400);
  }

  // Reactive: re-sync on a view / file / selection / depth / payload change. The
  // dependency reads are explicit (the `void`s); the write itself runs untracked
  // so `router.navigate` reading its own `route` $state can't retrigger this effect.
  $effect(() => {
    void workspace;
    void view;
    void openFile;
    void selected;
    void spaceTargetFqn;
    void seqDepth;
    void wPayload;
    void urlReady;
    untrack(() => syncUrl());
  });

  // Apply the URL's location (view / file / node / caret / depth) over a freshly
  // mounted workspace, reusing the same store writes as `applyLocation`. Runs
  // after the mount settles, so it wins over the workspace's doc-landing default.
  function applySession(r: Route) {
    const ws = wsStore.workspace;
    if (!ws) return;
    if (r.file) {
      const file = ws.files.find((f) => f.fqn === r.file);
      if (file) selectFile(file);
    }
    if (r.depth) selection.seqDepth = r.depth as Depth;
    // Space view: drive the 3D target through applySpaceTarget so the graph flies
    // to the node / lights the flow on mount (mirrors applyLocation's space branch);
    // setting selection.selected alone leaves spaceFocus null and the camera resting.
    if (r.view === "space") {
      applySpaceTarget(r.node ?? null);
      if (r.node) selectNode(r.node, { goto: false, origin: false, record: false });
      return;
    }
    if (r.node) {
      const hit = nodeIndex.get(r.node);
      selection.selected = {
        fqn: r.node,
        line: hit?.node.line ?? r.line ?? 0,
        col: hit?.node.col ?? r.col ?? 0,
        fileFqn: hit?.fileFqn ?? r.file ?? "",
      };
    }
    selection.view = r.view; // after selectFile, which may force "code"
    if (r.view === "code" && r.line && r.file) {
      selection.pendingGoto = { line: r.line, col: r.col ?? 0, fileFqn: r.file };
    }
  }

  // Restore the session from the URL hash on first load: mount the workspace by
  // base kind, then apply the location. Returns false (→ launcher) when nothing
  // could be mounted (bare hash, missing folder handle, denied permission).
  async function restoreSession(): Promise<boolean> {
    let r = parseHash(location.hash);
    if (r.base.kind === null) {
      // Legacy `#w=<payload>` share links still boot.
      const legacy = share.parseHashPayload(location.hash);
      if (!legacy) return false;
      r = { ...r, base: { kind: "w", value: legacy } };
    }
    try {
      if (r.base.kind === "w") {
        const bytes = base64UrlToBytes(r.base.value) as Uint8Array<ArrayBuffer>;
        await mountDecoded(await decodeWorkspace(bytes));
      } else {
        const handle = await reopenFolder("folder:" + r.base.value);
        if (!handle) return false; // no stored handle / denied — fall back to launcher
        await adoptWorkspace(await readWorkspace(handle));
      }
    } catch (e) {
      notify("error", "Could not restore from the URL", String((e as Error)?.message ?? e));
      return false;
    }
    applySession(r);
    return true;
  }

</script>

<svelte:head><title>PseudoScript Web IDE</title></svelte:head>

<svelte:window
  onkeydown={(e) => {
    // Double-Shift opens Search Everywhere (IntelliJ): two bare Shift presses in
    // quick succession, with no other key between them. Any other key cancels the
    // pending first press, so Shift-as-a-modifier never triggers it.
    if (e.key === "Shift") {
      if (!e.repeat) {
        const now = Date.now();
        if (workspace && now - lastShift < 400) {
          ui.commandOpen = true;
          lastShift = 0;
        } else {
          lastShift = now;
        }
      }
      return;
    }
    lastShift = 0;
    if (e.key === "Escape") {
      if (buildNotice) ui.buildNotice = false;
      if (settingsOpen) ui.settingsOpen = false;
      if (projectOpen && workspace) ui.projectOpen = false;
      ui.canvasUsages = null;
    }
    // Cmd/Ctrl-S saves the active file (Cmd/Ctrl-Shift-S saves all) even when the
    // editor isn't focused (e.g. on the canvas). The editor's own keymap handles
    // the focused case; this prevents the browser's "save page" dialog regardless.
    if ((e.metaKey || e.ctrlKey) && !e.altKey && (e.key === "s" || e.key === "S")) {
      e.preventDefault();
      if (e.shiftKey) saveAll();
      else saveActiveFile();
    }
    // Cmd/Ctrl-K (legacy) and Cmd/Ctrl-P (VSCode-style Quick Open) both open the
    // search palette; P also suppresses the browser's print dialog.
    if ((e.metaKey || e.ctrlKey) && !e.altKey && (e.key === "k" || e.key === "K" || e.key === "p" || e.key === "P")) {
      e.preventDefault();
      if (workspace) ui.commandOpen = true;
    }
  }}
  oncontextmenu={(e) => {
    // App-wide: the browser's native context menu never shows. The IDE's own
    // menus (file tree, structure, canvas nodes) open from their own handlers,
    // which run on the target before this window-level fallback; everywhere else
    // right-click is simply suppressed so nothing breaks the app's surface.
    e.preventDefault();
  }}
  onfocus={reloadExternalChanges}
  onbeforeunload={(e) => {
    // Warn before closing with unsaved work that can be persisted to disk.
    if (canPersist && dirtyCount > 0) {
      e.preventDefault();
      e.returnValue = "";
    }
  }}
/>

{#if dialog}
  <PromptDialog
    title={dialog.title}
    label={dialog.label}
    placeholder={dialog.placeholder}
    value={dialog.value}
    confirmLabel={dialog.confirmLabel}
    hint={dialog.hint}
    validate={dialog.validate}
    onconfirm={(v: string) => {
      const run = dialog?.run;
      ui.dialog = null;
      run?.(v);
    }}
    oncancel={() => (ui.dialog = null)}
  />
{/if}

{#if confirmDialog}
  <ConfirmDialog
    title={confirmDialog.title}
    message={confirmDialog.message}
    confirmLabel={confirmDialog.confirmLabel ?? "Delete"}
    onconfirm={() => {
      const run = confirmDialog?.run;
      ui.confirmDialog = null;
      run?.();
    }}
    oncancel={() => (ui.confirmDialog = null)}
  />
{/if}

<Notifications notes={notifications.notes} ondismiss={dismissNote} />

{#if workspace}
  <CommandPalette
    bind:open={ui.commandOpen}
    files={workspace.files.filter((f) => f.fqn).map((f) => ({ fqn: f.fqn!, path: f.path ?? "" }))}
    symbols={symbols as never}
    modules={allModules}
    actions={paletteActions}
    onopenfile={(f) => {
      const real = workspace?.files.find((x) => x.fqn === f.fqn);
      if (real) selectFile(real);
    }}
    onpicksymbol={(fqn) => selectNode(fqn, { goto: true })}
    onopentext={openAtLine}
  />
{/if}

{#if ready && projectOpen}
  <ProjectPanel
    {recents}
    dismissible={!!workspace}
    onpickrecent={openRecent}
    onopenfolder={openFolder}
    onnewproject={() => (ui.newProjectOpen = true)}
    onforget={forgetRecent}
    onclose={() => (ui.projectOpen = false)}
  />
{/if}

{#if ready && newProjectOpen}
  <NewProjectDialog
    {templates}
    onchoosefolder={chooseProjectFolder}
    onpick={(name, id, parent) => { ui.newProjectOpen = false; newProject(name, id, parent); }}
    onclose={() => (ui.newProjectOpen = false)}
  />
{/if}

{#if settingsOpen}
  <Settings initialTab={settingsTab} onclose={() => ((ui.settingsOpen = false), (settingsTab = "keyboard"))} />
{/if}

<!-- Canvas usages: a click-away list of references; picking one jumps to it. -->
{#if canvasUsages}
  <button type="button" class="canvas-backdrop" aria-label="Dismiss usages" onclick={() => (ui.canvasUsages = null)}></button>
  <div class="canvas-pop usages" style="left:{canvasUsages.x + 14}px; top:{canvasUsages.y + 14}px">
    <div class="usages-head">{canvasUsages.items.length} usage{canvasUsages.items.length === 1 ? "" : "s"} of <code>{canvasUsages.name}</code></div>
    {#each canvasUsages.items as occ (occ.fqn + occ.line + occ.col)}
      <button type="button" class="usage" onclick={() => pickCanvasUsage(occ)}>
        <span class="loc">{occ.fqn}:{occ.line}</span>
        {#if occ.text}<span class="text">{occ.text}</span>{/if}
      </button>
    {/each}
  </div>
{/if}

{#if buildNotice}
  <BuildNoticeDialog
    onopenfolder={fsSupported ? openFolderFromNotice : undefined}
    onbuild={confirmPreviewBuild}
    oncancel={() => (ui.buildNotice = false)}
  />
{/if}

{#if renamePrompt}
  <RenameDialog
    symbol={renamePrompt.symbol}
    occurrences={renamePrompt.occurrences}
    onconfirm={applyRename}
    oncancel={() => (renamePrompt = null)}
  />
{/if}

<!-- One breadcrumb hop: the node's C4-kind icon + its simple name. -->
{#snippet crumbNode(fqn: string)}
  {@const node = nodeIndex.get(fqn)?.node}
  {@const Icon = node ? (KIND_ICON[node.kind] ?? Box) : Box}
  <span class="cn kind-{node?.kind ?? 'callable'}">
    <Icon size={13} strokeWidth={1.9} aria-hidden="true" />
    <span class="cn-name">{node?.name ?? fqn.split("::").at(-1)}</span>
  </span>
{/snippet}

<!-- The selected item's path: ▸node / ▸node / … — each ancestor a hop, the leaf
     the current scope. Empty when nothing is selected. -->
{#snippet breadcrumb()}
  {@const chain = selected ? ancestry(selected.fqn) : []}
  <div class="crumb">
    {#each chain as fqn, i (fqn)}
      {#if i > 0}<span class="sep">/</span>{/if}
      {#if i < chain.length - 1}
        <button class="hop" onclick={() => selectNode(fqn)}>{@render crumbNode(fqn)}</button>
      {:else}
        <span class="hop leaf">{@render crumbNode(fqn)}</span>
      {/if}
    {/each}
  </div>
{/snippet}

<!-- The Markdown syntax cheat-sheet button + popover (docs only). -->
{#snippet mdHelp()}
  <div class="md-help">
    <button
      class="md-help-btn"
      class:open={mdHelpOpen}
      title="Markdown syntax"
      aria-label="Markdown syntax"
      aria-expanded={mdHelpOpen}
      onclick={() => (ui.mdHelpOpen = !mdHelpOpen)}
    >?</button>
    {#if mdHelpOpen}
      <button class="md-help-scrim" aria-label="Close" onclick={() => (ui.mdHelpOpen = false)}></button>
      <div class="md-help-pop" role="dialog" aria-label="Markdown syntax">
        <div class="md-help-head">Markdown syntax</div>
        <ul>
          {#each MD_SYNTAX as row (row.name)}
            <li>
              <span class="md-help-name">{row.name}</span>
              <code>{row.syntax}</code>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
  </div>
{/snippet}

<!-- The Markdown reading-width selector (docs only). -->
{#snippet docWidthToggle()}
  <div class="view-toggle" role="group" aria-label="Document width">
    {#each [["narrow", "Narrow"], ["wide", "Wide"], ["full", "Full"]] as [val, label] (val)}
      <button class:active={docWidth === val} onclick={() => setDocWidth(val)} title="{label} width">{label}</button>
    {/each}
  </div>
{/snippet}

<div class="ide">
  <TopBar
    workspaceName={workspace?.name ?? null}
    {building}
    {view}
    {structureOpen}
    {canBack}
    {canForward}
    onback={goBack}
    onforward={goForward}
    onopenfolder={() => (ui.projectOpen = true)}
    oncloseproject={closeProject}
    ongoto={() => (ui.commandOpen = true)}
    onnewfile={startNewFile}
    onnewdoc={startNewDoc}
    onsave={saveActiveFile}
    onsaveall={saveAll}
    {onshare}
    {onexport}
    {onimport}
    {onbuilddocs}
    onshortcuts={() => (ui.settingsOpen = true)}
    onaisettings={() => ((settingsTab = "ai"), (ui.settingsOpen = true))}
    onview={(v) => (selection.view = v)}
    ontogglestructure={() => (ui.structureOpen = !ui.structureOpen)}
  />

  <div
    class="body"
    class:loaded={ready && !!workspace && !wasmError && fsSupported}
    style="--explorer-w:{panelSizes.explorerW}px; --structure-w:{panelSizes.structureW}px; --problems-h:{panelSizes.problemsH}px; --explorer-track:{view === 'code' && explorerOpen ? panelSizes.explorerW + 'px' : '0px'}; --structure-track:{structureOpen ? panelSizes.structureW + 'px' : '0px'}; --problems-track:{problemsOpen ? panelSizes.problemsH + 'px' : '0px'}"
  >
    <ActivityBar
      active={view === "canvas" ? "canvas" : view === "space" ? "space" : "explorer"}
      {explorerOpen}
      {problemsOpen}
      problemCount={problems.length}
      {errorCount}
      onexplorer={() => {
        if (view !== "code") {
          selection.view = "code";
          ui.explorerOpen = true;
        } else ui.explorerOpen = !ui.explorerOpen;
      }}
      oncanvas={() => (selection.view = "canvas")}
      onspace={() => (selection.view = "space")}
      ontoggleproblems={() => (ui.problemsOpen = !ui.problemsOpen)}
    />

    {#if wasmError}
      <div class="curtain span">
        <div class="kicker">compiler failed to load</div>
        <p class="msg">{wasmError}</p>
        <button class="retry" onclick={boot}>Retry</button>
      </div>
    {:else if !fsSupported}
      <div class="curtain span">
        <div class="kicker">browser not supported</div>
        <p class="msg">
          The PseudoScript IDE reads and writes your project as real files on disk, which needs the File
          System Access API. That's available in Chromium browsers — Chrome, Edge, Brave, Arc. Firefox and
          Safari don't support it yet.
        </p>
      </div>
    {:else if ready && workspace}
      {#if view === "code" && explorerOpen}
      <section class="explorer island reveal r1">
        <FileTree
          entries={fileEntries}
          dirs={workspace.dirs ?? []}
          {openKey}
          {errorKeys}
          dirtyKeys={dirty}
          onopen={openEntry}
          oncreatefile={startNewFile}
          oncreatedoc={startNewDoc}
          oncreatefolder={startNewFolder}
          onrenamefolder={startRenameFolder}
          ondeletefolder={deleteFolder}
          onrenamefile={(fqn) => {
            const f = moduleByFqn(fqn);
            if (f) startRenameFile(f);
          }}
          onmovefile={({ fqn, destDir }) => {
            const f = moduleByFqn(fqn);
            if (f) moveFile({ file: f, destDir });
          }}
          ondeletefile={(fqn) => {
            const f = moduleByFqn(fqn);
            if (f) requestDeleteFile(f);
          }}
          onrefresh={reloadExternalChanges}
        />
      </section>
      <Splitter
        side="left"
        width={panelSizes.explorerW}
        min={PANEL_MIN}
        max={PANEL_MAX}
        label="Resize explorer"
        onresize={(px) => panelSizes.setExplorerW(px)}
        onreset={() => panelSizes.resetExplorer()}
      />
      {/if}

      <section class="center island reveal r2">
        {#if openFile?.isDoc}
          <header class="content-bar">
            <div class="bar-actions">{@render mdHelp()}{@render docWidthToggle()}</div>
          </header>
        {/if}
        <div class="content-body">
          <div class="layer code-layer" class:hidden={view !== "code"} data-doc-width={docWidth}>
            {#if tabList.length}
              <TabBar
                tabs={tabList}
                onselect={selectTab}
                onclose={closeTab}
                oncloseothers={closeOthers}
                onclosetoright={closeToRight}
                oncloseall={closeAll}
                onreorder={reorderTabs}
              />
            {/if}
            {#if openFile?.isManifest && manifestError}
              <div class="manifest-error" role="status">
                <span class="me-kicker">manifest error</span>
                <span class="me-msg">{manifestError}</span>
              </div>
            {/if}
            {#if openFile?.isManifest && manifestHasDeps}
              <div class="manifest-note" role="note">
                <code>[dependencies]</code> are resolved by <code>pds install</code> (CLI) — editing them here won't fetch or update them.
              </div>
            {/if}
            {#if openFile?.isOther && openFile.binary}
              <div class="binary-pane" role="note">
                <File size={28} strokeWidth={1.6} aria-hidden="true" />
                <p class="bp-name">{openFile.title}</p>
                <p class="bp-meta">Binary file{openFile.bytes != null ? ` · ${formatBytes(openFile.bytes)}` : ""} — not shown in the editor.</p>
              </div>
            {:else}
              <Editor
                value={source}
                onchange={onEditorChange}
                onready={(api) => (editorApi = api)}
                moduleFqn={openFile?.fqn ?? ""}
                diagnostics={editorDiagnostics}
                fileKey={openKey}
                plain={(openFile?.isDoc || openFile?.isManifest || openFile?.isOther) ?? false}
                markdown={openFile?.isDoc ?? false}
                toml={openFile?.isManifest ?? false}
                filename={openFile?.isOther ? (openFile?.path ?? "") : ""}
                {previewOpts}
                {symbols}
                onopensymbol={revealSymbol}
                ongotodefinition={(fqn) => selectNode(fqn, { goto: true })}
                onnavigate={openUsage}
                onrename={requestRename}
                {onformat}
                onsave={saveActiveFile}
                onopensettings={() => (ui.settingsOpen = true)}
              />
            {/if}
          </div>
          {#if view === "canvas"}
            <div class="layer canvas-layer" data-testid="canvas-view">
              <DiagramPane scene={canvas.scene} layout={canvas.layout} error={canvas.error} hint={canvasHint} onpick={pickNode} onup={navigateUp} flows={flowsByNode} depth={seqDepth} ondepth={(d: Depth) => (selection.seqDepth = d)} onusages={showCanvasUsages} onsource={openNodeInEditor} typeFqn={typeFqnByName as never} tweaks={canvasTweaks} onlayoutchange={(t) => ui.setLayoutTweaks(t)} unlocked={pinStore.unlocked} onpin={pinNode} onunlock={toggleUnlock} pinnedFqns={pinnedFqns} onunpin={unpinNode} onresetgrid={resetCurrentView} onuniverse={openUniverse} />
            </div>
          {:else if view === "space"}
            <div class="layer space-layer" data-testid="space-view">
              {#if spaceSnapshot}
                {#key spaceKey}
                  <ForceGraph snapshot={spaceSnapshot} flows={spaceFlows} focusFqn={spaceFocus} highlightPath={spacePath} flowSequence={spaceFlow} flowColor={spaceFlowColor} flowName={spaceFlowName} ondeselect={resetSpace} onpick={openUniverse} />
                {/key}
              {:else}
                <div class="note"><span class="kicker">3d graph</span><p>Building the relationship graph…</p></div>
              {/if}
            </div>
          {/if}
        </div>
      </section>

      {#if structureOpen}
        <Splitter
          side="right"
          width={panelSizes.structureW}
          min={PANEL_MIN}
          max={PANEL_MAX}
          label="Resize structure panel"
          onresize={(px) => panelSizes.setStructureW(px)}
          onreset={() => panelSizes.resetStructure()}
        />
        <StructurePanel
          symbols={symbols as never}
          selectedFqn={selected?.fqn ?? null}
          onpicknode={(fqn) => (view === "space" ? openUniverse(fqn) : selectNode(fqn, { goto: true }))}
          ongotodef={(fqn) => selectNode(fqn, { goto: true })}
          onreveal={revealSymbol}
          onshowuniverse={showInUniverse}
        />
      {/if}

      <RightRail {structureOpen} ontogglestructure={() => (ui.structureOpen = !ui.structureOpen)} />

      {#if problemsOpen}
        <Splitter
          side="bottom"
          width={panelSizes.problemsH}
          min={PROBLEMS_MIN}
          max={PROBLEMS_MAX}
          label="Resize problems"
          onresize={(px) => panelSizes.setProblemsH(px)}
          onreset={() => panelSizes.resetProblems()}
        />
        <BottomDock
          {problems}
          onpick={onProblemPick}
          oncopy={onProblemsCopy}
          oncollapse={() => (ui.problemsOpen = false)}
        />
      {/if}
    {:else if ready}
      <div class="stage-empty span"></div>
    {:else}
      <div class="curtain span">
        <div class="loader"><span class="bar"></span></div>
        <div class="kicker">compiling the compiler…</div>
      </div>
    {/if}
  </div>

  <StatusBar>
    {#if ready && workspace}{@render breadcrumb()}{/if}
    {#if llm.enabled}
      <button
        class="ai-chip"
        class:dim={!llm.ready || llm.lastDropReason !== null}
        class:err={llm.lastError !== null}
        data-testid="llm-status"
        title={llm.lastError
          ? `AI completion failing — ${llm.lastError.message}. Click to fix.`
          : llm.lastDropReason
            ? llm.lastDropReason === "invalid"
              ? `AI suggestion arrived but wasn't valid PseudoScript, so it was dropped — a stronger model than ${llm.model} helps.`
              : "AI completion answered with nothing to insert."
            : llm.ready
              ? `AI completion on — ${llm.model}`
              : "AI completion enabled but not fully configured — click to set it up"}
        onclick={() => ((settingsTab = "ai"), (ui.settingsOpen = true))}>AI</button
      >
    {/if}
    <PerfMeter />
  </StatusBar>
</div>


<style>
  /* The island shell: top bar / body / status, with a fixed activity rail and
     collapsible explorer + structure islands flanking the centre. */
  .ide {
    display: grid;
    grid-template-rows: var(--topbar-h) minmax(0, 1fr) var(--status-h);
    height: 100vh;
  }
  .body {
    position: relative;
    display: grid;
    grid-template-columns: var(--activity-w) minmax(0, 1fr);
    gap: var(--island-gap);
    /* Only flank the islands left/right. The header and footer centre their text,
       which already supplies the vertical breathing room; a top/bottom gap here
       would stack on top of that and read as too much space. */
    padding: 0 var(--island-gap);
    min-height: 0;
    background: var(--bg);
  }
  /* Tool-window grid: left rail | explorer | centre | structure | right rail,
     over a content row and a bottom problems-dock row. Each collapsible track
     (--explorer-track / --structure-track / --problems-track) is set inline on
     .body and falls to 0 when its island is closed, so one rule covers every
     open/closed combination. The two rails carry no chrome (see ActivityBar /
     RightRail) and span both rows. */
  .body.loaded {
    grid-template-columns: var(--activity-w) var(--explorer-track) minmax(0, 1fr) var(--structure-track) var(--right-rail-w);
    grid-template-rows: minmax(0, 1fr) var(--problems-track);
  }
  .body.loaded :global(.activity) {
    grid-column: 1;
    grid-row: 1 / -1;
  }
  .body.loaded .explorer {
    grid-column: 2;
    grid-row: 1;
  }
  .body.loaded .center {
    grid-column: 3;
    grid-row: 1;
  }
  .body.loaded :global(.structure) {
    grid-column: 4;
    grid-row: 1;
  }
  .body.loaded :global(.rail) {
    grid-column: 5;
    grid-row: 1 / -1;
  }
  .body.loaded :global(.dock) {
    grid-column: 2 / 5;
    grid-row: 2;
  }
  /* a curtain / empty stage spans everything right of the activity rail */
  .span {
    grid-column: 2 / -1;
  }
  /* the backdrop behind the project panel when no workspace is loaded yet */
  .stage-empty {
    min-height: 0;
    background-image:
      linear-gradient(var(--grid) 1px, transparent 1px),
      linear-gradient(90deg, var(--grid) 1px, transparent 1px);
    background-size: 30px 30px, 30px 30px;
  }
  .explorer {
    min-width: 0;
    min-height: 0;
    background: var(--island-bg);
  }
  /* the centre: a slim content-bar over the editor / canvas */
  .center {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    background: var(--island-bg);
  }
  .content-bar {
    grid-row: 1;
    display: flex;
    align-items: center;
    gap: 0.6rem;
    height: var(--bar-h);
    padding: 0 0.6rem;
    border-bottom: 1px solid var(--line);
  }
  .nav-buttons { flex: none; display: flex; gap: 0.2rem; }
  .nav-btn {
    width: 1.7rem;
    height: 1.7rem;
    display: grid;
    place-items: center;
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    color: var(--ink-soft);
    font-size: 0.85rem;
    line-height: 1;
  }
  .nav-btn:hover:not(:disabled) { border-color: var(--accent); color: var(--ink); }
  .nav-btn:disabled { opacity: 0.35; cursor: not-allowed; }

  .content-body {
    grid-row: 2;
    display: grid;
    min-height: 0;
  }
  /* the active views share one grid cell; the editor stays mounted and is
     hidden (not destroyed) when another view is shown, preserving its state. */
  .layer {
    grid-area: 1 / 1;
    min-width: 0;
    min-height: 0;
  }
  .layer.hidden { display: none; }
  /* the code layer stacks any manifest banners above the editor, which flexes
     to fill the rest. */
  .code-layer { display: flex; flex-direction: column; }
  .code-layer :global(.editor) { flex: 1; min-height: 0; }

  .binary-pane {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    color: var(--ink-faint);
    text-align: center;
    padding: 1.5rem;
  }
  .binary-pane .bp-name {
    margin: 0.2rem 0 0;
    font-family: var(--font-mono);
    font-size: 0.84rem;
    color: var(--ink-soft);
  }
  .binary-pane .bp-meta {
    margin: 0;
    font-size: 0.76rem;
  }

  .manifest-error,
  .manifest-note {
    flex: none;
    padding: 0.5rem 0.9rem;
    font-size: 0.78rem;
    line-height: 1.5;
    border-bottom: 1px solid var(--line);
  }
  .manifest-error {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    background: color-mix(in srgb, var(--err) 12%, transparent);
    color: var(--ink);
  }
  .manifest-error .me-kicker {
    font-family: var(--font-mono);
    font-size: 0.6rem;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--err);
  }
  .manifest-error .me-msg { font-family: var(--font-mono); font-size: 0.74rem; color: var(--ink-soft); }
  .manifest-note { background: var(--surface-2); color: var(--ink-soft); }
  .manifest-note code {
    font-family: var(--font-mono);
    font-size: 0.78em;
    color: var(--ink);
    background: var(--surface-3);
    padding: 0.05rem 0.3rem;
    border-radius: var(--radius-sm);
  }
  .canvas-layer {
    background: var(--island-bg);
  }
  /* positioned so ForceGraph's absolutely-filled canvas resolves to this cell */
  .space-layer {
    position: relative;
    background: var(--island-bg);
    display: grid;
    place-items: center;
  }
  .space-layer .note { max-width: 30rem; text-align: center; color: var(--ink-soft); }
  .space-layer .note .kicker {
    display: inline-block;
    font-family: var(--font-mono);
    font-size: 0.6rem;
    font-weight: 600;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--ink-faint);
    margin-bottom: 0.6rem;
  }
  .space-layer .note p { margin: 0; font-family: var(--font-mono); font-size: 0.82rem; }

  /* CODE | CANVAS | Problems toggle */
  .bar-actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .md-help {
    position: relative;
    flex: none;
  }
  .md-help-btn {
    width: 1.55rem;
    height: 1.55rem;
    display: grid;
    place-items: center;
    border-radius: 999px;
    border: 1px solid var(--line-strong);
    background: var(--surface-2);
    color: var(--ink-soft);
    font-family: var(--font-mono);
    font-size: 0.78rem;
    font-weight: 700;
    cursor: pointer;
    transition: color 0.12s, border-color 0.12s, background 0.12s;
  }
  .md-help-btn:hover,
  .md-help-btn.open {
    color: var(--accent);
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .md-help-scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: transparent;
    border: none;
    cursor: default;
  }
  .md-help-pop {
    position: absolute;
    top: calc(100% + 0.4rem);
    right: 0;
    z-index: 41;
    width: 22rem;
    max-height: 70vh;
    overflow-y: auto;
    padding: 0.5rem;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg);
    scrollbar-width: thin;
  }
  .md-help-head {
    padding: 0.25rem 0.45rem 0.5rem;
    font-family: var(--font-mono);
    font-size: 0.62rem;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .md-help-pop ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .md-help-pop li {
    display: grid;
    grid-template-columns: 7rem 1fr;
    gap: 0.6rem;
    align-items: start;
    padding: 0.3rem 0.45rem;
    border-top: 1px solid var(--line);
  }
  .md-help-pop li:first-child {
    border-top: none;
  }
  .md-help-name {
    font-size: 0.78rem;
    color: var(--ink-soft);
    padding-top: 0.1rem;
  }
  .md-help-pop code {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: var(--ink);
    white-space: pre-wrap;
    word-break: break-word;
  }
  /* The AI-completion status chip: present while the feature is enabled, dimmed
     when it still lacks an endpoint/model; opens Settings to fix either. */
  .ai-chip {
    margin-left: auto;
    flex: none;
    background: transparent;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    color: var(--accent-hi);
    font-family: var(--font-mono);
    font-size: 0.6rem;
    letter-spacing: 0.1em;
    padding: 0.05rem 0.35rem;
    cursor: pointer;
  }
  .ai-chip:hover {
    border-color: var(--accent);
  }
  .ai-chip.dim {
    color: var(--warn);
  }
  .ai-chip.err {
    color: var(--err);
    border-color: var(--err);
  }
  .crumb {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    /* Indent so the leading kind-icon centres under the activity rail's icon
       column (rail is --activity-w wide; the status bar spans full width). */
    padding-left: 0.7rem;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: var(--ink-soft);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .crumb .sep { color: var(--ink-faint); }
  .crumb .hop {
    background: transparent;
    border: none;
    padding: 0;
    font: inherit;
    color: var(--ink-soft);
    cursor: pointer;
  }
  /* a hop: kind icon (coloured by C4 level) + name; the leaf isn't a button */
  .crumb :global(.cn) {
    display: inline-flex;
    align-items: center;
    gap: 0.28rem;
  }
  .crumb .cn-name { color: var(--ink-soft); }
  .crumb .hop.leaf .cn-name { color: var(--ink); }
  .crumb .hop:hover .cn-name { color: var(--ink); }
  .crumb :global(.cn.kind-person) { color: #6e8bff; }
  .crumb :global(.cn.kind-system) { color: var(--accent-hi); }
  .crumb :global(.cn.kind-container) { color: #2dd4bf; }
  .crumb :global(.cn.kind-component) { color: #b87bf5; }
  .crumb :global(.cn.kind-data) { color: var(--warn); }
  .crumb :global(.cn.kind-callable) { color: var(--ink-faint); }
  .crumb :global(.cn.kind-feature) { color: #34d399; }

  /* one orchestrated staggered reveal on load */
  .reveal { animation: rise 0.5s cubic-bezier(0.2, 0.7, 0.2, 1) both; }
  .r1 { animation-delay: 0.02s; }
  .r2 { animation-delay: 0.09s; }

  .count {
    font-size: 0.62rem;
    background: var(--surface-3);
    color: var(--ink-soft);
    padding: 0.05rem 0.4rem;
    border-radius: 999px;
  }
  .count.bad { background: color-mix(in srgb, var(--err) 18%, transparent); color: var(--err); }

  .curtain {
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 0.9rem;
    text-align: center;
  }
  .curtain .kicker {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .curtain .msg { font-family: var(--font-mono); font-size: 0.82rem; color: var(--err); max-width: 32rem; }
  .loader {
    width: 180px;
    height: 3px;
    background: var(--surface-2);
    border-radius: 2px;
    overflow: hidden;
  }
  .loader .bar {
    display: block;
    width: 40%;
    height: 100%;
    background: var(--accent);
    animation: sweep 1.1s ease-in-out infinite;
  }
  .retry {
    color: var(--accent-ink);
    background: var(--accent);
    border: none;
    border-radius: var(--radius-sm);
    padding: 0.5rem 1.1rem;
    font-weight: 700;
  }

  /* canvas find-usages popover, anchored at the pointer */
  .canvas-backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    border: 0;
    padding: 0;
    background: transparent;
    cursor: default;
  }
  .canvas-pop {
    position: fixed;
    z-index: 61;
    max-width: 26rem;
    padding: 0.6rem 0.8rem;
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-md);
    pointer-events: auto;
  }
  .canvas-pop .usages-head {
    font-size: 0.72rem;
    color: var(--ink-soft);
    margin-bottom: 0.4rem;
  }
  .canvas-pop .usages-head code { font-family: var(--font-mono); color: var(--ink); }
  .canvas-pop .usage {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    width: 100%;
    text-align: left;
    padding: 0.3rem 0.4rem;
    background: transparent;
    border: 0;
    border-radius: 4px;
    cursor: pointer;
  }
  .canvas-pop .usage:hover { background: color-mix(in srgb, var(--accent) 14%, transparent); }
  .canvas-pop .usage .loc { font-family: var(--font-mono); font-size: 0.7rem; color: var(--accent); }
  .canvas-pop .usage .text { font-family: var(--font-mono); font-size: 0.72rem; color: var(--ink-soft); }
</style>
