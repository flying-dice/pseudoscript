<script>
  // Client-only: an animated code-flow timeline. As the sequence plays, the
  // entries it touches appear as graph nodes and reposition as the active set
  // grows; the current call draws an animated edge, past calls dim. A scrubber
  // and play/pause drive the cursor.
  import { onDestroy } from "svelte";
  import { SvelteFlow, Background } from "@xyflow/svelte";

  let { scene } = $props();

  const leaf = (fqn) => fqn.split("::").pop();

  // Flatten ordered items (messages + alt/loop frames) into linear steps,
  // carrying any enclosing frame label for context.
  function flatten(items, frame, out) {
    for (const item of items) {
      if (item.Message) out.push({ ...item.Message, frame });
      else if (item.Frame) {
        flatten(item.Frame.body, `${item.Frame.kind} ${item.Frame.cond}`, out);
      }
    }
    return out;
  }
  const steps = flatten(scene.items, null, []);
  const kindOf = new Map(scene.participants.map((p) => [p.fqn, p.kind]));

  const COL = 232;
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
    timer = setInterval(() => {
      if (cursor >= steps.length) stop();
      else cursor += 1;
    }, 1100);
  }
  const toggle = () => (playing ? stop() : play());
  onDestroy(stop);

  // The entries visible so far: the entry, then every endpoint the played steps
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
      position: { x: i * COL + 30, y: 70 },
      data: { label: leaf(fqn) },
      class: `c4-node ${kindOf.get(fqn) ?? "component"}`,
      width: NODE_W,
      height: 58,
      selectable: false,
      draggable: false,
    }));
    edges = steps.slice(0, cursor).map((step, i) => ({
      id: `m${i}`,
      source: step.from,
      target: step.to,
      label: step.label || undefined,
      type: "smoothstep",
      animated: i === cursor - 1,
      class: `flow-edge ${i === cursor - 1 ? "current" : "past"}`,
    }));
  });

  const current = $derived(cursor > 0 ? steps[cursor - 1] : null);
</script>

<div class="timeline">
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
      <Background gap={22} />
    </SvelteFlow>
  </div>
  <div class="timeline-bar">
    <button class="play" onclick={toggle} aria-label={playing ? "Pause" : "Play"}>
      {playing ? "❚❚" : "▶"}
    </button>
    <input
      class="scrub"
      type="range"
      min="0"
      max={steps.length}
      bind:value={cursor}
      oninput={stop}
      aria-label="Scrub the flow"
    />
    <span class="step-count">{cursor} / {steps.length}</span>
    <span class="step-label">
      {#if current}
        {leaf(current.from)} <span class="arrow">&rarr;</span> {leaf(current.to)}{#if current.label}
          · {current.label}{/if}
        {#if current.frame}<em class="frame">[{current.frame}]</em>{/if}
      {:else}
        entry · {leaf(scene.entry)}
      {/if}
    </span>
  </div>
</div>
