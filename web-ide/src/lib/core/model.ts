// The structural model index and its resolvers — pure.
//
// One builder turns the workspace outline into every lookup the view needs (node
// index, per-file grouping, flows, type map, collapse info), computed once. The
// resolvers (FQN → node, member → owner, ancestry, byte offset) are pure over
// that index. The WASM outline is produced by the view and passed in as `nodes`;
// no Svelte, no WASM import here.

import { charToByte } from "../offsets.js";
import type { Info, Scene, StructureNode, Symbol } from "./types.js";

// A node plus the FQN of the file that declares it.
export type NodeRef = { node: StructureNode; fileFqn: string };

// An entry-point flow listed under its owning node.
export type Flow = { fqn: string; name: string; triggered: boolean };

// Everything derived from the workspace outline, bundled so it computes once.
export type ModelIndex = {
  nodes: StructureNode[];
  structureByFile: Record<string, StructureNode[]>;
  nodeIndex: Map<string, NodeRef>;
  symbols: Symbol[];
  flowsByNode: Map<string, Flow[]>;
  nodeInfo: Info;
  typeFqnByName: Record<string, string>;
};

/**
 * Build the model index from the workspace outline `nodes` and the workspace's
 * module FQNs. Each node is grouped to its file by the longest module FQN that
 * prefixes it — node FQNs use the file's path-derived module name, the scheme
 * `hover`/`definition`/`references` resolve against (single-file `outline` keys
 * by the `//!` header instead, which diverged and broke cross-module GOTO).
 */
export function buildModelIndex(nodes: StructureNode[], moduleFqns: string[]): ModelIndex {
  const structureByFile: Record<string, StructureNode[]> = {};
  for (const mf of moduleFqns) if (mf) structureByFile[mf] = [];
  const byLongest = [...moduleFqns].sort((a, b) => b.length - a.length);
  for (const n of nodes) {
    const fileFqn = byLongest.find((mf) => n.fqn === mf || n.fqn.startsWith(`${mf}::`)) ?? "";
    (structureByFile[fileFqn] ??= []).push(n);
  }

  const nodeIndex = new Map<string, NodeRef>();
  for (const [fileFqn, list] of Object.entries(structureByFile)) {
    for (const n of list) nodeIndex.set(n.fqn, { node: n, fileFqn });
  }

  const symbols: Symbol[] = [...nodeIndex.entries()].map(([fqn, { node, fileFqn }]) => ({ ...node, fqn, fileFqn }));

  const flowsByNode = new Map<string, Flow[]>();
  for (const n of nodes) {
    if (n.kind !== "callable") continue;
    const parent = n.fqn.split("::").slice(0, -1).join("::");
    let bucket = flowsByNode.get(parent);
    if (!bucket) flowsByNode.set(parent, (bucket = []));
    bucket.push({ fqn: n.fqn, name: n.name, triggered: n.triggered });
  }

  const nodeInfo: Info = {};
  for (const n of nodes) nodeInfo[n.fqn] = { kind: n.kind, parent: n.parent ?? null };

  const typeFqnByName: Record<string, string> = {};
  for (const n of nodes) if (n.kind === "data") typeFqnByName[n.name] = n.fqn;

  return { nodes, structureByFile, nodeIndex, symbols, flowsByNode, nodeInfo, typeFqnByName };
}

/** An empty index, for the not-ready / no-workspace state. */
export function emptyModelIndex(): ModelIndex {
  return {
    nodes: [],
    structureByFile: {},
    nodeIndex: new Map(),
    symbols: [],
    flowsByNode: new Map(),
    nodeInfo: {},
    typeFqnByName: {},
  };
}

/**
 * Resolve a possibly depth-collapsed callee FQN to a real node: a direct hit, else
 * a callable named `method` somewhere beneath the collapsed owner (so a call's
 * member still resolves when its target was folded into an ancestor).
 */
export function resolveNodeFqn(index: ModelIndex, fqn: string): string | null {
  if (index.nodeIndex.has(fqn)) return fqn;
  const sep = fqn.lastIndexOf("::");
  if (sep < 0) return null;
  const owner = fqn.slice(0, sep);
  const method = fqn.slice(sep + 2);
  for (const n of index.nodes) {
    if (n.kind !== "callable" || n.name !== method) continue;
    for (let cur = n.parent; cur; cur = index.nodeInfo[cur]?.parent ?? null) {
      if (cur === owner) return n.fqn;
    }
  }
  return null;
}

/**
 * The nearest enclosing structural node for a member/field FQN: drop trailing
 * `::segment`s until one names a node. A field FQN like `M::Conv::id` isn't a node,
 * so without this go-to-definition on a field would silently do nothing (PDS-GOTO-002).
 */
export function ownerNodeOf(index: ModelIndex, fqn: string): string | null {
  let cur = fqn;
  while (cur.includes("::")) {
    cur = cur.slice(0, cur.lastIndexOf("::"));
    if (index.nodeIndex.has(cur)) return cur;
  }
  return null;
}

/** The structural ancestor chain (root system → … → the node), following `parent`. */
export function ancestry(index: ModelIndex, fqn: string): string[] {
  const chain: string[] = [];
  const seen = new Set<string>();
  let cur: string | null = fqn;
  while (cur && index.nodeIndex.has(cur) && !seen.has(cur)) {
    seen.add(cur);
    chain.unshift(cur);
    cur = index.nodeIndex.get(cur)?.node.parent ?? null;
  }
  return chain;
}

/** A node's breadcrumb title: kind + simple name, falling back to the FQN leaf. */
export function nodeTitle(index: ModelIndex, fqn: string): string {
  const n = index.nodes.find((x) => x.fqn === fqn);
  return n ? `${n.kind} \`${n.name}\`` : `\`${fqn.split("::").at(-1)}\``;
}

/** A minimal single-lifeline sequence scene for a symbol with nothing to project. */
export function singleLifelineScene(index: ModelIndex, fqn: string): Scene {
  const node = index.nodeIndex.get(fqn)?.node;
  return {
    view: "sequence",
    entry: fqn,
    participants: [{ fqn, label: node?.name ?? fqn.split("::").at(-1), kind: node?.kind ?? "callable" }],
    items: [],
  };
}

/** The byte offset of a 1-based line/byte-column in `source` (col is a byte column). */
export function nodeByteOffset(source: string, line: number, col: number): number {
  const lines = source.split("\n");
  let charOffset = 0;
  for (let i = 0; i < line - 1 && i < lines.length; i++) charOffset += lines[i].length + 1;
  return charToByte(source, charOffset + Math.max(0, col - 1));
}
