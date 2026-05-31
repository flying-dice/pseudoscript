<script>
  // Stacked toast notifications, keyed by id. The page pushes `{id, kind,
  // title, body}` entries (kind: success | error | info) and auto-dismisses
  // them; each is also closable.
  let { notes = [], ondismiss } = $props();

  const ICON = { success: "✓", error: "!", info: "i" };
</script>

<div class="notes" role="region" aria-label="Notifications" aria-live="polite">
  {#each notes as note (note.id)}
    <div class="note {note.kind}" role="status">
      <div class="note-head">
        <span class="note-icon" aria-hidden="true">{ICON[note.kind] ?? "i"}</span>
        <span class="note-title">{note.title}</span>
        <button class="note-close" aria-label="Dismiss notification" onclick={() => ondismiss?.(note.id)}>✕</button>
      </div>
      {#if note.body}<p class="note-body">{note.body}</p>{/if}
    </div>
  {/each}
</div>

<style>
  .notes {
    position: fixed;
    top: calc(var(--topbar-h) + 0.7rem);
    right: 0.9rem;
    z-index: 60;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    width: min(360px, calc(100vw - 1.8rem));
    pointer-events: none;
  }
  .note {
    pointer-events: auto;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-left: 3px solid var(--accent);
    border-radius: var(--radius);
    box-shadow: var(--shadow-md);
    padding: 0.7rem 0.8rem;
    animation: slide-in 0.24s cubic-bezier(0.2, 0.7, 0.2, 1) both;
  }
  .note.success { border-left-color: var(--ok); }
  .note.error { border-left-color: var(--err); }
  .note.info { border-left-color: var(--accent); }

  .note-head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .note-icon {
    display: grid;
    place-items: center;
    width: 1.05rem;
    height: 1.05rem;
    flex: none;
    border-radius: 50%;
    font-size: 0.62rem;
    font-weight: 700;
    color: var(--accent-ink);
    background: var(--accent);
  }
  .success .note-icon { background: var(--ok); }
  .error .note-icon { background: var(--err); }
  .note-title {
    font-family: var(--font-display);
    font-weight: 600;
    font-size: 0.85rem;
    color: var(--ink);
    flex: 1;
    min-width: 0;
  }
  .note-close {
    flex: none;
    background: transparent;
    border: none;
    color: var(--ink-faint);
    font-size: 0.72rem;
    cursor: pointer;
    padding: 0.1rem 0.2rem;
  }
  .note-close:hover { color: var(--ink); }
  .note-body {
    margin: 0.4rem 0 0;
    font-family: var(--font-mono);
    font-size: 0.74rem;
    line-height: 1.6;
    color: var(--ink-soft);
  }

  @keyframes slide-in {
    from { opacity: 0; transform: translateX(12px); }
    to { opacity: 1; transform: translateX(0); }
  }
</style>
