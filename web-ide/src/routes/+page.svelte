<script>
  import { onMount, tick } from "svelte";
  import "../app.css";
  import { checkModules, emitScene, format as formatSource, initWasm, outline, version } from "$lib/pds.js";
  import { fsSupported, openWorkspace, readFile, writeFile } from "$lib/workspace.js";
  import { SAMPLE_WORKSPACE } from "$lib/sample-workspace.js";
  import Editor from "$lib/components/Editor.svelte";
  import Toolbar from "$lib/components/Toolbar.svelte";
  import FileTree from "$lib/components/FileTree.svelte";
  import DiagramPane from "$lib/components/DiagramPane.svelte";
  import ProblemsPane from "$lib/components/ProblemsPane.svelte";

  let ready = $state(false);
  let wasmError = $state(null);
  let ver = $state("");
  let view = $state("container");
  let target = $state("");
  let tab = $state("diagram");
  let toast = $state(null);
  let editorApi = $state(null);

  // Workspace state. Defaults to the bundled sample (in-memory, handles null);
  // "Open folder" swaps in a real on-disk workspace whose files persist on edit.
  let workspace = $state(null);
  let openFile = $state(null);
  let moduleSources = $state({});

  const source = $derived(openFile ? (moduleSources[openFile.fqn] ?? "") : "");

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
    if (!ready) return [];
    try {
      return outline(source);
    } catch {
      return [];
    }
  });
  const targets = $derived.by(() => {
    if (view === "container") return nodes.filter((n) => n.kind === "system");
    if (view === "component") return nodes.filter((n) => n.kind === "container");
    if (view === "sequence") return nodes.filter((n) => n.triggered);
    return [];
  });

  $effect(() => {
    if (view === "context") {
      if (target !== "") target = "";
      return;
    }
    const fqns = targets.map((t) => t.fqn);
    if (!fqns.includes(target)) target = fqns[0] ?? "";
  });

  const diagram = $derived.by(() => {
    if (!ready || !openFile) return { scene: null, error: "" };
    if (view !== "context" && !target) return { scene: null, error: "" }; // no target chosen yet
    try {
      return { scene: emitScene(source, view, target), error: "" };
    } catch (e) {
      return { scene: null, error: String(e?.message ?? e) };
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
    moduleSources = Object.fromEntries(SAMPLE_WORKSPACE.files.map((f) => [f.fqn, f.source]));
    workspace = { name: SAMPLE_WORKSPACE.name, root: null, files: SAMPLE_WORKSPACE.files };
    const files = SAMPLE_WORKSPACE.files;
    openFile = files.find((f) => f.fqn === "internet_banking") ?? files[0] ?? null;
    ready = true;
  }
  onMount(boot);

  let saveTimer;
  let toastTimer;
  function flash(message) {
    toast = message;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), 2400);
  }

  function scheduleSave(handle, text) {
    if (!handle) return; // in-memory sample: session-only
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => writeFile(handle, text).catch(() => flash("Could not save to disk")), 400);
  }

  function onEditorChange(value) {
    if (!openFile) return;
    moduleSources = { ...moduleSources, [openFile.fqn]: value };
    scheduleSave(openFile.handle, value);
  }

  function selectFile(file) {
    clearTimeout(saveTimer);
    openFile = file;
  }

  async function onProblemPick(d) {
    if (d.file && workspace && d.file !== openFile?.fqn) {
      const f = workspace.files.find((x) => x.fqn === d.file);
      if (f) {
        selectFile(f);
        await tick();
      }
    }
    editorApi?.goto(d.start_line, d.start_col);
  }

  async function openFolder() {
    try {
      const ws = await openWorkspace();
      const sources = {};
      for (const file of ws.files) sources[file.fqn] = await readFile(file.handle);
      moduleSources = sources;
      workspace = ws;
      openFile = ws.files[0] ?? null;
      flash(`Opened ${ws.name} · ${ws.files.length} modules`);
    } catch {
      // picker cancelled or permission denied — keep the current workspace
    }
  }

  function onformat() {
    if (!openFile) return;
    try {
      onEditorChange(formatSource(source));
    } catch {
      flash("Cannot format — fix syntax errors first");
    }
  }

  const hasTargets = $derived(view === "context" || targets.length > 0);
