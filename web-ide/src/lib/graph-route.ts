// Containment ancestry + gateway routing over the C4 node tree, for the 3D graph.
// Pure string logic on a `parentOf` map (node id → its parent id, or null for a
// top-level system); a node is "present" iff it is a key. Extracted from the renderer
// so the routing — which has non-trivial lowest-common-ancestor and bridge logic — is
// unit-testable on its own.

export type ParentOf = ReadonlyMap<string, string | null>;

/** An FQN's leaf — its last `::` segment (`banking::core::Mainframe` → `Mainframe`).
 *  Used for display labels and for bridging a sequence FQN to a graph node by name. */
export function simpleName(fqn: string): string {
  return fqn.split("::").at(-1) ?? fqn;
}

/** The gateway route between two nodes: the polyline of nodes it passes through, and
 *  the single segment that straddles the meeting point. */
export type Route = { path: string[]; bridge: [string, string] | null };

/** The containment chain `[self, …, root]` for `id`, stopping at the first ancestor
 *  not present in the tree. */
export function ancestors(id: string, parentOf: ParentOf): string[] {
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
 *  or `null` when the two nodes are on the same containment line. */
export function routeOf(a: string, b: string, parentOf: ParentOf): Route {
  const A = ancestors(a, parentOf);
  const B = ancestors(b, parentOf);
  const bIdx = new Map(B.map((id, i) => [id, i] as const));
  const i = A.findIndex((id) => bIdx.has(id));
  if (i >= 0) {
    const j = bIdx.get(A[i])!;
    const path = [...A.slice(0, i + 1), ...B.slice(0, j).reverse()];
    const bridge: [string, string] | null =
      i - 1 >= 0 && i + 1 < path.length ? [path[i - 1], path[i + 1]] : null;
    return { path, bridge };
  }
  // disjoint roots (different systems): join the two roots directly.
  return { path: [...A, ...B.slice().reverse()], bridge: [A[A.length - 1], B[B.length - 1]] };
}
