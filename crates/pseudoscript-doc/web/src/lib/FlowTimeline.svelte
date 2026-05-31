<script>
  // The animated code-flow timeline — the behavioural lens. A triggered entry
  // point's call sequence plays out: the components it touches appear and
  // reposition as the request travels, the current call is highlighted, past
  // calls dim. Built to read a journey, not code. (Svelte Flow's CSS is pulled
  // in by the global stylesheet.)
  import { onDestroy } from "svelte";
  import { Background, SvelteFlow } from "@xyflow/svelte";

  let { scene } = $props();

  const leaf = (fqn) => fqn.split("::").pop();

  // A message's display label. A marked return carries its payload as a generic
  // argument of the marker (`Ok<Order>`, `Err<Rejected>`); a bare value return
  // shows its whole type; calls and self-messages show their method name.
  function msgLabel(m) {
    if (!m) return undefined;
    if (m.kind === "return") {
      if (!m.detail) return m.label || "return";
      if (!m.label) return `return ${m.detail}`;
      return `${m.label}<${m.detail}>`;
    }
    return m.label || undefined;
  }

  // Flatten ordered items (messages + alt/loop frames) into linear steps,
  // carrying any enclosing frame label for context.
  function flatten(items, frame, out) {
    for (const item of items) {
      if (item.Message) out.push({ ...item.Message, frame });
      else if (item.Frame) flatten(item.Frame.body, `${item.Frame.kind} ${item.Frame.cond}`, out);
    }
    return out;
  }
  const steps = flatten(scene.items, null, []);
  const kindOf = new Map(scene.participants.map((p) => [p.fqn, p.kind]));

  const COL = 230;
  const NODE_W = 168;

  let cursor = $state(0); // 0 = entry only; n = first n calls played
  let playing = $state(false);
  let timer = null;

  function stop() {
    if (timer) clearInterval(timer);
    timer = null;
    playing = false;
  }
  function play() {
    if (cursor >= steps.length) cursor = 0;
    playing = true;
    timer = setInterval(() => (cursor >= steps.length ? stop() : (cursor += 1)), 1150);
  }
  const toggle = () => (playing ? stop() : play());
  const step = (delta) => {
    stop();
    cursor = Math.max(0, Math.min(steps.length, cursor + delta));
  };
  onDestroy(stop);

  // Entries visible so far: the entry, then every endpoint the played steps
  // touch, in first-appearance order.
  function visibleSet(n) {
    const seen = [];
    const add = (fqn) => {
      if (!seen.includes(fqn)) seen.push(fqn);
    };
    add(scene.entry);
    for (let i = 0; i < n; i++) {
      add(steps[i].from);
      add(steps[i].to);
    }
    return seen;
  }

  let nodes = $state([]);
  let edges = $state([]);

  $effect(() => {
    const visible = visibleSet(cursor);
    nodes = visible.map((fqn, i) => ({
      id: fqn,
      position: { x: i * COL + 30, y: 80 },
      data: { label: leaf(fqn), kind: kindOf.get(fqn) ?? "component" },
      class: `c4-node ${kindOf.get(fqn) ?? "component"}`,
      width: NODE_W,
      height: 58,
      selectable: false,
      draggable: false,
    }));
    edges = steps.slice(0, cursor).map((s, i) => ({
      id: `m${i}`,
      source: s.from,
      target: s.to,
      label: msgLabel(s),
      type: "smoothstep",
      animated: i === cursor - 1,
      class: `flow-edge ${i === cursor - 1 ? "current" : "past"}`,
    }));
  });

  const current = $derived(cursor > 0 ? steps[cursor - 1] : null);
  const done = $derived(cursor >= steps.length && steps.length > 0);
</script>

