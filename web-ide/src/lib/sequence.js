// Depth collapsing for sequence scenes. A trace is projected at full depth
// (every component lifeline); the IDE then collapses it to a chosen C4 level by
// rewriting each participant to its nearest allowed ancestor and dropping the
// interactions that become internal to a collapsed node. The structural graph
// the IDE already holds (fqn -> { kind, parent }) supplies the ancestry.

// The C4 kinds visible at each depth, widest first. "component" keeps every
// lifeline (no collapse); "container" hides components; "system" hides both.
const ALLOWED = {
  system: new Set(["person", "system"]),
  container: new Set(["person", "system", "container"]),
  component: new Set(["person", "system", "container", "component"]),
};

// The depth options, in widening order — drives the selector and validates input.
export const DEPTHS = [
  { id: "system", label: "Persons & Systems" },
  { id: "container", label: "Include Containers" },
  { id: "component", label: "Include Components" },
];

// The fqn that represents `fqn` at `depth`: itself if its kind is allowed,
// otherwise its nearest ancestor whose kind is. Synthesised initiators (a
// `caller`/`client`/`event:` token absent from `info`) are external actors and
// always show as-is.
function displayFqn(fqn, depth, info) {
  const allowed = ALLOWED[depth] ?? ALLOWED.component;
  let cur = fqn;
  const seen = new Set();
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
function remapItems(items, map) {
  const out = [];
  for (const item of items) {
    if (item.Message) {
      const m = item.Message;
      const from = map(m.from);
      const to = map(m.to);
      if (m.kind !== "self" && from === to) continue; // internal to a collapsed node
      const next = { Message: { ...m, from, to } };
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
export function collapseSequence(scene, depth, info) {
  if (!scene || !Array.isArray(scene.participants)) return scene;
  if (depth === "component" || !ALLOWED[depth]) return scene;

  const map = (fqn) => displayFqn(fqn, depth, info);

  const participants = [];
  const seen = new Set();
  for (const p of scene.participants) {
    const fqn = map(p.fqn);
    if (seen.has(fqn)) continue;
    seen.add(fqn);
    participants.push({ ...p, fqn, kind: info[fqn]?.kind ?? p.kind });
  }

  return { ...scene, participants, items: remapItems(scene.items, map) };
}
