<script>
  import { onMount, tick } from "svelte";
  import "../app.css";
  import { checkModules, docManifest, emitSceneModules, format as formatSource, hover, initWasm, layoutScene, outline, outlineModules, references, renderDocSite, symbolScene, version } from "$lib/pds.js";
  import { charToByte } from "$lib/offsets.js";
  import { fsSupported, openWorkspace, readWorkspace, readDocPages, readFile, writeFile, writeSite } from "$lib/workspace.js";
  import { collapseSequence } from "$lib/sequence.js";
  import { SAMPLES, loadSample } from "$lib/samples.js";
  import { getRecents, recordSample, recordFolder, reopenFolder, forget } from "$lib/recents.js";
  import Editor from "$lib/components/Editor.svelte";
  import Toolbar from "$lib/components/Toolbar.svelte";
  import FileTree from "$lib/components/FileTree.svelte";
  import DiagramPane from "$lib/components/DiagramPane.svelte";
  import ProblemsPane from "$lib/components/ProblemsPane.svelte";
  import Notifications from "$lib/components/Notifications.svelte";
  import ProjectPanel from "$lib/components/ProjectPanel.svelte";
  import Settings from "$lib/components/Settings.svelte";

  let ready = $state(false);
  let wasmError = $state(null);
  let ver = $state("");
  let toast = $state(null);
  let editorApi = $state(null);

  // The selected item's view: its source ("code"), its interactive diagram
  // ("canvas"), or the workspace problem list ("problems"). The nav stays put;
  // only this content pane swaps.
  let view = $state("code");

  // The C4 depth a sequence diagram is collapsed to (persons & systems /
  // + containers / + components). Components = full detail (no collapse).
  let seqDepth = $state("component");

  // The structure node selected in the nav / drilled into on the canvas, as
  // { fqn, line, col, fileFqn }, or null for the whole-model scope. Drives the
  // canvas diagram, the breadcrumb, and which nav row is highlighted.
  let selected = $state(null);
  // A queued editor jump (set when a node is picked); applied once the code view
  // is mounted and showing the node's file.
  let pendingGoto = $state(null);

  // Navigation history: every code location jumped to (go-to-definition, a nav
  // pick, a problem, a find-usages hit), as { fileFqn, line, col, label, fqn? }.
  // `histIndex` is the current position; back/forward step through it without
  // recording. New jumps truncate the forward tail (browser-history semantics).
  let history = $state([]);
  let histIndex = $state(-1);
  const canBack = $derived(histIndex > 0);
  const canForward = $derived(histIndex >= 0 && histIndex < history.length - 1);

  // Record a visited location, dropping any forward tail and collapsing a repeat
  // of the current location (label may refresh). Capped so the list stays bounded.
  function recordLocation(loc) {
    const trail = history.slice(0, histIndex + 1);
    const last = trail.at(-1);
    if (last && last.fileFqn === loc.fileFqn && last.line === loc.line && last.col === loc.col) {
      trail[trail.length - 1] = loc;
      history = trail;
    } else {
      history = [...trail, loc].slice(-50);
    }
    histIndex = history.length - 1;
  }

  // Apply a location without recording it (back/forward, history-list click):
  // open its file, jump the editor there, and re-scope to its node when it has one.
  function applyLocation(loc) {
    const file = workspace?.files.find((f) => f.fqn === loc.fileFqn);
    if (!file) return;
    if (openFile?.fqn !== file.fqn) {
      clearTimeout(saveTimer);
      openFile = file;
    }
    if (loc.fqn) selected = { fqn: loc.fqn, line: loc.line, col: loc.col, fileFqn: loc.fileFqn };
    view = "code";
    pendingGoto = { line: loc.line, col: loc.col, fileFqn: loc.fileFqn };
  }

  const goBack = () => canBack && (histIndex -= 1, applyLocation(history[histIndex]));
  const goForward = () => canForward && (histIndex += 1, applyLocation(history[histIndex]));

  // Open a find-usages occurrence: jump to it and record it in history.
  function openUsage(occ) {
    applyLocation({ fileFqn: occ.fqn, line: occ.line, col: occ.col });
    recordLocation({ fileFqn: occ.fqn, line: occ.line, col: occ.col, label: occ.text || `${occ.fqn}:${occ.line}` });
  }

  // Workspace state. Defaults to the bundled sample (in-memory, handles null);
  // "Open folder" swaps in a real on-disk workspace whose files persist on edit.
  let workspace = $state(null);
  let openFile = $state(null);
  let moduleSources = $state({});
  // Authored docs (`[[doc.sidebar]]`): the sidebar groups (`{ title, items:
  // [{ title, path, handle }] }`), each page's live Markdown by path, and the
  // `{ name, theme }` parsed from `[doc]` for the site build. Loaded on open.
  let docGroups = $state([]);
  let docSources = $state({});
  let docMeta = $state({});

  // The Markdown reading width (narrow | wide | full), persisted across sessions.
  let docWidth = $state(loadDocWidth());
  function loadDocWidth() {
    try {
      return localStorage.getItem("pds-doc-width") || "narrow";
    } catch {
      return "narrow";
    }
  }
  function setDocWidth(w) {
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
  let recents = $state([]);
  const refreshRecents = () => (recents = getRecents());

  const source = $derived(
    openFile?.isDoc
      ? (docSources[openFile.path] ?? "")
      : openFile
        ? (moduleSources[openFile.fqn] ?? "")
        : "",
  );

  // Every module as {fqn, source} — diagrams and the target list span the whole
  // workspace (cross-module containers/components/edges), not just the open file.
  const allModules = $derived(
    workspace ? workspace.files.map((f) => ({ fqn: f.fqn, source: moduleSources[f.fqn] ?? "" })) : [],
  );

  const workspaceResults = $derived.by(() => {
    if (!ready || !workspace) return null;
    const modules = workspace.files.map((f) => ({ fqn: f.fqn, source: moduleSources[f.fqn] ?? "" }));
    try {
      return checkModules(modules);
    } catch {
      return null;
    }
  });

  const problems = $derived.by(() => {
    if (workspace && workspaceResults) {
      return workspaceResults.flatMap((m) => m.diagnostics.map((d) => ({ ...d, file: m.fqn })));
    }
    return [];
  });
  const errorCount = $derived(problems.filter((d) => d.severity === "error").length);

  const errorPaths = $derived.by(() => {
    const paths = new Set();
    if (workspace && workspaceResults) {
      const byFqn = new Map(workspace.files.map((f) => [f.fqn, f.path]));
      for (const m of workspaceResults) {
        if (m.diagnostics.some((d) => d.severity === "error")) paths.add(byFqn.get(m.fqn));
      }
    }
    return paths;
  });

  const nodes = $derived.by(() => {
    if (!ready || !workspace) return [];
    try {
      return outlineModules(allModules);
    } catch {
      return [];
    }
  });
  // The C4 nodes each file declares, keyed by file FQN, so the workspace tree can
  // nest a module's structure beneath it. Per-file (not the workspace outline) so
  // each module shows only what it contributes.
  const structureByFile = $derived.by(() => {
    const by = {};
    if (!ready || !workspace) return by;
    for (const f of workspace.files) {
      try {
        by[f.fqn] = outline(moduleSources[f.fqn] ?? "");
      } catch {
        by[f.fqn] = [];
      }
    }
    return by;
  });
  // FQN → { node, fileFqn } for every declared node, so a canvas drill or a
  // breadcrumb hop resolves a node to its declaring file (for goto).
  const nodeIndex = $derived.by(() => {
    const idx = new Map();
    for (const [fileFqn, list] of Object.entries(structureByFile)) {
      for (const n of list) idx.set(n.fqn, { node: n, fileFqn });
    }
    return idx;
  });
  // Every declared node, tagged with its file — flat, for the nav's symbol tree
  // to nest by structural `parent` (which crosses files: a container's system
  // may live in another module).
  const symbols = $derived.by(() =>
    [...nodeIndex.entries()].map(([fqn, { node, fileFqn }]) => ({ ...node, fqn, fileFqn })),
  );
  // Entry-point flows keyed by their owning node FQN: each node's callables, so
  // the System view's popover can offer "play this flow" links down to a
  // sequence trace (the only path from the structural graph into a flow).
  const flowsByNode = $derived.by(() => {
    const m = new Map();
    for (const n of nodes) {
      if (n.kind !== "callable") continue;
      const parent = n.fqn.split("::").slice(0, -1).join("::");
      if (!m.has(parent)) m.set(parent, []);
      m.get(parent).push({ fqn: n.fqn, name: n.name, triggered: n.triggered });
    }
    return m;
  });
  // FQN → { kind, parent } for every declared node, the ancestry the sequence
  // depth-collapse walks to fold a participant into its nearest allowed C4 level.
  const nodeInfo = $derived.by(() => {
    const m = {};
    for (const n of nodes) m[n.fqn] = { kind: n.kind, parent: n.parent ?? null };
    return m;
  });
  // Type name → FQN for the `data` declarations, so a type token in a message
  // signature (e.g. `Session`) resolves to its declaration for hover/usages.
  const typeFqnByName = $derived.by(() => {
    const m = {};
    for (const n of nodes) if (n.kind === "data") m[n.name] = n.fqn;
    return m;
  });

  // The CANVAS diagram: the selected node's fitting view, or the whole-model
  // context overview when nothing is selected. The compiler picks the view; a
  // sequence scene is then collapsed to the chosen depth in the IDE.
  const canvas = $derived.by(() => {
    if (!ready || !workspace) return { scene: null, error: "" };
    try {
      const scene = selected
        ? symbolScene(allModules, selected.fqn)
        : emitSceneModules(allModules, "context", "");
      const isSeq = scene && Array.isArray(scene.participants);
      const shown = isSeq ? collapseSequence(scene, seqDepth, nodeInfo) : scene;
      // A sequence scene is positioned by the layout engine; C4 stays as-is.
      const layout = isSeq ? layoutScene(shown) : null;
      return { scene: shown, layout, error: "" };
    } catch (e) {
      return { scene: null, layout: null, error: String(e?.message ?? e) };
    }
  });

  const canvasHint = $derived(
    selected
      ? "Nothing to draw for this item."
      : "No persons or systems declared yet — the context overview draws systems and people.",
  );

  // Canvas interaction mirrors the code editor: hovering a node shows its
  // information; Cmd/Ctrl-clicking shows its usages. Both are popovers anchored
  // at the pointer.
  let canvasInfo = $state(null); // { kind, name, fqn, x, y }
  let canvasUsages = $state(null); // { name, items, x, y }

  // The byte offset of a node's declaration (line/col are 1-based; col is a byte
  // column, exact for the ASCII model source).
  function nodeByteOffset(fileFqn, line, col) {
    const src = moduleSources[fileFqn] ?? "";
    const lines = src.split("\n");
    let charOffset = 0;
    for (let i = 0; i < line - 1 && i < lines.length; i++) charOffset += lines[i].length + 1;
    return charToByte(src, charOffset + Math.max(0, col - 1));
  }

  // Synthesised trigger actors aren't declared nodes; give them a fixed blurb.
  const ACTOR_DOC = {
    client: { kind: "person", title: "client", body: "An external client calling in over HTTP." },
    scheduler: { kind: "system", title: "scheduler", body: "A scheduled trigger (timer / cron)." },
    caller: { kind: "person", title: "caller", body: "The caller of this operation." },
  };

  // Resolve a possibly depth-collapsed callee FQN to a real node: a direct hit,
  // else a callable named `method` somewhere beneath the collapsed owner (so a
  // call's member still resolves when its target was folded into an ancestor).
  function resolveNodeFqn(fqn) {
    if (nodeIndex.has(fqn)) return fqn;
    const sep = fqn.lastIndexOf("::");
    if (sep < 0) return null;
    const owner = fqn.slice(0, sep);
    const method = fqn.slice(sep + 2);
    for (const n of nodes) {
      if (n.kind !== "callable" || n.name !== method) continue;
      for (let cur = n.parent; cur; cur = nodeInfo[cur]?.parent ?? null) {
        if (cur === owner) return n.fqn;
      }
    }
    return null;
  }

  // A symbol's { kind, title, body } documentation via the editor hover, or null.
  function docFor(fqn) {
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

  function showCanvasInfo(fqn, e) {
    const at = { fqn, x: e.clientX, y: e.clientY };
    if (ACTOR_DOC[fqn]) {
      canvasInfo = { ...ACTOR_DOC[fqn], ...at };
      return;
    }
    // An `event:<fqn>` actor documents the event node it names.
    const real = fqn.startsWith("event:") ? fqn.slice(6) : resolveNodeFqn(fqn);
    const doc = real ? docFor(real) : null;
    canvasInfo = doc
      ? { ...doc, fqn: real, x: e.clientX, y: e.clientY }
      : fqn.startsWith("event:")
        ? { kind: "system", title: fqn.slice(6), body: "Triggered by this event.", ...at }
        : null;
  }
  const hideCanvasInfo = () => (canvasInfo = null);

  function showCanvasUsages(fqn, e) {
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
    canvasUsages = { name: refs.fqn.split("::").at(-1), items: refs.occurrences, x: e.clientX, y: e.clientY };
  }
  function pickCanvasUsage(occ) {
    canvasUsages = null;
    openUsage(occ);
  }

  // A node's display title (for the breadcrumb): its kind + simple name, falling
  // back to the FQN's last segment.
  function nodeTitle(fqn) {
    const n = nodes.find((x) => x.fqn === fqn);
    return n ? `${n.kind} \`${n.name}\`` : `\`${fqn.split("::").at(-1)}\``;
  }

  // Select a structure node: open its declaring file and remember it as the
  // current scope. `goto` (a nav click) also shows the code and jumps the editor
  // to the declaration; a canvas drill leaves the view alone.
  function selectNode(fqn, { goto = false } = {}) {
    const hit = nodeIndex.get(fqn);
    if (!hit) return;
    const file = workspace?.files.find((f) => f.fqn === hit.fileFqn);
    if (!file) return;
    if (openFile?.fqn !== file.fqn) {
      clearTimeout(saveTimer);
      openFile = file;
    }
    selected = { fqn, line: hit.node.line, col: hit.node.col, fileFqn: file.fqn };
    // A nav click jumps the editor to the declaration — but only when the canvas
    // isn't showing; on the canvas the new scope is the navigation, so stay put.
    if (goto && view !== "canvas") {
      view = "code";
      pendingGoto = { line: hit.node.line, col: hit.node.col, fileFqn: file.fqn };
      recordLocation({ fileFqn: file.fqn, line: hit.node.line, col: hit.node.col, fqn, label: nodeTitle(fqn) });
    }
  }

  // Clicking a node in the canvas drills the selection into it (staying on the
  // canvas); synthetic initiators (client, scheduler, …) aren't declared nodes.
  const pickNode = (fqn) => selectNode(fqn);
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

  // The structural ancestor chain for a node (root system → … → the node), by
  // following `parent` pointers. Drives the breadcrumb.
  function ancestry(fqn) {
    const chain = [];
    const seen = new Set();
    let cur = fqn;
    while (cur && nodeIndex.has(cur) && !seen.has(cur)) {
      seen.add(cur);
      chain.unshift(cur);
      cur = nodeIndex.get(cur).node.parent ?? null;
    }
    return chain;
  }

  // The editor's hover popover: reveal the symbol's diagram on the canvas.
  function revealSymbol(info) {
    if (!nodeIndex.has(info.fqn)) return;
    selectNode(info.fqn);
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
    wasmError = null;
    try {
      await initWasm();
    } catch (e) {
      wasmError = String(e?.message ?? e);
      return;
    }
    ver = version();
    refreshRecents();
    ready = true;
    projectOpen = true; // open the project panel on start — never autoload a model
  }
  onMount(boot);

  // Swap in a freshly-loaded workspace, resetting navigation to `landing`.
  function mountWorkspace(ws, landing) {
    clearTimeout(saveTimer);
    workspace = ws;
    openFile = ws.files.find((f) => f.fqn === landing) ?? ws.files[0] ?? null;
    selected = null;
    pendingGoto = null;
    view = "code";
    history = [];
    histIndex = -1;
    projectOpen = false;
    docGroups = [];
    docSources = {};
    docMeta = {};
    loadWorkspaceDocs(ws); // async: populates the Documentation section when ready
  }

  // Parses the workspace `[doc]` manifest and loads its `[[doc.sidebar]]` pages
  // (from disk for a folder, from the bundled map for a sample) into the doc
  // state the FileTree and the site build read.
  let docLoadSeq = 0;
  async function loadWorkspaceDocs(ws) {
    const seq = (docLoadSeq += 1);
    if (!ws.manifestToml) return;
    let manifest;
    try {
      manifest = docManifest(ws.manifestToml);
    } catch {
      return; // malformed pds.toml — the auto docs still build
    }
    const groups = ws.root
      ? await readDocPages(ws.root, ws.base, manifest.sidebar)
      : sampleDocPages(manifest.sidebar, ws.docs ?? {});
    // A later workspace may have mounted while we awaited; ignore a stale load.
    if (seq !== docLoadSeq) return;
    const sources = {};
    for (const g of groups) for (const it of g.items) sources[it.path] = it.content;
    docSources = sources;
    docMeta = { name: manifest.name, theme: manifest.theme };
    docGroups = groups.map((g) => ({
      title: g.title,
      items: g.items.map(({ title, path, handle }) => ({ title, path, handle: handle ?? null })),
    }));
  }

  // Load a bundled example (edits are session-only). Called from the project
  // panel's examples block.
  function openSample(id) {
    const loaded = loadSample(id);
    if (!loaded) return;
    moduleSources = Object.fromEntries(loaded.workspace.files.map((f) => [f.fqn, f.source]));
    mountWorkspace(loaded.workspace, loaded.landing);
    recordSample(SAMPLES.find((s) => s.id === id));
    refreshRecents();
  }

  // Re-open a recent project: a sample by id, or a folder from its stored handle
  // (falling back to the picker if the handle is gone or permission is denied).
  async function openRecent(entry) {
    if (entry.kind === "sample") {
      openSample(entry.sampleId);
      return;
    }
    const handle = await reopenFolder(entry.key);
    if (!handle) {
      openFolder();
      return;
    }
    try {
      const ws = await readWorkspace(handle);
      const sources = {};
      for (const file of ws.files) sources[file.fqn] = await readFile(file.handle);
      moduleSources = sources;
      mountWorkspace(ws, ws.files[0]?.fqn);
      await recordFolder(ws.name, ws.root);
      refreshRecents();
      flash(`Opened ${ws.name} · ${ws.files.length} modules`);
    } catch {
      openFolder();
    }
  }

  function forgetRecent(entry) {
    forget(entry.key);
    refreshRecents();
  }

  let saveTimer;
  let toastTimer;
  function flash(message) {
    toast = message;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), 2400);
  }

  // Toast notifications (kind: success | error | info), shown stacked top-right.
  let notes = $state([]);
  let noteSeq = 0;
  function notify(kind, title, body = "") {
    const id = (noteSeq += 1);
    notes = [...notes, { id, kind, title, body }];
    setTimeout(() => dismissNote(id), kind === "error" ? 9000 : 6000);
  }
  function dismissNote(id) {
    notes = notes.filter((n) => n.id !== id);
  }

  function scheduleSave(handle, text) {
    if (!handle) return; // in-memory sample: session-only
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => writeFile(handle, text).catch(() => flash("Could not save to disk")), 400);
  }

  function onEditorChange(value) {
    if (!openFile) return;
    if (openFile.isDoc) {
      docSources = { ...docSources, [openFile.path]: value };
    } else {
      moduleSources = { ...moduleSources, [openFile.fqn]: value };
    }
    scheduleSave(openFile.handle, value);
  }

  // Opening a file from the nav clears any node scope; it shows the source,
  // unless the canvas is up — then it stays on the canvas (whole-model context).
  function selectFile(file) {
    clearTimeout(saveTimer);
    openFile = file;
    selected = null;
    if (view !== "canvas") view = "code";
  }

  // Open an authored doc page (`[[doc.sidebar]]`) as raw Markdown in the editor.
  // Marked `isDoc` so the editor drops PseudoScript language features and edits
  // route to `docSources` (and save to the page's handle on a real folder).
  function openDoc(item) {
    clearTimeout(saveTimer);
    openFile = { isDoc: true, path: item.path, title: item.title, handle: item.handle ?? null };
    selected = null;
    view = "code";
  }

  async function onProblemPick(d) {
    view = "code";
    if (d.file && workspace && d.file !== openFile?.fqn) {
      const f = workspace.files.find((x) => x.fqn === d.file);
      if (f) selectFile(f);
    }
    await tick();
    editorApi?.goto(d.start_line, d.start_col);
    if (d.file) recordLocation({ fileFqn: d.file, line: d.start_line, col: d.start_col, label: d.message });
  }

  async function openFolder() {
    try {
      const ws = await openWorkspace();
      const sources = {};
      for (const file of ws.files) sources[file.fqn] = await readFile(file.handle);
      moduleSources = sources;
      mountWorkspace(ws, ws.files[0]?.fqn);
      await recordFolder(ws.name, ws.root);
      refreshRecents();
      flash(`Opened ${ws.name} · ${ws.files.length} modules`);
    } catch {
      // picker cancelled or permission denied — keep the current workspace
    }
  }

  function onformat() {
    if (!openFile) return;
    if (openFile.isDoc) return; // the PseudoScript formatter doesn't apply to Markdown
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
  function buildDocConfig() {
    return {
      name: docMeta.name ?? workspace.name,
      theme: docMeta.theme ?? "dark",
      docs: docGroups.map((g) => ({
        title: g.title,
        items: g.items.map((i) => ({ title: i.title, path: i.path, content: docSources[i.path] ?? "" })),
      })),
    };
  }

  // Folds the bundled sample Markdown into the manifest sidebar, dropping any
  // page with no bundled content (mirrors the folder path's warn-and-skip).
  function sampleDocPages(sidebar, docMap) {
    return (sidebar ?? []).map((group) => ({
      title: group.title,
      items: (group.items ?? [])
        .filter((item) => docMap[item.path] != null)
        .map((item) => ({ ...item, content: docMap[item.path] })),
    }));
  }

  // Renders the site, then writes it to `target/doc/` (opened folder) or opens a
  // preview (example), reporting the outcome as a notification.
  async function runBuild() {
    building = true;
    try {
      const config = buildDocConfig();
      const files = renderDocSite(allModules, config);
      if (workspace.root) {
        const dir = await writeSite(workspace.root, files);
        notify(
          "success",
          "Documentation built",
          `Wrote ${files.length} files to ${dir}/ in “${workspace.name}”. Open ${dir}/index.html to view it.`,
        );
      } else {
        previewSite(files);
        notify("success", `Preview built (${files.length} files)`, "Opened a read-only preview in a new tab.");
      }
    } catch (e) {
      notify("error", "Documentation build failed", String(e?.message ?? e));
    } finally {
      building = false;
    }
  }

  // No folder to write to (the bundled sample): open index.html in a new tab,
  // with style.css inlined since its relative href can't resolve from a blob.
  function previewSite(files) {
    const byPath = Object.fromEntries(files.map((f) => [f.path, f.contents]));
    const css = byPath["style.css"] ?? "";
    const html = (byPath["index.html"] ?? "").replace(
      /<link[^>]*href="[^"]*style\.css"[^>]*>/,
      `<style>${css}</style>`,
    );
    const url = URL.createObjectURL(new Blob([html], { type: "text/html" }));
    window.open(url, "_blank");
    setTimeout(() => URL.revokeObjectURL(url), 60_000);
  }

