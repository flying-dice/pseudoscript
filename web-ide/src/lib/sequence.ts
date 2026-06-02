// Depth collapsing for sequence scenes. A trace is projected at full depth
// (every component lifeline); the IDE then collapses it to a chosen C4 level by
// rewriting each participant to its nearest allowed ancestor and dropping the
// interactions that become internal to a collapsed node. The structural graph
// the IDE already holds (fqn -> { kind, parent }) supplies the ancestry.

// The C4 depth levels, widest first.
export type Depth = "system" | "container" | "component";

// One structural node: its C4 kind, the fqn of its parent (null at the root),
// and its `///` summary (shown dimmed on a lifeline head).
export interface NodeInfo {
  kind: string;
  parent: string | null;
  summary?: string | null;
}

// fqn -> structural node, the ancestry the collapse walks.
export type Info = Record<string, NodeInfo | undefined>;

// A single lifeline in the scene.
export interface Participant {
  fqn: string;
  kind: string;
  summary?: string | null;
  parent_path?: string | null;
  [key: string]: unknown;
}

// A call/return arrow between two lifelines.
export interface Message {
  from: string;
  to: string;
  kind: string;
  label?: string;
  detail?: string;
  [key: string]: unknown;
}

// A grouping frame wrapping a body of items (loop/alt/opt/etc.).
export interface Frame {
  body: SceneItem[];
  [key: string]: unknown;
}

// A scene item is either a message or a frame (tagged-union shape from WASM).
export interface SceneItem {
  Message?: Message;
  Frame?: Frame;
}

// A sequence scene: its lifelines and the ordered items between them.
export interface Scene {
  participants: Participant[];
  items: SceneItem[];
  [key: string]: unknown;
}

// Remap a fqn to the fqn it displays as at the chosen depth.
type FqnMap = (fqn: string) => string;

// The C4 kinds visible at each depth, widest first. "component" keeps every
// lifeline (no collapse); "container" hides components; "system" hides both.
const ALLOWED: Record<Depth, Set<string>> = {
  system: new Set(["person", "system"]),
  container: new Set(["person", "system", "container"]),
  component: new Set(["person", "system", "container", "component"]),
};

// The depth options, in widening order — drives the selector and validates input.
export const DEPTHS: { id: Depth; label: string }[] = [
  { id: "system", label: "Persons & Systems" },
  { id: "container", label: "Include Containers" },
  { id: "component", label: "Include Components" },
];

// The fqn that represents `fqn` at `depth`: itself if its kind is allowed,
// otherwise its nearest ancestor whose kind is. Synthesised initiators (a
// `caller`/`client`/`event:` token absent from `info`) are external actors and
// always show as-is.
// The simple (final-segment) name of an fqn.
function simpleName(fqn: string): string {
  return fqn.split("::").at(-1) ?? fqn;
}

// The structural ancestry shown dimmed under a container/component name:
// enclosing node names, outermost first, joined with `::`. Walks the `info`
// parent chain (the fqn is module-flat and does not carry the C4 nesting).
// `null` for other kinds and for a node with no enclosing parent. Mirrors the
// Rust projection's `ancestry_path` so collapsed and uncollapsed views agree.
function ancestryPath(fqn: string, info: Info): string | null {
  const kind = info[fqn]?.kind;
  if (kind !== "container" && kind !== "component") return null;
  const names: string[] = [];
  const seen = new Set<string>();
  let cur = info[fqn]?.parent ?? null;
  while (cur && !seen.has(cur)) {
    seen.add(cur);
    names.push(simpleName(cur));
    cur = info[cur]?.parent ?? null;
  }
  return names.length ? names.reverse().join("::") : null;
}

function displayFqn(fqn: string, depth: string, info: Info): string {
  const allowed = ALLOWED[depth as Depth] ?? ALLOWED.component;
  let cur = fqn;
  const seen = new Set<string>();
  while (cur && !seen.has(cur)) {
    seen.add(cur);
    const node = info[cur];
    if (!node || allowed.has(node.kind)) return cur;
    if (!node.parent) return cur; // top of the chain, nothing allowed above
    cur = node.parent;
  }
  return cur;
}

// Recursively remap a list of scene items to the collapsed fqns, dropping
// call/return messages that become self-internal and frames whose body empties.
// Adjacent messages that collapse to the same arrow are de-duplicated so a
// container that fans out to several of its components reads as one call.
function remapItems(items: SceneItem[], map: FqnMap): SceneItem[] {
  const out: SceneItem[] = [];
  for (const item of items) {
    if (item.Message) {
      const m = item.Message;
      const from = map(m.from);
      const to = map(m.to);
      if (m.kind !== "self" && from === to) continue; // internal to a collapsed node
      const next: SceneItem = { Message: { ...m, from, to } };
      const prev = out[out.length - 1]?.Message;
      if (
        prev &&
        prev.kind === m.kind &&
        prev.from === from &&
        prev.to === to &&
        prev.label === m.label &&
        prev.detail === m.detail
      ) {
        continue; // duplicate adjacent arrow after collapse
      }
      out.push(next);
    } else if (item.Frame) {
      const body = remapItems(item.Frame.body, map);
      if (body.length > 0) out.push({ Frame: { ...item.Frame, body } });
    }
  }
  return out;
}

// Collapse a sequence `scene` to `depth` using `info` (fqn -> { kind, parent }).
// Returns a new scene; `depth === "component"` (or unknown) is the identity.
// The original scene is never mutated.
export function collapseSequence(scene: Scene | null | undefined, depth: string, info: Info): Scene | null | undefined {
  if (!scene || !Array.isArray(scene.participants)) return scene;
  if (depth === "component" || !ALLOWED[depth as Depth]) return scene;

  const map: FqnMap = (fqn) => displayFqn(fqn, depth, info);

  const participants: Participant[] = [];
  const seen = new Set<string>();
  for (const p of scene.participants) {
    const fqn = map(p.fqn);
    if (seen.has(fqn)) continue;
    seen.add(fqn);
    // The collapsed lifeline shows the ancestor node's own summary and ancestry,
    // not the pre-collapse leaf's.
    participants.push({
      ...p,
      fqn,
      kind: info[fqn]?.kind ?? p.kind,
      summary: info[fqn]?.summary ?? null,
      parent_path: ancestryPath(fqn, info),
    });
  }

  return { ...scene, participants, items: remapItems(scene.items, map) };
}
