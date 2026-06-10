// Containment ancestry + gateway routing over the C4 node tree, for the 3D graph.
// Pure string logic on a `parentOf` map (node id → its parent id, or null for a
// top-level system); a node is "present" iff it is a key. Ported verbatim from the
// web IDE's graph-route.ts (types dropped) so the universe island routes exactly
// like the IDE's 3D graph.

/** An FQN's leaf — its last `::` segment (`banking::core::Mainframe` → `Mainframe`).
 *  Used for display labels and for bridging a sequence FQN to a graph node by name.
 *  @param {string} fqn @returns {string} */
export function simpleName(fqn) {
  const parts = fqn.split("::");
  return parts[parts.length - 1] ?? fqn;
}

/** The containment chain `[self, …, root]` for `id`, stopping at the first ancestor
 *  not present in the tree.
 *  @param {string} id @param {ReadonlyMap<string, string | null>} parentOf
 *  @returns {string[]} */
export function ancestors(id, parentOf) {
  const chain = [id];
  let p = parentOf.get(id) ?? null;
  while (p && parentOf.has(p)) {
    chain.push(p);
    p = parentOf.get(p) ?? null;
  }
  return chain;
}

/** The gateway route from `a` to `b`: up each side to the lowest common ancestor (or,
 *  across systems, joining the two roots). `bridge` is the single segment that
 *  straddles the meeting point — the pair the relationship force should pull together,
 *  or `null` when the two nodes are on the same containment line.
 *  @param {string} a @param {string} b
 *  @param {ReadonlyMap<string, string | null>} parentOf
 *  @returns {{ path: string[], bridge: [string, string] | null }} */
export function routeOf(a, b, parentOf) {
  const A = ancestors(a, parentOf);
  const B = ancestors(b, parentOf);
  const bIdx = new Map(B.map((id, i) => [id, i]));
  const i = A.findIndex((id) => bIdx.has(id));
  if (i >= 0) {
    const j = bIdx.get(A[i]);
    const path = [...A.slice(0, i + 1), ...B.slice(0, j).reverse()];
    const bridge = i - 1 >= 0 && i + 1 < path.length ? [path[i - 1], path[i + 1]] : null;
    return { path, bridge };
  }
  // disjoint roots (different systems): join the two roots directly.
  return { path: [...A, ...B.slice().reverse()], bridge: [A[A.length - 1], B[B.length - 1]] };
}
