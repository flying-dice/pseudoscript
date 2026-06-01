<script lang="ts">
  import { onMount, tick } from "svelte";
  import { dev } from "$app/environment";
  import { base } from "$app/paths";
  import "../app.css";
  import { checkModules, docManifest, emitSceneModules, format as formatSource, hover, layoutScene, outlineModules, references, renderDocSite, symbolScene } from "$lib/pds.js";
  import type { Module, Occurrence } from "$lib/pds.js";
  import { fsSupported, scaffoldWorkspace, emptySeed, openWorkspace, readWorkspace, readDocPages, readFile, writeFile, writeSite, resolveDocAsset, fqnOf, createFile, movePath, deletePath, serializeManifest } from "$lib/workspace.js";
  import type { Workspace, WorkspaceFile, SiteFile } from "$lib/workspace.js";
  import type { Depth } from "$lib/sequence.js";
  import { reportError } from "$lib/errors.js";
  import { SAMPLES, sampleSeed } from "$lib/samples.js";
  import { getRecents, recordFolder, reopenFolder, forget } from "$lib/recents.js";
  import type { Recent } from "$lib/recents.js";
  import { encodeWorkspace, decodeWorkspace, bytesToBase64Url, base64UrlToBytes, MAX_HASH_BYTES } from "$lib/codec.js";
  import type { MountableWorkspace } from "$lib/codec.js";
  import { theme } from "$lib/theme.svelte.js";
  import * as nav from "$lib/core/navigation.js";
  import { computeDiagnostics } from "$lib/core/diagnostics.js";
  import * as ops from "$lib/core/workspace-ops.js";
  import { keyOf, computeDirty, dirtyPaths as dirtyPathsOf, seedBaseline as advanceBaseline, classifyReload } from "$lib/core/dirty.js";
  import * as share from "$lib/core/share.js";
  import * as docs from "$lib/core/docs.js";
  import * as model from "$lib/core/model.js";
  import { projectCanvas, canvasHint as canvasHintOf } from "$lib/core/canvas.js";
  import { notifications } from "$lib/stores/notifications.svelte.js";
  import { wasm } from "$lib/stores/wasm.svelte.js";
  import Editor from "$lib/components/Editor.svelte";
  import Toolbar from "$lib/components/Toolbar.svelte";
  import FileTree from "$lib/components/FileTree.svelte";
  import DiagramPane from "$lib/components/DiagramPane.svelte";
  import ProblemsPane from "$lib/components/ProblemsPane.svelte";
  import Notifications from "$lib/components/Notifications.svelte";
  import ProjectPanel from "$lib/components/ProjectPanel.svelte";
  import Settings from "$lib/components/Settings.svelte";
  import PromptDialog from "$lib/components/PromptDialog.svelte";

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
    CanvasInfo,
    CanvasUsages,
    Dialog,
    ConfirmDialog,
    NoteKind,
    PendingWrite,
  } from "$lib/core/types.js";

  // The in-memory mount payload `mountWorkspace` consumes (sample / decoded).
  type MountInput = { workspace: PageWorkspace; landing?: string | null };

  // WASM readiness / version / init error — owned by the wasm store; the page
  // reads them through these derived aliases (keeping every call site unchanged).
  const ready = $derived(wasm.ready);
  const wasmError = $derived(wasm.error);
  const ver = $derived(wasm.version);
  let editorApi = $state<EditorApi | null>(null);

  // The selected item's view: its source ("code"), its interactive diagram
  // ("canvas"), or the workspace problem list ("problems"). The nav stays put;
  // only this content pane swaps.
  let view = $state<"code" | "canvas" | "problems">("code");

  // The C4 depth a sequence diagram is collapsed to (persons & systems /
  // + containers / + components). Components = full detail (no collapse).
  let seqDepth = $state<Depth>("component");

  // The structure node selected in the nav / drilled into on the canvas, as
  // { fqn, line, col, fileFqn }, or null for the whole-model scope. Drives the
  // canvas diagram, the breadcrumb, and which nav row is highlighted.
  let selected = $state<{ fqn: string; line: number; col: number; fileFqn: string } | null>(null);
  // A queued editor jump (set when a node is picked); applied once the code view
  // is mounted and showing the node's file.
  let pendingGoto = $state<{ line: number; col: number; fileFqn: string } | null>(null);

  // Navigation history: every code location jumped to (go-to-definition, a nav
  // pick, a problem, a find-usages hit), as { fileFqn, line, col, label, fqn? }.
  // `histIndex` is the current position; back/forward step through it without
  // recording. New jumps truncate the forward tail (browser-history semantics).
  let history = $state<Loc[]>([]);
  let histIndex = $state(-1);
  const canBack = $derived(nav.canBack({ history, index: histIndex }));
  const canForward = $derived(nav.canForward({ history, index: histIndex }));

  // Record a visited location (forward-tail truncation, repeat collapse, cap).
  function recordLocation(loc: Loc) {
    const next = nav.recordLocation({ history, index: histIndex }, loc);
    history = next.history;
    histIndex = next.index;
  }

  // Before a jump, record where the caret currently is so Back returns to the
  // starting point. Skips when the caret already sits at the history cursor.
  function recordOrigin() {
    const loc = editorApi?.location?.();
    if (!loc || !openFile?.fqn) return;
    const here = nav.originLoc(openFile.fqn, loc.line, loc.col);
    if (nav.samePosition(history[histIndex], here)) return;
    recordLocation(here);
  }

  // Apply a location without recording it (back/forward, history-list click):
  // open its file, jump the editor there, and re-scope to its node when it has one.
  function applyLocation(loc: Loc) {
    const file = workspace?.files.find((f) => f.fqn === loc.fileFqn);
    if (!file) return;
    if (openFile?.fqn !== file.fqn) {
      flushSave();
      openFile = file;
    }
    if (loc.fqn) selected = { fqn: loc.fqn, line: loc.line, col: loc.col, fileFqn: loc.fileFqn };
    view = "code";
    pendingGoto = { line: loc.line, col: loc.col, fileFqn: loc.fileFqn };
  }

  function goBack() {
    const step = nav.stepBack({ history, index: histIndex });
    if (!step) return;
    histIndex = step.state.index;
    applyLocation(step.loc);
  }
  function goForward() {
    const step = nav.stepForward({ history, index: histIndex });
    if (!step) return;
    histIndex = step.state.index;
    applyLocation(step.loc);
  }

  // Open a find-usages occurrence: jump to it and record it in history.
  function openUsage(occ: Occurrence) {
    recordOrigin();
    applyLocation({ fileFqn: occ.fqn, line: occ.line, col: occ.col });
    recordLocation({ fileFqn: occ.fqn, line: occ.line, col: occ.col, label: occ.text || `${occ.fqn}:${occ.line}` });
  }

  // Workspace state. Defaults to the bundled sample (in-memory, handles null);
  // "Open folder" swaps in a real on-disk workspace whose files persist on edit.
  let workspace = $state<PageWorkspace | null>(null);
  let openFile = $state<OpenFile | null>(null);
  let moduleSources = $state<Record<string, string>>({});
  // Authored docs (`[[doc.sidebar]]`): the sidebar groups (`{ title, items:
  // [{ title, path, handle }] }`), each page's live Markdown by path, and the
  // `{ name, theme }` parsed from `[doc]` for the site build. Loaded on open.
  let docGroups = $state<LiveDocGroup[]>([]);
  let docSources = $state<Record<string, string>>({});
  let docMeta = $state<{ name?: string; theme?: string }>({});

  // The raw `pds.toml` text, editable as a first-class file. Keyed in the dirty
  // baseline by the manifest path. Re-resolved (doc nav / name / theme) on save.
  let manifestSource = $state("");
  // The last manifest parse error, shown inline above the editor when the open
  // file is the manifest (the IDE keeps the last good doc nav meanwhile).
  let manifestError = $state<string | null>(null);
  // Whether the live manifest declares a `[dependencies]` table — drives the
  // read-only "resolved by pds install" note.
  const manifestHasDeps = $derived(docs.manifestHasDeps(manifestSource));

  // The persisted baseline: the text last read from / written to disk, keyed the
  // same way as the live buffers (FQN for modules, path for docs). A file is
  // "dirty" when its live buffer differs from this baseline. Bundled samples have
  // no handle and never enter this map — they're session-only, not dirty.
  let persisted = $state<Record<string, string>>({});
  // The save lifecycle of the active file, for the status cue: idle | saving |
  // saved | error. `saved` shows briefly after a successful write.
  let saveState = $state<"idle" | "saving" | "saved" | "error">("idle");
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
  const dirty = $derived(computeDirty(persisted, { manifestKey, manifestSource, moduleSources, docSources }));
  const dirtyCount = $derived(dirty.size);

  // Dirty keys mapped to their tree paths, for the FileTree row dot.
  const dirtyPaths = $derived(workspace ? dirtyPathsOf(dirty, workspace.files) : new Set<string>());

  // The Markdown reading width (narrow | wide | full), persisted across sessions.
  let docWidth = $state(loadDocWidth());
  function loadDocWidth(): string {
    try {
      return localStorage.getItem("pds-doc-width") || "narrow";
    } catch {
      return "narrow";
    }
  }
  function setDocWidth(w: string) {
    docWidth = w;
    try {
      localStorage.setItem("pds-doc-width", w);
    } catch {
      /* storage unavailable — session-only */
    }
  }

  // The Markdown syntax cheat-sheet shown from the doc toolbar's "?" button —
  // every flavour the live preview and `pds doc` render.
  let mdHelpOpen = $state(false);
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
  let projectOpen = $state(false);
  // Whether the keyboard-shortcuts settings modal is open (toolbar gear or the
  // bound shortcut). Shell-owned so it's reachable with or without a file open.
  let settingsOpen = $state(false);
  let recents = $state<Recent[]>([]);
  // Only persisted projects (folders) are recents; in-memory samples re-open
  // from the catalogue, so they're never recorded — and legacy sample entries
  // are filtered out of the list.
  const refreshRecents = () => (recents = getRecents().filter((r) => r.kind !== "sample"));

  const source = $derived(
    openFile?.isManifest
      ? manifestSource
      : openFile?.isDoc
        ? (docSources[openFile.path ?? ""] ?? "")
        : openFile
          ? (moduleSources[openFile.fqn ?? ""] ?? "")
          : "",
  );

  // Every module as {fqn, source} — diagrams and the target list span the whole
  // workspace (cross-module containers/components/edges), not just the open file.
  const allModules = $derived<Module[]>(
    workspace ? workspace.files.map((f) => ({ fqn: f.fqn ?? "", source: moduleSources[f.fqn ?? ""] ?? "" })) : [],
  );

  // Static-check the whole workspace; the view reads its problems/errors/paths.
  const diag = $derived.by(() =>
    ready && workspace
      ? computeDiagnostics(allModules, workspace.files, checkModules)
      : { results: null, problems: [], errorCount: 0, errorPaths: new Set<string>() },
  );
  const problems = $derived(diag.problems);
  const errorCount = $derived(diag.errorCount);
  const errorPaths = $derived(diag.errorPaths);

  const nodes = $derived.by<StructureNode[]>(() => {
    if (!ready || !workspace) return [];
    try {
      return outlineModules(allModules) as unknown as StructureNode[];
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
  const flowsByNode = $derived(index.flowsByNode);
  const nodeInfo = $derived(index.nodeInfo);
  const typeFqnByName = $derived(index.typeFqnByName);

  // The CANVAS diagram: the selected node's fitting view, or the whole-model
  // context overview when nothing is selected. The compiler picks the view; a
  // sequence scene is then collapsed to the chosen depth in the IDE.
  const canvas = $derived(
    ready && workspace
      ? projectCanvas({ selected, seqDepth, modules: allModules, index, wasm: { symbolScene, emitSceneModules, layoutScene }, onError: reportError })
      : { scene: null, error: "" },
  );
  const canvasHint = $derived(canvasHintOf(selected));

  // Canvas interaction mirrors the code editor: hovering a node shows its
  // information; Cmd/Ctrl-clicking shows its usages. Both are popovers anchored
  // at the pointer.
  let canvasInfo = $state<CanvasInfo | null>(null); // { kind, name, fqn, x, y }
  let canvasUsages = $state<CanvasUsages | null>(null); // { name, items, x, y }

  // The byte offset of a node's declaration in its module source.
  const nodeByteOffset = (fileFqn: string, line: number, col: number) =>
    model.nodeByteOffset(moduleSources[fileFqn] ?? "", line, col);

  // Synthesised trigger actors aren't declared nodes; give them a fixed blurb.
  const ACTOR_DOC: Record<string, { kind: string; title: string; body: string }> = {
    client: { kind: "person", title: "client", body: "An external client calling in over HTTP." },
    scheduler: { kind: "system", title: "scheduler", body: "A scheduled trigger (timer / cron)." },
    caller: { kind: "person", title: "caller", body: "The caller of this operation." },
  };

  const resolveNodeFqn = (fqn: string) => model.resolveNodeFqn(index, fqn);

  // A symbol's { kind, title, body } documentation via the editor hover, or null.
  function docFor(fqn: string): { kind: string; title: string; body: string } | null {
    const hit = nodeIndex.get(fqn);
    if (!hit) return null;
    let info = null;
    try {
      info = hover(allModules, hit.fileFqn, nodeByteOffset(hit.fileFqn, hit.node.line, hit.node.col))?.info;
    } catch {
      info = null;
    }
    return { kind: hit.node.kind, title: info?.title || hit.node.name, body: info?.body || "" };
  }

  function showCanvasInfo(fqn: string, e: MouseEvent) {
    const at = { fqn, x: e.clientX, y: e.clientY };
    if (ACTOR_DOC[fqn]) {
      canvasInfo = { ...ACTOR_DOC[fqn], ...at };
      return;
    }
    // An `event:<fqn>` actor documents the event node it names.
    const real = fqn.startsWith("event:") ? fqn.slice(6) : resolveNodeFqn(fqn);
    const doc = real ? docFor(real) : null;
    canvasInfo = doc
      ? { ...doc, fqn: real ?? undefined, x: e.clientX, y: e.clientY }
      : fqn.startsWith("event:")
        ? { kind: "system", title: fqn.slice(6), body: "Triggered by this event.", ...at }
        : null;
  }
  const hideCanvasInfo = () => (canvasInfo = null);

  function showCanvasUsages(fqn: string, e: MouseEvent) {
    if (ACTOR_DOC[fqn]) {
      notify("info", "No usages", `\`${fqn}\` is a trigger actor, not a declared symbol.`);
      return;
    }
    const target = resolveNodeFqn(fqn.startsWith("event:") ? fqn.slice(6) : fqn);
    const hit = target ? nodeIndex.get(target) : null;
    if (!hit) {
      notify("info", "No usages", "Not a resolvable symbol.");
      return;
    }
    canvasInfo = null;
    let refs = null;
    try {
      refs = references(allModules, hit.fileFqn, nodeByteOffset(hit.fileFqn, hit.node.line, hit.node.col));
    } catch {
      refs = null;
    }
    if (!refs?.occurrences?.length) {
      notify("info", "No usages", `\`${hit.node.name}\` is not referenced.`);
      return;
    }
    canvasUsages = { name: refs.fqn.split("::").at(-1) ?? "", items: refs.occurrences, x: e.clientX, y: e.clientY };
  }
  function pickCanvasUsage(occ: Occurrence) {
    canvasUsages = null;
    openUsage(occ);
  }

  const nodeTitle = (fqn: string) => model.nodeTitle(index, fqn);

  const ownerNodeOf = (fqn: string) => model.ownerNodeOf(index, fqn);

  // Select a structure node: open its declaring file and remember it as the
  // current scope. `goto` (a nav click) also shows the code and jumps the editor
  // to the declaration; a canvas drill leaves the view alone. A member/field fqn
  // (`Owner::name`) isn't itself a node — fall back to its owner so GOTO on a
  // field opens its declaring type instead of no-opping (PDS-GOTO-002).
  function selectNode(fqn: string, { goto = false }: { goto?: boolean } = {}) {
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
    // Record the pre-jump caret before the file/scope changes, so Back returns.
    if (goto && view !== "canvas") recordOrigin();
    if (openFile?.fqn !== fileFqn) {
      flushSave();
      openFile = file;
    }
    selected = { fqn: targetFqn, line: hit.node.line, col: hit.node.col, fileFqn };
    // A nav click jumps the editor to the declaration — but only when the canvas
    // isn't showing; on the canvas the new scope is the navigation, so stay put.
    if (goto && view !== "canvas") {
      view = "code";
      pendingGoto = { line: hit.node.line, col: hit.node.col, fileFqn };
      recordLocation({ fileFqn, line: hit.node.line, col: hit.node.col, fqn: targetFqn, label: nodeTitle(targetFqn) });
    }
  }

  // Clicking a node in the canvas drills the selection into it (staying on the
  // canvas); synthetic initiators (client, scheduler, …) aren't declared nodes.
  const pickNode = (fqn: string) => selectNode(fqn);
  // Reset the canvas scope to the whole-model context.
  const resetScope = () => (selected = null);
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

  // The editor's hover popover: reveal the symbol's diagram on the canvas.
  function revealSymbol(fqn: string) {
    if (!nodeIndex.has(fqn)) return;
    selectNode(fqn);
    view = "canvas";
  }

  // Apply a queued editor jump once the code view is mounted on the right file.
  // Deferred a tick so the editor has synced the (possibly just-switched) file's
  // text before we index into it — otherwise a cross-file jump lands in the old doc.
  $effect(() => {
    if (view === "code" && editorApi && pendingGoto && openFile?.fqn === pendingGoto.fileFqn) {
      const target = pendingGoto;
      pendingGoto = null;
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
    // A `#w=` share link restores its workspace and skips the project panel;
    // otherwise open the panel on start (never autoload a model).
    const restored = await restoreFromHash();
    projectOpen = !restored;
  }
  onMount(boot);

  // Register the PWA service worker in production only (dev skips it to keep
  // Vite's HMR/module loading uncontended — see svelte.config.js).
  onMount(() => {
    if (!dev && "serviceWorker" in navigator) {
      navigator.serviceWorker.register(`${base}/service-worker.js`, { type: "module" });
    }
  });

  // Watch for edits made outside the IDE while this tab is visible: an immediate
  // re-read on focus, plus a modest backstop timer (visible-only, so a hidden tab
  // does no disk churn). `reloadExternalChanges` no-ops without an open folder.
  onMount(() => {
    const timer = setInterval(() => {
      if (document.visibilityState === "visible") reloadExternalChanges();
    }, 2500);
    return () => clearInterval(timer);
  });

  // Swap in a freshly-loaded workspace, resetting navigation to `landing`.
  async function mountWorkspace(ws: PageWorkspace, landing?: string | null) {
    flushSave();
    workspace = ws;
    // An explicit landing FQN (meta.json) resolves to its module immediately;
    // otherwise tentatively open the first module and revisit once docs load.
    const explicit = landing ? ws.files.find((f) => f.fqn === landing) : null;
    openFile = explicit ?? ws.files[0] ?? null;
    selected = null;
    pendingGoto = null;
    view = "code";
    history = [];
    histIndex = -1;
    projectOpen = false;
    docGroups = [];
    docSources = {};
    docMeta = {};
    // Reset the dirty/save state for the new workspace; module baselines are
    // seeded by the opener (folder/recent) before mount, doc baselines on load.
    saveState = "idle";
    clearTimeout(saveStateTimer);
    // Seed the editable manifest buffer; folder manifests also get an on-disk
    // baseline so the manifest row only shows dirty after a real edit.
    manifestSource = ws.manifestToml ?? "";
    manifestError = null;
    if (ws.root && ws.manifest) seedBaseline([{ key: ws.manifest.path, text: ws.manifestToml ?? "" }]);

    // Docs load async; await them so the initial open can prefer the docs
    // landing page. `loadWorkspaceDocs` returns null for a stale (superseded)
    // load — in which case a newer mount owns the open file, so leave it alone.
    const groups = await loadWorkspaceDocs(ws);
    if (groups === null) return; // a faster switch superseded this mount
    // With no explicit landing, prefer the first authored doc page (manifest
    // sidebar order); fall back to the first module already opened above. Don't
    // override a doc/manifest the user navigated to while docs loaded.
    if (!explicit && !openFile?.isDoc && !openFile?.isManifest) {
      const firstDoc = groups.flatMap((g) => g.items).find((it) => it?.path);
      if (firstDoc) openDoc(firstDoc);
    }
  }

  // Parses the workspace `[doc]` manifest and loads its `[[doc.sidebar]]` pages
  // (from disk for a folder, from the bundled map for a sample) into the doc
  // state the FileTree and the site build read.
  let docLoadSeq = 0;
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
    docSources = sources;
    docMeta = { name: manifest.name, theme: manifest.theme };
    docGroups = groups.map((g) => ({
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
    moduleSources = sources;
    persisted = base;
    workspace = { ...workspace!, files };
  }

  // Persist a programmatic `[[doc.sidebar]]` manifest change (T10): update the
  // live `workspace.manifestToml` + editable buffer, write `pds.toml` to disk
  // when folder-backed, re-seed its baseline, and re-resolve doc nav.
  async function persistManifest(toml: string) {
    const handle = workspace?.manifest?.handle;
    workspace = { ...workspace!, manifestToml: toml };
    manifestSource = toml;
    if (handle && workspace.manifest) {
      await writeFile(handle, toml);
      seedBaseline([{ key: workspace.manifest.path, text: toml }]);
    }
  }

  // One dialog drives every FileTree name prompt. `dialog` holds its config or
  // is null when closed; `confirmDialog` is the destructive-action confirm.
  let dialog = $state<Dialog | null>(null);
  let confirmDialog = $state<ConfirmDialog | null>(null);

  // A minimal valid `.pds` module: a system shell with one container and one
  // behaviour, mirroring the new-workspace starter so it resolves clean.
  // ---- T9: new .pds file ---------------------------------------------------
  // Thin view wrappers over the pure `core/workspace-ops` helpers, supplying the
  // live workspace context (file set, base dir) the pure functions take as args.
  const pdsSkeleton = (fqn: string) => ops.pdsSkeleton(fqn);
  const normalizePdsPath = (name: string) => ops.normalizePdsPath(name);
  const withBase = (rel: string) => ops.withBase(workspace?.base, rel);
  const validateNewFile = (name: string) => ops.validateNewFile(name, workspace?.files ?? [], workspace?.base);

  function startNewFile() {
    if (!workspace) return;
    dialog = {
      title: "New .pds file",
      label: "Module path",
      placeholder: "banking/core",
      value: "",
      confirmLabel: "Create",
      hint: "A .pds extension is added automatically. Use / for subfolders.",
      validate: validateNewFile,
      run: createNewFile,
    };
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
    dialog = {
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
    docSources = { ...docSources, [path]: body };
    if (ws.root && handle) seedBaseline([{ key: path, text: body }]);
    docGroups = docGroups.length
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
    dialog = {
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
    if (openFile && !openFile.isDoc && !openFile.isManifest && openFile.fqn === oldFqn) openFile = updated;
    const importers = danglingImporters(newFqn, oldFqn);
    if (importers.length) notify("info", `Renamed to ${newFqn}`, `${importers.length} module(s) still import the old name.`);
    else notify("success", `Renamed to ${newFqn}`);
  }

  const danglingImporters = (newFqn: string, oldFqn: string) =>
    ops.danglingImporters(workspace?.files ?? [], moduleSources, newFqn, oldFqn);

  function requestDeleteFile(file: OpenFile) {
    if (!workspace) return;
    confirmDialog = {
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
      else openFile = null;
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
      flash(`Opened ${ws.name} · ${ws.files.length} modules`);
    } catch {
      openFolder();
    }
  }

  function forgetRecent(entry: Recent) {
    forget(entry.key);
    refreshRecents();
  }

  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  // The pending debounced write, captured so a file switch (or Cmd/Ctrl-S) can
  // flush it instead of dropping it — the old silent-data-loss path.
  let pendingWrite: PendingWrite | null = null; // { handle, text, key }
  const flash = (message: string) => notifications.flash(message);

  // Briefly show a "saved" cue after a successful write, then settle to idle.
  function markSaved() {
    saveState = "saved";
    clearTimeout(saveStateTimer);
    saveStateTimer = setTimeout(() => (saveState = "idle"), 1600);
  }

  // Seed the persisted baseline for a batch of files read from disk, so they
  // start clean. `entries` is `[{ key, text }]`.
  function seedBaseline(entries: { key: string; text: string }[]) {
    persisted = advanceBaseline(persisted, entries);
  }

  // Write one buffer to disk and, on success, advance its baseline so it's no
  // longer dirty. Returns the write promise (already resolved for handle-less
  // samples). Failure keeps the baseline stale (still dirty) and surfaces it.
  async function persistFile(handle: FileSystemFileHandle | null | undefined, key: string, text: string) {
    if (!handle) return; // in-memory sample: session-only, no baseline to advance
    saveState = "saving";
    try {
      await writeFile(handle, text);
      persisted = { ...persisted, [key]: text };
      markSaved();
      // A saved manifest re-resolves the doc nav / name / theme.
      if (key === manifestKey) resolveManifest(text);
    } catch (e) {
      saveState = "error";
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
  async function reloadExternalChanges() {
    if (!workspace?.root || pendingWrite || saveState === "saving") return;
    const targets: { key: string; handle: FileSystemFileHandle; kind: "module" | "doc" | "manifest" }[] = [];
    for (const f of workspace.files) if (f.handle && f.fqn) targets.push({ key: f.fqn, handle: f.handle, kind: "module" });
    if (workspace.manifest?.handle && manifestKey)
      targets.push({ key: manifestKey, handle: workspace.manifest.handle, kind: "manifest" });
    for (const g of docGroups) for (const it of g.items) if (it.handle) targets.push({ key: it.path, handle: it.handle, kind: "doc" });

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
      const buffer = kind === "module" ? moduleSources[key] : kind === "doc" ? docSources[key] : manifestSource;
      const action = classifyReload(disk, base, buffer);
      if (action === "skip") continue;
      if (action === "conflict") {
        conflicts += 1;
        continue;
      }
      applyExternalReload(key, disk, kind);
    }
    if (conflicts > 0) flash(`${conflicts} file(s) changed on disk — your unsaved edits are kept`);
  }

  // Apply one externally-changed file: update its live buffer (which flows to the
  // editor and re-derives the model) and advance its baseline so it reads clean.
  function applyExternalReload(key: string, disk: string, kind: "module" | "doc" | "manifest") {
    if (kind === "module") moduleSources = { ...moduleSources, [key]: disk };
    else if (kind === "doc") docSources = { ...docSources, [key]: disk };
    else {
      manifestSource = disk;
      resolveManifest(disk);
    }
    persisted = { ...persisted, [key]: disk };
  }

  // Toast notifications — owned by the notifications store; thin view wrappers.
  const notify = (kind: NoteKind, title: string, body = "") => notifications.notify(kind, title, body);
  const dismissNote = (id: string | number) => notifications.dismiss(id);

  // Debounce a disk write for the active file. The pending write is captured so a
  // file switch or Cmd/Ctrl-S can flush it (rather than the old clearTimeout that
  // silently dropped a sub-400 ms edit).
  function scheduleSave(handle: FileSystemFileHandle | null | undefined, key: string | undefined, text: string) {
    if (!handle || !key) return; // in-memory sample: session-only
    clearTimeout(saveTimer);
    pendingWrite = { handle, key, text };
    saveTimer = setTimeout(() => {
      const w = pendingWrite;
      pendingWrite = null;
      if (w) persistFile(w.handle, w.key, w.text).catch(() => {});
    }, 400);
  }

  // Flush any debounced write immediately and await it — called before switching
  // files (so the outgoing edit lands) and from manual save.
  async function flushSave() {
    if (!pendingWrite) return;
    clearTimeout(saveTimer);
    const w = pendingWrite;
    pendingWrite = null;
    await persistFile(w.handle, w.key, w.text).catch(() => {});
  }

  // Manual save (Cmd/Ctrl-S): flush a pending debounce, else write the active
  // file's current buffer straight away. A clean file is a no-op cue.
  async function saveActiveFile() {
    if (pendingWrite) {
      await flushSave();
      return;
    }
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
      manifestSource = value;
      // A folder manifest re-resolves on save (debounced write); a session-only
      // sample has no save, so re-resolve live instead.
      if (openFile.handle) validateManifest(value);
      else resolveManifest(value);
    } else if (openFile.isDoc) {
      docSources = { ...docSources, [openFile.path ?? ""]: value };
    } else {
      moduleSources = { ...moduleSources, [openFile.fqn ?? ""]: value };
    }
    scheduleSave(openFile.handle, keyOf(openFile), value);
  }

  // Live parse check for the inline error cue; doesn't touch the doc nav.
  function validateManifest(toml: string) {
    try {
      docManifest(toml);
      manifestError = null;
    } catch (e) {
      manifestError = String((e as Error)?.message ?? e);
    }
  }

  // Re-resolve the workspace doc nav / name / theme from the saved manifest. A
  // parse error keeps the last good doc nav; reuses the shared doc loader by
  // swapping the live manifest text onto the workspace.
  function resolveManifest(toml: string) {
    try {
      docManifest(toml); // throws on malformed TOML
      manifestError = null;
    } catch (e) {
      manifestError = String((e as Error)?.message ?? e);
      return; // keep the last good doc nav
    }
    if (workspace) loadWorkspaceDocs({ ...workspace, manifestToml: toml });
  }

  // Open the workspace manifest (`pds.toml`) as raw, editable TOML. A folder's
  // manifest persists to its handle; a sample's is session-only (no handle).
  function openManifest() {
    if (!workspace?.manifest) return;
    flushSave();
    openFile = {
      isManifest: true,
      path: workspace.manifest.path,
      title: "pds.toml",
      handle: workspace.manifest.handle ?? null,
    };
    selected = null;
    view = "code";
  }

  // Opening a file from the nav clears any node scope; it shows the source,
  // unless the canvas is up — then it stays on the canvas (whole-model context).
  function selectFile(file: OpenFile) {
    flushSave();
    openFile = file;
    selected = null;
    if (view !== "canvas") view = "code";
  }

  // Open an authored doc page (`[[doc.sidebar]]`) as raw Markdown in the editor.
  // Marked `isDoc` so the editor drops PseudoScript language features and edits
  // route to `docSources` (and save to the page's handle on a real folder).
  function openDoc(item: LiveDocItem) {
    flushSave();
    openFile = { isDoc: true, path: item.path, title: item.title, handle: item.handle ?? null };
    selected = null;
    view = "code";
  }

  async function onProblemPick(d: Problem) {
    view = "code";
    if (d.file && workspace && d.file !== openFile?.fqn) {
      const f = workspace.files.find((x) => x.fqn === d.file);
      if (f) selectFile(f);
    }
    await tick();
    editorApi?.goto(d.start_line, d.start_col);
    if (d.file) recordLocation({ fileFqn: d.file, line: d.start_line, col: d.start_col, label: d.message });
  }

  // Read every module into the live buffer, seed a clean on-disk baseline, mount,
  // and record the folder in recents. The single adopt path shared by opening a
  // folder, scaffolding a new project, and re-opening a recent — every workspace
  // is disk-backed, so this always has real handles to read and persist against.
  async function adoptWorkspace(ws: PageWorkspace, landing?: string | null) {
    const sources: Record<string, string> = {};
    for (const file of ws.files) if (file.handle) sources[file.fqn ?? ""] = await readFile(file.handle);
    moduleSources = sources;
    persisted = { ...sources };
    await mountWorkspace(ws, landing ?? ws.files[0]?.fqn);
    if (ws.root) {
      await recordFolder(ws.name, ws.root);
      refreshRecents();
    }
  }

  // Bootstrap a project from a template — the empty one-module starter, or an
  // example scaffolded onto disk. Prompts for a parent directory, writes the
  // files, then opens it like any folder. Cancelling the picker is a no-op.
  async function newProject(name: string, templateId: string) {
    const tpl =
      templateId === "empty" ? { seed: emptySeed(name), landing: null as string | null } : sampleSeed(templateId);
    if (!tpl) return;
    try {
      const ws = await scaffoldWorkspace(name, tpl.seed);
      await adoptWorkspace(ws, tpl.landing);
      flash(`Created ${ws.name}`);
    } catch {
      // picker cancelled or permission denied — keep the current workspace
    }
  }

  async function openFolder() {
    try {
      const ws = await openWorkspace();
      await adoptWorkspace(ws);
      flash(`Opened ${ws.name} · ${ws.files.length} modules`);
    } catch {
      // picker cancelled or permission denied — keep the current workspace
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
    if (!openFile) return;
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
        flash("Cannot format Markdown — check the document");
      }
      return;
    }
    try {
      onEditorChange(formatSource(source));
    } catch {
      flash("Cannot format — fix syntax errors first");
    }
  }

  let building = $state(false);
  let buildNotice = $state(false); // the blocking example-vs-folder modal

  // Build the static documentation site (the browser equivalent of `pds doc`).
  // An opened folder builds straight to disk; the bundled example first opens a
  // blocking notice explaining it can only be previewed.
  function onbuilddocs() {
    if (!ready || !workspace || building) return;
    if (workspace.root) {
      runBuild();
    } else {
      buildNotice = true;
    }
  }

  // Confirmed from the modal: build the example as a read-only preview.
  function confirmPreviewBuild() {
    buildNotice = false;
    runBuild();
  }
  // From the modal: open a real folder to build to disk instead.
  function openFolderFromNotice() {
    buildNotice = false;
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
    building = true;
    try {
      const config = buildDocConfig();
      const files = renderDocSite(allModules, config);
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
      building = false;
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
  function mountDecoded({ workspace: ws, landing }: MountInput) {
    moduleSources = share.mountedSources(ws.files as { fqn?: string; source?: string }[]);
    persisted = {}; // imported/shared: no on-disk baseline, session-only
    mountWorkspace(ws, landing);
  }

  let busyShare = $state(false);

  // Share: encode the live workspace, base64url it into the URL hash, and copy
  // the link. Over the size guard, fall back to a file export instead.
  async function onshare() {
    if (!workspace || busyShare) return;
    busyShare = true;
    try {
      const bytes = await encodeWorkspace(snapshotWorkspace());
      if (bytes.length > MAX_HASH_BYTES) {
        notify("info", "Workspace too large to share by link", "Exported it as a file instead.");
        await onexport();
        return;
      }
      const payload = bytesToBase64Url(bytes);
      const url = `${location.origin}${location.pathname}#w=${payload}`;
      // `window.` qualified: the component's nav `history` $state array shadows
      // the global `history`.
      window.history.replaceState(null, "", `#w=${payload}`);
      try {
        await navigator.clipboard.writeText(url);
        flash("Share link copied to clipboard");
      } catch {
        flash("Share link is in the address bar");
      }
    } catch (e) {
      notify("error", "Could not create share link", String((e as Error)?.message ?? e));
    } finally {
      busyShare = false;
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
      flash(`Exported ${a.download}`);
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
        flash(`Imported ${file.name}`);
      } catch (e) {
        notify("error", "Could not import workspace", String((e as Error)?.message ?? e));
      }
    };
    input.click();
  }

  // Restore a workspace from a `#w=<payload>` share link on first load. Cleared
  // from the URL after mounting so a refresh doesn't re-trigger it.
  async function restoreFromHash() {
    const payload = share.parseHashPayload(location.hash);
    if (!payload) return false;
    try {
      const bytes = base64UrlToBytes(payload) as Uint8Array<ArrayBuffer>;
      mountDecoded(await decodeWorkspace(bytes));
      window.history.replaceState(null, "", location.pathname + location.search);
      flash("Restored shared workspace");
      return true;
    } catch (e) {
      notify("error", "Could not open shared link", String((e as Error)?.message ?? e));
      window.history.replaceState(null, "", location.pathname + location.search);
      return false;
    }
  }

</script>

<svelte:head><title>PseudoScript Web IDE</title></svelte:head>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") {
      if (buildNotice) buildNotice = false;
      if (projectOpen && workspace) projectOpen = false;
      canvasInfo = null;
      canvasUsages = null;
    }
    // Cmd/Ctrl-S saves the active file even when the editor isn't focused (e.g.
    // on the canvas or problems view). The editor's own keymap handles the
    // focused case; this prevents the browser's "save page" dialog regardless.
    if ((e.metaKey || e.ctrlKey) && !e.altKey && (e.key === "s" || e.key === "S")) {
      e.preventDefault();
      saveActiveFile();
    }
  }}
  onfocus={reloadExternalChanges}
  onbeforeunload={(e) => {
    // Warn before closing with unsaved work that can be persisted, or with a
    // write still in flight.
    if (canPersist && (dirtyCount > 0 || pendingWrite)) {
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
      dialog = null;
      run?.(v);
    }}
    oncancel={() => (dialog = null)}
  />
{/if}

{#if confirmDialog}
  <div
    class="confirm-backdrop"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) confirmDialog = null;
    }}
  >
    <div class="confirm" role="alertdialog" aria-modal="true" aria-label={confirmDialog.title}>
      <h2>{confirmDialog.title}</h2>
      <p>{confirmDialog.message}</p>
      <div class="confirm-actions">
        <button class="ghost" type="button" onclick={() => (confirmDialog = null)}>Cancel</button>
        <button
          class="danger"
          type="button"
          onclick={() => {
            const run = confirmDialog?.run;
            confirmDialog = null;
            run?.();
          }}
        >
          {confirmDialog.confirmLabel ?? "Delete"}
        </button>
      </div>
    </div>
  </div>
{/if}

<Notifications notes={notifications.notes} ondismiss={dismissNote} />

{#if ready && projectOpen}
  <ProjectPanel
    {templates}
    {recents}
    dismissible={!!workspace}
    onpickrecent={openRecent}
    onopenfolder={openFolder}
    onnewproject={newProject}
    onforget={forgetRecent}
    onclose={() => (projectOpen = false)}
  />
{/if}

{#if settingsOpen}
  <Settings onclose={() => (settingsOpen = false)} />
{/if}

<!-- Canvas hover info: kind eyebrow, name, FQN, anchored at the pointer. -->
{#if canvasInfo}
  <div class="canvas-pop info" style="left:{canvasInfo.x + 14}px; top:{canvasInfo.y + 14}px">
    <span class="kind {canvasInfo.kind}">{canvasInfo.kind}</span>
    <span class="name">{canvasInfo.title}</span>
    {#if canvasInfo.body}<p class="doc">{canvasInfo.body}</p>{/if}
    <span class="fqn">{canvasInfo.fqn}</span>
  </div>
{/if}

<!-- Canvas usages: a click-away list of references; picking one jumps to it. -->
{#if canvasUsages}
  <button type="button" class="canvas-backdrop" aria-label="Dismiss usages" onclick={() => (canvasUsages = null)}></button>
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
  <!-- Backdrop-click is a mouse convenience; Escape and the Cancel button give
       full keyboard access, so the static-element interaction lint is waived. -->
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="scrim" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) buildNotice = false; }}>
    <div class="modal" role="dialog" aria-modal="true" aria-labelledby="build-notice-title">
      <h2 id="build-notice-title">Build the example as a preview?</h2>
      <p>
        You're working in the bundled <b>example</b>, which lives in memory — there's no folder to write to.
        Building it opens a <b>read-only preview</b> in a new tab, and the interactive diagrams don't load there.
      </p>
      <p>
        To build a real, on-disk site like the <code>pds doc</code> CLI — written under <code>target/doc/</code>,
        with the diagrams hydrated — open a folder as your workspace first.
      </p>
      <div class="modal-actions">
        <button class="ghost" onclick={() => (buildNotice = false)}>Cancel</button>
        {#if fsSupported}
          <button class="ghost" onclick={openFolderFromNotice}>Open a folder…</button>
        {/if}
        <button class="primary" onclick={confirmPreviewBuild}>Build preview</button>
      </div>
    </div>
  </div>
{/if}

{#snippet symbolLabel(title: string)}
  {#each title.split("`") as part, i}{#if i % 2}<code>{part}</code>{:else}<span>{part}</span>{/if}{/each}
{/snippet}

<!-- The selected item's path: context / system / container / … — each ancestor a
     hop, the leaf the current scope. -->
{#snippet breadcrumb()}
  {@const chain = selected ? ancestry(selected.fqn) : []}
  <div class="crumb">
    <button class="reset" class:active={!selected} onclick={resetScope} title="Whole-model context">context</button>
    {#each chain as fqn, i (fqn)}
      <span class="sep">/</span>
      {#if i < chain.length - 1}
        <button class="hop" onclick={() => selectNode(fqn)}>{@render symbolLabel(nodeTitle(fqn))}</button>
      {:else}
        {@render symbolLabel(nodeTitle(fqn))}
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
      onclick={() => (mdHelpOpen = !mdHelpOpen)}
    >?</button>
    {#if mdHelpOpen}
      <button class="md-help-scrim" aria-label="Close" onclick={() => (mdHelpOpen = false)}></button>
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

<!-- The CODE | CANVAS view toggle, with a Problems tab carrying the error count. -->
{#snippet viewToggle()}
  <div class="view-toggle" role="tablist" aria-label="View">
    <button role="tab" aria-selected={view === "code"} class:active={view === "code"} onclick={() => (view = "code")}>Code</button>
    <button role="tab" aria-selected={view === "canvas"} class:active={view === "canvas"} onclick={() => (view = "canvas")}>Canvas</button>
    <button role="tab" aria-selected={view === "problems"} class:active={view === "problems"} class:has-errors={errorCount > 0} onclick={() => (view = "problems")}>
      Problems{#if problems.length}<span class="count" class:bad={errorCount > 0}>{problems.length}</span>{/if}
    </button>
  </div>
{/snippet}

<div class="app">
  <Toolbar
    {errorCount}
    workspaceName={workspace?.name ?? null}
    {building}
    {dirtyCount}
    {saveState}
    {onformat}
    onproject={() => (projectOpen = true)}
    {onbuilddocs}
    {onshare}
    {onexport}
    {onimport}
    onopensettings={() => (settingsOpen = true)}
  />

  {#if wasmError}
    <div class="curtain">
      <div class="kicker">compiler failed to load</div>
      <p class="msg">{wasmError}</p>
      <button class="retry" onclick={boot}>Retry</button>
    </div>
  {:else if !fsSupported}
    <div class="curtain">
      <div class="kicker">browser not supported</div>
      <p class="msg">
        The PseudoScript IDE reads and writes your project as real files on disk, which needs the File
        System Access API. That's available in Chromium browsers — Chrome, Edge, Brave, Arc. Firefox and
        Safari don't support it yet.
      </p>
    </div>
  {:else if ready && workspace}
    <main class="workspace has-tree">
      <section class="pane tree-pane reveal r1">
        <FileTree
          workspaceName={workspace.name}
          files={workspace.files as { fqn: string; path: string }[]}
          openPath={openFile?.path ?? null}
          {docGroups}
          ondocopen={openDoc}
          symbols={symbols as never}
          selectedFqn={selected?.fqn ?? null}
          {errorPaths}
          {dirtyPaths}
          manifestPath={workspace?.manifest?.path ?? null}
          base={workspace?.base ?? ""}
          onmanifestopen={openManifest}
          onopen={selectFile}
          onpicknode={(fqn) => selectNode(fqn, { goto: true })}
          oncreatefile={startNewFile}
          oncreatedoc={startNewDoc}
          onrenamefile={startRenameFile}
          onmovefile={moveFile}
          ondeletefile={requestDeleteFile}
        />
      </section>

      <section class="pane content-pane reveal r2">
        <header class="content-bar">
          <div class="nav-buttons">
            <button class="nav-btn" onclick={goBack} disabled={!canBack} title="Back (previous location)" aria-label="Back">←</button>
            <button class="nav-btn" onclick={goForward} disabled={!canForward} title="Forward (next location)" aria-label="Forward">→</button>
          </div>
          {@render breadcrumb()}
          <div class="bar-actions">
            {#if openFile?.isDoc}{@render mdHelp()}{@render docWidthToggle()}{/if}
            {@render viewToggle()}
          </div>
        </header>
        <div class="content-body">
          <div class="layer code-layer" class:hidden={view !== "code"} data-doc-width={docWidth}>
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
            <Editor
              value={source}
              onchange={onEditorChange}
              onready={(api) => (editorApi = api)}
              modules={allModules}
              moduleFqn={openFile?.fqn ?? ""}
              plain={(openFile?.isDoc || openFile?.isManifest) ?? false}
              markdown={openFile?.isDoc ?? false}
              {previewOpts}
              {symbols}
              onopensymbol={revealSymbol}
              ongotodefinition={(fqn) => selectNode(fqn, { goto: true })}
              onnavigate={openUsage}
              {onformat}
              onsave={saveActiveFile}
              onopensettings={() => (settingsOpen = true)}
            />
          </div>
          {#if view === "canvas"}
            <div class="layer canvas-layer">
              <DiagramPane scene={canvas.scene} layout={canvas.layout} error={canvas.error} hint={canvasHint} onpick={pickNode} onup={navigateUp} flows={flowsByNode} depth={seqDepth} ondepth={(d: Depth) => (seqDepth = d)} oninfo={showCanvasInfo} oninfoend={hideCanvasInfo} onusages={showCanvasUsages} typeFqn={typeFqnByName as never} />
            </div>
          {:else if view === "problems"}
            <div class="layer">
              <ProblemsPane diagnostics={problems} onpick={onProblemPick} />
            </div>
          {/if}
        </div>
      </section>
    </main>
  {:else if ready}
    <div class="stage-empty"></div>
  {:else}
    <div class="curtain">
      <div class="loader"><span class="bar"></span></div>
      <div class="kicker">compiling the compiler…</div>
    </div>
  {/if}

  <footer class="statusbar">
    <span class="seg accent">pds</span>
    <span class="seg">wasm{ver ? ` ${ver}` : ""}</span>
    {#if workspace}
      <span class="seg file" class:dirty={openFile && dirty.has(keyOf(openFile) ?? "")}>
        {#if openFile && dirty.has(keyOf(openFile) ?? "")}<span class="unsaved-dot" aria-hidden="true"></span>{/if}
        {openFile?.fqn ?? openFile?.title ?? "—"}
      </span>
      <span class="seg dim">{workspace.files.length} modules</span>
    {/if}
    <span class="grow"></span>
    {#if notifications.toast}<span class="seg toast">{notifications.toast}</span>{/if}
    <span class="seg dim">{view}</span>
    <span class="seg dim">{selected?.fqn ?? "context"}</span>
  </footer>
</div>

<style>
  /* destructive-action confirm modal (file delete) */
  .confirm-backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--bg, #000) 62%, transparent);
    backdrop-filter: blur(2px);
  }
  .confirm {
    width: min(26rem, calc(100vw - 2rem));
    background: var(--surface, #fff);
    border: 1px solid var(--line);
    border-radius: var(--radius, 10px);
    padding: 1.1rem 1.2rem 1.2rem;
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.35);
  }
  .confirm h2 {
    margin: 0 0 0.5rem;
    font-size: 0.95rem;
    color: var(--ink);
  }
  .confirm p {
    margin: 0;
    font-size: 0.82rem;
    color: var(--ink-soft);
  }
  .confirm-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 1.1rem;
  }
  .confirm-actions button {
    padding: 0.45rem 0.85rem;
    font-size: 0.8rem;
    border-radius: var(--radius-sm, 6px);
    cursor: pointer;
    border: 1px solid var(--line);
  }
  .confirm-actions .ghost {
    background: transparent;
    color: var(--ink-soft);
  }
  .confirm-actions .ghost:hover {
    background: var(--surface-2);
    color: var(--ink);
  }
  .confirm-actions .danger {
    background: var(--err);
    border-color: var(--err);
    color: #fff;
  }

  .app {
    display: grid;
    grid-template-rows: var(--topbar-h) 1fr var(--status-h);
    height: 100vh;
  }
  .workspace {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    min-height: 0;
  }
  .workspace.has-tree {
    grid-template-columns: 268px minmax(0, 1fr);
  }
  /* the backdrop behind the project panel when no workspace is loaded yet */
  .stage-empty {
    min-height: 0;
    background-image:
      linear-gradient(var(--grid) 1px, transparent 1px),
      linear-gradient(90deg, var(--grid) 1px, transparent 1px);
    background-size: 30px 30px, 30px 30px;
  }
  .pane { min-width: 0; min-height: 0; }
  .tree-pane {
    border-right: 1px solid var(--line);
    background: color-mix(in srgb, var(--surface) 70%, transparent);
  }

  /* the content pane: a header (breadcrumb + view toggle) over the active view */
  .content-pane {
    display: grid;
    grid-template-rows: auto 1fr;
    min-height: 0;
    background: color-mix(in srgb, var(--surface) 55%, transparent);
  }
  .content-bar {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    padding: 0.45rem 0.7rem;
    border-bottom: 1px solid var(--line);
    background: color-mix(in srgb, var(--surface) 60%, transparent);
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
    background:
      radial-gradient(900px 520px at 60% -10%, color-mix(in srgb, var(--accent) 6%, transparent), transparent 70%),
      var(--bg);
  }

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
  .view-toggle {
    flex: none;
    display: flex;
    gap: 0.15rem;
    padding: 0.18rem;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
  }
  .view-toggle button {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    background: transparent;
    border: none;
    border-radius: calc(var(--radius-sm) - 2px);
    color: var(--ink-faint);
    font-family: var(--font-mono);
    font-size: 0.72rem;
    letter-spacing: 0.04em;
    padding: 0.3rem 0.7rem;
    cursor: pointer;
  }
  .view-toggle button:hover { color: var(--ink); }
  .view-toggle button.active { background: var(--accent); color: var(--accent-ink); }
  .view-toggle .count {
    font-size: 0.62rem;
    padding: 0 0.3rem;
    border-radius: 999px;
    background: var(--surface-3);
    color: var(--ink-soft);
  }
  .view-toggle button.active .count { background: var(--accent-ink); color: var(--accent); }
  .view-toggle .count.bad { background: var(--err); color: #fff; }

  .crumb {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: var(--ink-soft);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .crumb :global(code) {
    color: var(--ink);
    background: var(--surface-2);
    padding: 0.05rem 0.35rem;
    border-radius: var(--radius-sm);
  }
  .crumb .sep { color: var(--ink-faint); }
  .crumb .reset {
    background: transparent;
    border: none;
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: 0.72rem;
    cursor: pointer;
    padding: 0;
  }
  .crumb .reset:hover { text-decoration: underline; }
  .crumb .reset.active { color: var(--ink-faint); cursor: default; }
  .crumb .reset.active:hover { text-decoration: none; }
  .crumb .hop {
    background: transparent;
    border: none;
    padding: 0;
    font: inherit;
    color: var(--ink-soft);
    cursor: pointer;
  }
  .crumb .hop:hover { color: var(--ink); }
  .crumb .hop:hover :global(code) { color: var(--ink); }

  /* blocking build-notice modal */
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 70;
    display: grid;
    place-items: center;
    padding: 1.5rem;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(2px);
    animation: fade-in 0.16s ease both;
  }
  .modal {
    width: min(460px, 100%);
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg);
    padding: 1.3rem 1.4rem;
    animation: rise 0.22s cubic-bezier(0.2, 0.7, 0.2, 1) both;
  }
  .modal h2 {
    margin: 0 0 0.7rem;
    font-family: var(--font-display);
    font-size: 1.12rem;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: var(--ink);
  }
  .modal p {
    margin: 0 0 0.7rem;
    font-size: 0.86rem;
    line-height: 1.65;
    color: var(--ink-soft);
  }
  .modal code {
    font-family: var(--font-mono);
    font-size: 0.82em;
    background: var(--surface-2);
    color: var(--ink);
    padding: 0.05rem 0.35rem;
    border-radius: var(--radius-sm);
  }
  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 1.1rem;
  }
  .modal-actions button {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    padding: 0.5rem 0.95rem;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .modal-actions .ghost {
    background: var(--surface-2);
    border: 1px solid var(--line-strong);
    color: var(--ink-soft);
  }
  .modal-actions .ghost:hover { border-color: var(--accent); color: var(--ink); }
  .modal-actions .primary {
    background: var(--accent);
    border: 1px solid var(--accent);
    color: var(--accent-ink);
    font-weight: 700;
  }
  .modal-actions .primary:hover { background: var(--accent-hi); }

  @keyframes fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

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

  .statusbar {
    display: flex;
    align-items: center;
    gap: 0.9rem;
    padding: 0 1.1rem;
    border-top: 1px solid var(--line);
    background: color-mix(in srgb, var(--surface) 80%, transparent);
    backdrop-filter: blur(8px);
    font-family: var(--font-mono);
    font-size: 0.7rem;
    color: var(--ink-soft);
  }
  .statusbar .seg { white-space: nowrap; }
  .statusbar .seg.accent { color: var(--accent); font-weight: 600; letter-spacing: 0.05em; }
  .statusbar .seg.dim { color: var(--ink-faint); }
  .statusbar .seg.toast { color: var(--accent); }
  .statusbar .grow { flex: 1; }

  /* active-file segment: an unsaved dot when the open file differs from disk */
  .statusbar .seg.file { display: inline-flex; align-items: center; gap: 0.4rem; }
  .statusbar .seg.file.dirty { color: var(--warn); }
  .statusbar .unsaved-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--warn);
  }

  /* canvas hover/usages popovers, anchored at the pointer */
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
  .canvas-pop.info { display: flex; flex-direction: column; gap: 0.15rem; pointer-events: none; }
  .canvas-pop .kind {
    font-family: var(--font-mono);
    font-size: 0.52rem;
    font-weight: 600;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--k, var(--ink-faint));
  }
  .canvas-pop .kind.person { --k: var(--k-person); }
  .canvas-pop .kind.system { --k: var(--k-system); }
  .canvas-pop .kind.container { --k: var(--k-container); }
  .canvas-pop .kind.component { --k: var(--k-component); }
  .canvas-pop .kind.data { --k: var(--k-data); }
  .canvas-pop .kind.callable { --k: var(--k-callable); }
  .canvas-pop .name { font-family: var(--font-mono); font-weight: 600; color: var(--ink); }
  .canvas-pop .doc {
    margin: 0.35rem 0 0;
    font-size: 0.8rem;
    line-height: 1.5;
    color: var(--ink-soft);
    white-space: pre-wrap;
  }
  .canvas-pop .fqn { margin-top: 0.35rem; font-family: var(--font-mono); font-size: 0.7rem; color: var(--ink-faint); }
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
