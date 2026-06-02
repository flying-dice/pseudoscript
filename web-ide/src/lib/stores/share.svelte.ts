// Share / export transient state — a reactive rune store.
//
// Just the in-flight flag for the share-link encode (which gates re-entry and
// drives the button's busy state). The encode/clipboard/download logic stays in
// the view; the pure snapshot/hash helpers live in core/share.

class ShareStore {
  // Whether a share-link encode is in progress.
  busy = $state(false);
}

export const shareStore = new ShareStore();
