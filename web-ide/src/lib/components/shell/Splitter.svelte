<script lang="ts">
  // A drag handle that overlays the gutter between two islands and resizes the
  // panel on its `side`. `left`/`right` are vertical handles (resize width);
  // `bottom` is a horizontal handle (resize the dock height). Pointer-captured
  // drag (so it tracks past the handle), arrow-key nudge, and double-click-to-
  // reset. The owner clamps and persists the size this reports — the handle only
  // computes deltas.
  type Props = {
    side: "left" | "right" | "bottom";
    width: number;
    min: number;
    max: number;
    onresize: (px: number) => void;
    onreset?: () => void;
    label?: string;
  };

  let { side, width, min, max, onresize, onreset, label = "Resize panel" }: Props = $props();

  const NUDGE = 8;
  const axis = $derived(side === "bottom" ? "y" : "x");
  // Drag sign: moving right grows a left panel; moving right/down shrinks a
  // right/bottom one.
  const sign = $derived(side === "left" ? 1 : -1);

  let dragging = $state(false);
  let start = 0;
  let startW = 0;

  // Mirror the drag onto <body> so the resize cursor + selection lock apply
  // everywhere (see `body.resizing` in app.css). The cleanup guarantees the
  // class clears even if this handle unmounts mid-drag.
  $effect(() => {
    if (!dragging) return;
    document.body.classList.add("resizing");
    return () => document.body.classList.remove("resizing");
  });

  function down(e: PointerEvent) {
    dragging = true;
    start = axis === "y" ? e.clientY : e.clientX;
    startW = width;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    e.preventDefault();
  }

  function move(e: PointerEvent) {
    if (!dragging) return;
    const pos = axis === "y" ? e.clientY : e.clientX;
    onresize(startW + sign * (pos - start));
  }

  function up(e: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
  }

  function key(e: KeyboardEvent) {
    const less = axis === "y" ? "ArrowUp" : "ArrowLeft";
    const more = axis === "y" ? "ArrowDown" : "ArrowRight";
    if (e.key === less) onresize(width - sign * NUDGE);
    else if (e.key === more) onresize(width + sign * NUDGE);
    else return;
    e.preventDefault();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_no_noninteractive_element_interactions -->
<div
  class="splitter {side}"
  class:dragging
  role="separator"
  aria-orientation={axis === "y" ? "horizontal" : "vertical"}
  aria-label={label}
  aria-valuenow={width}
  aria-valuemin={min}
  aria-valuemax={max}
  tabindex="0"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onlostpointercapture={up}
  onkeydown={key}
  ondblclick={() => onreset?.()}
></div>

<style>
  .splitter {
    position: absolute;
    top: var(--island-gap);
    bottom: var(--island-gap);
    width: 11px;
    z-index: 6;
    cursor: col-resize;
    /* invisible wide hit-area with a centred hairline that lights up on interaction */
    background: linear-gradient(var(--accent), var(--accent)) center / 1px 100% no-repeat;
    opacity: 0;
    transition: opacity 0.12s;
    touch-action: none;
  }
  .splitter.left {
    left: calc(var(--island-gap) + var(--activity-w) + var(--island-gap) + var(--explorer-w) + var(--island-gap) / 2);
    transform: translateX(-50%);
  }
  .splitter.right {
    right: calc(var(--island-gap) + var(--right-rail-w) + var(--island-gap) + var(--structure-w) + var(--island-gap) / 2);
    transform: translateX(50%);
  }
  /* horizontal handle on the top edge of the bottom problems dock, inset by the
     two rails so it tracks the gutter above the dock */
  .splitter.bottom {
    top: auto;
    left: calc(var(--island-gap) + var(--activity-w));
    right: calc(var(--island-gap) + var(--right-rail-w));
    bottom: calc(var(--island-gap) + var(--problems-h) + var(--island-gap) / 2);
    width: auto;
    height: 11px;
    cursor: row-resize;
    background: linear-gradient(var(--accent), var(--accent)) center / 100% 1px no-repeat;
    transform: translateY(50%);
  }
  .splitter:hover,
  .splitter:focus-visible,
  .splitter.dragging {
    opacity: 0.7;
    outline: none;
  }
</style>
