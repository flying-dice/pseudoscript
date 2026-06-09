// A flow's colour: a stable index into a varied categorical palette, hashed from a
// key. The key is a node id, a directed pair (`from>to`), or a flow's entry fqn — so
// the same flow reads the same hue everywhere it appears (the 3D graph's filaments and
// the page's flow wiring). A categorical palette (vs. archetype-derived hues, which
// cluster to one "service" green) keeps the graph reading in many colours.
export const FLOW_PALETTE = [
  "#ff6b6b", "#ffa94d", "#ffd43b", "#a9e34b", "#69db7c", "#38d9a9", "#3bc9db",
  "#4dabf7", "#748ffc", "#9775fa", "#da77f2", "#f783ac", "#ff8787", "#ffc078",
  "#94d82d", "#20c997",
] as const;

// FNV-1a (32-bit) over `key`.
function fnv1a(key: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < key.length; i++) {
    h ^= key.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

/** A stable palette hex for `key` (a node id, a `from>to` pair, or a flow fqn). */
export function flowColor(key: string): string {
  return FLOW_PALETTE[fnv1a(key) % FLOW_PALETTE.length];
}