<div class="timeline">
  <header class="flow-head">
    <div class="title">
      <span class="kicker">flow</span>
      <span class="name">{leaf(scene.entry)}</span>
    </div>
    <div class="narration">
      {#if current}
        <span class="chip {kindOf.get(current.from) ?? 'component'}">{leaf(current.from)}</span>
        <span class="arrow">&rarr;</span>
        <span class="chip {kindOf.get(current.to) ?? 'component'}">{leaf(current.to)}</span>
        {#if msgLabel(current)}<span class="method">{msgLabel(current)}</span>{/if}
        {#if current.frame}<span class="frame">{current.frame}</span>{/if}
      {:else}
        <span class="muted">triggered entry point — press play to trace the request</span>
      {/if}
    </div>
  </header>

  <div class="flow">
    <SvelteFlow
      bind:nodes
      bind:edges
      fitView
      minZoom={0.2}
      maxZoom={2}
      nodesDraggable={false}
      proOptions={{ hideAttribution: true }}
    >
      <Background gap={24} />
    </SvelteFlow>
  </div>

  <div class="transport">
    <button class="ctrl" onclick={() => step(-1)} disabled={cursor === 0} aria-label="Previous step">◀</button>
    <button class="play" onclick={toggle} aria-label={playing ? "Pause" : "Play"}>
      {playing ? "❚❚" : done ? "↺" : "▶"}
    </button>
    <button class="ctrl" onclick={() => step(1)} disabled={cursor >= steps.length} aria-label="Next step">▶</button>
    <input class="scrub" type="range" min="0" max={steps.length} bind:value={cursor} oninput={stop} aria-label="Scrub the flow" />
    <span class="count">{cursor}/{steps.length}</span>
  </div>
</div>

<style>
  .timeline {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .flow-head {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.7rem 1rem;
    border-bottom: 1px solid var(--line);
    background: var(--surface);
  }
  .title { display: flex; align-items: baseline; gap: 0.5rem; flex: none; }
  .title .kicker {
    font-family: var(--font-mono);
    font-size: 0.56rem;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .title .name {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 1rem;
    color: var(--ink);
  }
  .narration {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    min-width: 0;
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: 0.78rem;
  }
  .narration .chip {
    padding: 0.1rem 0.45rem;
    border-radius: 5px;
    color: var(--ink);
    background: var(--surface-3);
    border-left: 2px solid var(--k, var(--ink-faint));
    white-space: nowrap;
  }
  .narration .chip.person { --k: var(--k-person); }
  .narration .chip.system { --k: var(--k-system); }
  .narration .chip.container { --k: var(--k-container); }
  .narration .chip.component { --k: var(--k-component); }
  .narration .chip.data { --k: var(--k-data); }
  .narration .chip.callable { --k: var(--k-callable); }
  .narration .arrow { color: var(--accent); }
  .narration .method { color: var(--ink-soft); }
  .narration .frame {
    margin-left: 0.3rem;
    padding: 0.05rem 0.4rem;
    border: 1px dashed var(--line-strong);
    border-radius: 4px;
    color: var(--ink-faint);
    font-size: 0.7rem;
  }
  .narration .muted { color: var(--ink-faint); }

  .flow { flex: 1; min-height: 0; }

  .transport {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    padding: 0.55rem 1rem;
    border-top: 1px solid var(--line);
    background: var(--surface);
  }
  .transport .ctrl,
  .transport .play {
    display: grid;
    place-items: center;
    border: 1px solid var(--line-strong);
    background: var(--surface-2);
    color: var(--ink-soft);
    border-radius: 6px;
  }
  .transport .ctrl { width: 1.8rem; height: 1.8rem; font-size: 0.7rem; }
  .transport .ctrl:disabled { opacity: 0.35; cursor: not-allowed; }
  .transport .ctrl:hover:not(:disabled) { border-color: var(--accent); color: var(--ink); }
  .transport .play {
    width: 2.1rem;
    height: 2.1rem;
    font-size: 0.75rem;
    color: var(--accent-ink);
    background: var(--accent);
    border-color: var(--accent);
  }
  .transport .play:hover { background: var(--accent-hi); }
  .transport .scrub { flex: 1; accent-color: var(--accent); cursor: pointer; }
  .transport .count {
    flex: none;
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: var(--ink-faint);
    min-width: 3rem;
    text-align: right;
  }
</style>