</script>

<svelte:head><title>PseudoScript Web IDE</title></svelte:head>

<svelte:window onkeydown={(e) => {
  if (e.key === "Escape") {
    if (buildNotice) buildNotice = false;
    if (projectOpen && workspace) projectOpen = false;
    canvasInfo = null;
    canvasUsages = null;
  }
}} />

<Notifications {notes} ondismiss={dismissNote} />

{#if ready && projectOpen}
  <ProjectPanel
    examples={SAMPLES}
    {recents}
    canOpenFolder={fsSupported}
    dismissible={!!workspace}
    onpicksample={openSample}
    onpickrecent={openRecent}
    onopenfolder={openFolder}
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

{#snippet symbolLabel(title)}
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
    {onformat}
    onproject={() => (projectOpen = true)}
    {onbuilddocs}
    onopensettings={() => (settingsOpen = true)}
  />

  {#if wasmError}
    <div class="curtain">
      <div class="kicker">compiler failed to load</div>
      <p class="msg">{wasmError}</p>
      <button class="retry" onclick={boot}>Retry</button>
    </div>
  {:else if ready && workspace}
    <main class="workspace has-tree">
      <section class="pane tree-pane reveal r1">
        <FileTree
          workspaceName={workspace.name}
          files={workspace.files}
          openPath={openFile?.path ?? null}
          {docGroups}
          ondocopen={openDoc}
          {symbols}
          selectedFqn={selected?.fqn ?? null}
          {errorPaths}
          onopen={selectFile}
          onpicknode={(fqn) => selectNode(fqn, { goto: true })}
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
          <div class="layer" class:hidden={view !== "code"} data-doc-width={docWidth}>
            <Editor
              value={source}
              onchange={onEditorChange}
              onready={(api) => (editorApi = api)}
              modules={allModules}
              moduleFqn={openFile?.fqn ?? ""}
              plain={openFile?.isDoc ?? false}
              markdown={openFile?.isDoc ?? false}
              {symbols}
              onopensymbol={revealSymbol}
              ongotodefinition={(fqn) => selectNode(fqn, { goto: true })}
              onnavigate={openUsage}
              {onformat}
              onopensettings={() => (settingsOpen = true)}
            />
          </div>
          {#if view === "canvas"}
            <div class="layer canvas-layer">
              <DiagramPane scene={canvas.scene} layout={canvas.layout} error={canvas.error} hint={canvasHint} onpick={pickNode} onup={navigateUp} flows={flowsByNode} depth={seqDepth} ondepth={(d) => (seqDepth = d)} oninfo={showCanvasInfo} oninfoend={hideCanvasInfo} onusages={showCanvasUsages} typeFqn={typeFqnByName} />
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
      <span class="seg">{openFile?.fqn ?? "—"}</span>
      <span class="seg dim">{workspace.files.length} modules</span>
    {/if}
    <span class="grow"></span>
    {#if toast}<span class="seg toast">{toast}</span>{/if}
    <span class="seg dim">{view}</span>
    <span class="seg dim">{selected?.fqn ?? "context"}</span>
  </footer>
</div>

<style>
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
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.5);
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
    box-shadow: 0 10px 30px -10px rgba(0, 0, 0, 0.7);
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
