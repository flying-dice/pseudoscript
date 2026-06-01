// Selection / view state — a reactive rune store.
//
// Owns which content pane is shown, the selected node scope, the sequence-diagram
// depth, and the queued editor jump. The orchestration (selectNode, applyLocation,
// the pendingGoto → editor effect) stays in the view, which writes these fields.

import type { Depth } from "$lib/core/types.js";

type Selected = { fqn: string; line: number; col: number; fileFqn: string } | null;
type PendingGoto = { line: number; col: number; fileFqn: string } | null;

class SelectionStore {
  // The content pane: source ("code"), diagram ("canvas"), or problems list.
  view = $state<"code" | "canvas" | "problems">("code");
  // The C4 depth a sequence diagram is collapsed to.
  seqDepth = $state<Depth>("component");
  // The selected node scope (drives canvas / breadcrumb / nav highlight), or null.
  selected = $state<Selected>(null);
  // A queued editor jump, applied once the code view is mounted on its file.
  pendingGoto = $state<PendingGoto>(null);
}

export const selection = new SelectionStore();
