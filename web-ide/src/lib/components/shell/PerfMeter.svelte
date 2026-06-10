<script lang="ts">
  // A live frame-rate + JS-heap readout for the status bar. FPS is measured from this
  // component's own rAF cadence — which tracks the whole app's render loop, so a heavy
  // 3D view shows up here. Heap is Chrome's non-standard `performance.memory`; absent
  // elsewhere, the memory readout is simply hidden.
  import { onMount } from "svelte";

  let fps = $state(0);
  let memMb = $state<number | null>(null);

  type WithMemory = Performance & { memory?: { usedJSHeapSize: number } };

  onMount(() => {
    let raf = 0;
    let frames = 0;
    let last = performance.now();
    const tick = () => {
      frames++;
      const now = performance.now();
      if (now - last >= 500) {
        // ~2 updates/sec so the number is readable, not jittery.
        fps = Math.round((frames * 1000) / (now - last));
        frames = 0;
        last = now;
        const used = (performance as WithMemory).memory?.usedJSHeapSize;
        memMb = used != null ? Math.round(used / 1048576) : null;
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  });

  // Colour the frame rate by health (60 is smooth; below 30 is visibly choppy).
  const tone = $derived(fps >= 50 ? "good" : fps >= 30 ? "warn" : "bad");
</script>

<div class="perf" data-testid="perf-meter" title="Frame rate{memMb != null ? ' · JS heap used' : ''}">
  <span class="fps {tone}">{fps} fps</span>
  {#if memMb != null}<span class="mem">{memMb} MB</span>{/if}
</div>

<style>
  .perf {
    margin-left: auto; /* sits at the far right of the status bar */
    display: inline-flex;
    align-items: center;
    gap: 0.6rem;
    font-variant-numeric: tabular-nums; /* fixed width — no jitter as digits change */
  }
  .fps.good { color: var(--ok, #4ade80); }
  .fps.warn { color: var(--warn, #fbbf24); }
  .fps.bad { color: var(--err, #f87171); }
  .mem { color: var(--ink-faint); }
</style>
