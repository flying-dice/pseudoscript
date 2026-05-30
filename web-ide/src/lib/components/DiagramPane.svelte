<script>
  // The system, shown live. Structure views (context/container/component) render
  // an interactive C4 graph; the flow view renders the animated timeline. Both
  // are fed the compiler's laid-out scene (emit_scene), so the pane is the
  // model itself, not a picture of it.
  import C4Flow from "./C4Flow.svelte";
  import FlowTimeline from "./FlowTimeline.svelte";

  let { scene = null, error = "", view = "context", hasTargets = true } = $props();

  const isFlow = $derived(view === "sequence");
  const hasC4 = $derived(!!scene && Array.isArray(scene.nodes) && scene.nodes.length > 0);
  const hasFlow = $derived(!!scene && Array.isArray(scene.participants) && scene.participants.length > 0);
  const ready = $derived(isFlow ? hasFlow : hasC4);
  // Remount the flow when the scene content changes so layout/animation reset.
  const sig = $derived(scene ? JSON.stringify(scene) : "");

  const hint = $derived(
    view === "context"
      ? "No persons or systems declared in this module — the context view draws systems and people."
      : !hasTargets
        ? isFlow
          ? "No triggered entry points in this module. Mark a callable with #[http], #[schedule], or #[manual] to trace its flow."
          : `No ${view === "component" ? "containers" : "systems"} in this module for a ${view} view.`
        : "Nothing to draw for this selection.",
  );
</script>

<div class="stage" class:framed={ready}>
  {#if error}
    <div class="note error">
      <span class="kicker">cannot project</span>
      <p>{error}</p>
    </div>
  {:else if ready}
    {#key sig}
      {#if isFlow}<FlowTimeline {scene} />{:else}<C4Flow {scene} />{/if}
    {/key}
  {:else}
    <div class="note">
      <span class="kicker">{isFlow ? "flow" : view} view</span>
      <p>{hint}</p>
    </div>
  {/if}
</div>

<style>
  .stage {
    height: 100%;
    min-height: 0;
    background:
      radial-gradient(900px 520px at 60% -10%, color-mix(in srgb, var(--accent) 6%, transparent), transparent 70%),
      var(--bg);
  }
  /* centre the placeholder notes; the flow components fill the stage themselves */
  .stage:not(.framed) {
    display: grid;
    place-items: center;
    padding: 1.6rem;
  }
  .note { max-width: 30rem; text-align: center; color: var(--ink-soft); }
  .note .kicker {
    display: inline-block;
    font-family: var(--font-mono);
    font-size: 0.6rem;
    font-weight: 600;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--ink-faint);
    margin-bottom: 0.6rem;
  }
  .note.error .kicker { color: var(--err); }
  .note p { margin: 0; font-family: var(--font-mono); font-size: 0.82rem; line-height: 1.7; }
  .note.error p { color: var(--err); }
</style>
