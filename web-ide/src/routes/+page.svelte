<script>
  import { onMount } from "svelte";
  import "../app.css";
  import { check, emitSvg, format as formatSource, initWasm, version } from "$lib/pds.js";
  import { SAMPLE } from "$lib/sample.js";
  import Editor from "$lib/components/Editor.svelte";
  import Toolbar from "$lib/components/Toolbar.svelte";
  import DiagramPane from "$lib/components/DiagramPane.svelte";
  import ProblemsPane from "$lib/components/ProblemsPane.svelte";

  let source = $state(SAMPLE);
  let view = $state("context");
  let target = $state("");
  let ready = $state(false);
  let ver = $state("");
  let tab = $state("diagram");

  const diagnostics = $derived.by(() => {
    if (!ready) return [];
    try {
      return check(source);
    } catch {
      return [];
    }
  });
  const errorCount = $derived(diagnostics.filter((d) => d.severity === "error").length);

  const diagram = $derived.by(() => {
    if (!ready) return { svg: "", error: "" };
    try {
      return { svg: emitSvg(source, view, target), error: "" };
    } catch (e) {
      return { svg: "", error: String(e?.message ?? e) };
    }
  });

  onMount(async () => {
    await initWasm();
    ver = version();
    ready = true;
  });

  function onformat() {
    try {
      source = formatSource(source);
    } catch {
      // unparseable: leave the buffer untouched
    }
  }
</script>

<svelte:head><title>PseudoScript Web IDE</title></svelte:head>

<div class="app">
  <Toolbar bind:view bind:target {errorCount} {onformat} />

  {#if ready}
    <main class="workspace">
      <section class="pane editor-pane">
        <Editor value={source} onchange={(v) => (source = v)} />
      </section>

      <section class="pane preview-pane">
        <div class="tabs">
          <button class:active={tab === "diagram"} onclick={() => (tab = "diagram")}>Diagram</button>
          <button class:active={tab === "problems"} onclick={() => (tab = "problems")}>
            Problems{#if diagnostics.length}<span class="count">{diagnostics.length}</span>{/if}
          </button>
        </div>
        <div class="tab-body">
          {#if tab === "diagram"}
            <DiagramPane svg={diagram.svg} error={diagram.error} />
          {:else}
            <ProblemsPane {diagnostics} />
          {/if}
        </div>
      </section>
    </main>
  {:else}
    <div class="loading">compiling the compiler…</div>
  {/if}

  <footer class="statusbar">
    <span>PseudoScript compiler · wasm{ver ? ` v${ver}` : ""}</span>
    <span class="right">{view}{target ? ` · ${target}` : ""}</span>
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
  .pane { min-width: 0; min-height: 0; }
  .editor-pane { border-right: 1px solid var(--line); }
  .preview-pane {
    display: grid;
    grid-template-rows: auto 1fr;
    min-height: 0;
  }
  .tabs {
    display: flex;
    gap: 0.2rem;
    padding: 0.4rem 0.6rem 0;
    border-bottom: 1px solid var(--line);
    background: var(--surface);
  }
  .tabs button {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--ink-faint);
    font-family: var(--font-mono);
    font-size: 0.74rem;
    letter-spacing: 0.04em;
    padding: 0.5rem 0.7rem;
  }
  .tabs button:hover { color: var(--ink); }
  .tabs button.active {
    color: var(--ink);
    border-bottom-color: var(--accent);
  }
  .tabs .count {
    font-size: 0.62rem;
    background: var(--accent-soft);
    color: var(--accent);
    padding: 0.05rem 0.35rem;
    border-radius: 999px;
  }
  .tab-body { min-height: 0; }
  .loading {
    display: grid;
    place-items: center;
    font-family: var(--font-mono);
    font-size: 0.85rem;
    color: var(--ink-faint);
  }
  .statusbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 1rem;
    border-top: 1px solid var(--line);
    background: var(--surface);
    font-family: var(--font-mono);
    font-size: 0.7rem;
    color: var(--ink-faint);
  }
</style>