</script>

<svelte:head><title>PseudoScript Web IDE</title></svelte:head>

<div class="app">
  <Toolbar
    bind:view
    bind:target
    {targets}
    {errorCount}
    workspaceName={workspace?.name ?? null}
    canOpenFolder={fsSupported}
    {onformat}
    onopenfolder={openFolder}
  />

  {#if wasmError}
    <div class="curtain">
      <div class="kicker">compiler failed to load</div>
      <p class="msg">{wasmError}</p>
      <button class="retry" onclick={boot}>Retry</button>
    </div>
  {:else if ready}
    <main class="workspace" class:has-tree={!!workspace}>
      {#if workspace}
        <section class="pane tree-pane reveal r1">
          <FileTree
            workspaceName={workspace.name}
            files={workspace.files}
            openPath={openFile?.path ?? null}
            {errorPaths}
            onopen={selectFile}
          />
        </section>
      {/if}

      <section class="pane editor-pane reveal r2">
        <Editor value={source} onchange={onEditorChange} onready={(api) => (editorApi = api)} />
      </section>

      <section class="pane preview-pane reveal r3">
        <div class="tabs" role="tablist">
          <button role="tab" aria-selected={tab === "diagram"} class:active={tab === "diagram"} onclick={() => (tab = "diagram")}>
            Diagram
          </button>
          <button role="tab" aria-selected={tab === "problems"} class:active={tab === "problems"} onclick={() => (tab = "problems")}>
            Problems{#if problems.length}<span class="count" class:bad={errorCount > 0}>{problems.length}</span>{/if}
          </button>
        </div>
        <div class="tab-body">
          {#if tab === "diagram"}
            <DiagramPane scene={diagram.scene} error={diagram.error} {view} {hasTargets} />
          {:else}
            <ProblemsPane diagnostics={problems} onpick={onProblemPick} />
          {/if}
        </div>
      </section>
    </main>
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
    <span class="seg dim">{view}{target ? ` · ${target.split("::").pop()}` : ""}</span>
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
    grid-template-columns: minmax(0, 1.15fr) minmax(0, 1fr);
    min-height: 0;
  }
  .workspace.has-tree {
    grid-template-columns: 248px minmax(0, 1.12fr) minmax(0, 1fr);
  }
  .pane { min-width: 0; min-height: 0; }
  .tree-pane {
    border-right: 1px solid var(--line);
    background: color-mix(in srgb, var(--surface) 70%, transparent);
  }
  .editor-pane {
    border-right: 1px solid var(--line);
    background: color-mix(in srgb, var(--surface) 60%, transparent);
  }
  .preview-pane {
    display: grid;
    grid-template-rows: auto 1fr;
    min-height: 0;
  }

  /* one orchestrated staggered reveal on load */
  .reveal { animation: rise 0.5s cubic-bezier(0.2, 0.7, 0.2, 1) both; }
  .r1 { animation-delay: 0.02s; }
  .r2 { animation-delay: 0.09s; }
  .r3 { animation-delay: 0.16s; }

  .tabs {
    display: flex;
    gap: 0.1rem;
    padding: 0 0.6rem;
    border-bottom: 1px solid var(--line);
    background: var(--surface);
  }
  .tabs button {
    position: relative;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    background: transparent;
    border: none;
    color: var(--ink-faint);
    font-family: var(--font-mono);
    font-size: 0.72rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 0.7rem 0.8rem;
  }
  .tabs button::after {
    content: "";
    position: absolute;
    left: 0.8rem;
    right: 0.8rem;
    bottom: -1px;
    height: 2px;
    background: var(--accent);
    transform: scaleX(0);
    transform-origin: left;
    transition: transform 0.2s ease;
  }
  .tabs button:hover { color: var(--ink); }
  .tabs button.active { color: var(--ink); }
  .tabs button.active::after { transform: scaleX(1); }
  .tabs .count {
    font-size: 0.62rem;
    background: var(--surface-3);
    color: var(--ink-soft);
    padding: 0.05rem 0.4rem;
    border-radius: 999px;
  }
  .tabs .count.bad { background: color-mix(in srgb, var(--err) 18%, transparent); color: var(--err); }
  .tab-body { min-height: 0; }

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
</style>
