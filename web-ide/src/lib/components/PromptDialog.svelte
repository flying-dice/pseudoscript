<script>
  // A small modal text prompt shared by the FileTree create/rename flows. The
  // parent passes a `title`, an input `label`/`placeholder`, an initial `value`,
  // and a `validate(value)` returning an error string (or null when valid). On
  // confirm it calls `onconfirm(trimmedValue)`; Escape or the backdrop cancels.
  let {
    title = "",
    label = "Name",
    placeholder = "",
    value = "",
    confirmLabel = "Create",
    hint = "",
    validate = () => null,
    onconfirm,
    oncancel,
  } = $props();

  let draft = $state(value);
  let touched = $state(false);
  let inputEl = $state(null);

  const error = $derived(touched ? validate(draft.trim()) : null);
  const canSubmit = $derived(draft.trim().length > 0 && !validate(draft.trim()));

  function submit() {
    touched = true;
    if (validate(draft.trim())) return;
    onconfirm?.(draft.trim());
  }

  function onKey(e) {
    if (e.key === "Escape") {
      e.preventDefault();
      oncancel?.();
    } else if (e.key === "Enter") {
      e.preventDefault();
      submit();
    }
  }

  $effect(() => {
    inputEl?.focus();
    inputEl?.select();
  });
</script>

<div
  class="backdrop"
  role="presentation"
  onclick={(e) => {
    if (e.target === e.currentTarget) oncancel?.();
  }}
>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={title} tabindex="-1" onkeydown={onKey}>
    <h2>{title}</h2>
    <label class="field">
      <span class="lbl">{label}</span>
      <input
        bind:this={inputEl}
        bind:value={draft}
        {placeholder}
        oninput={() => (touched = true)}
        spellcheck="false"
        autocomplete="off"
      />
    </label>
    {#if error}
      <p class="err">{error}</p>
    {:else if hint}
      <p class="hint">{hint}</p>
    {/if}
    <div class="actions">
      <button class="ghost" type="button" onclick={() => oncancel?.()}>Cancel</button>
      <button class="primary" type="button" disabled={!canSubmit} onclick={submit}>
        {confirmLabel}
      </button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--bg, #000) 62%, transparent);
    backdrop-filter: blur(2px);
  }
  .dialog {
    width: min(28rem, calc(100vw - 2rem));
    background: var(--surface, #fff);
    border: 1px solid var(--line);
    border-radius: var(--radius, 10px);
    padding: 1.1rem 1.2rem 1.2rem;
    box-shadow: var(--shadow-lg);
  }
  h2 {
    margin: 0 0 0.85rem;
    font-size: 0.95rem;
    color: var(--ink);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }
  .lbl {
    font-family: var(--font-mono);
    font-size: 0.6rem;
    font-weight: 600;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  input {
    width: 100%;
    box-sizing: border-box;
    padding: 0.5rem 0.6rem;
    font-family: var(--font-mono);
    font-size: 0.85rem;
    color: var(--ink);
    background: var(--surface-2, #f4f4f5);
    border: 1px solid var(--line);
    border-radius: var(--radius-sm, 6px);
  }
  input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .err {
    margin: 0.5rem 0 0;
    font-size: 0.76rem;
    color: var(--err);
  }
  .hint {
    margin: 0.5rem 0 0;
    font-size: 0.76rem;
    color: var(--ink-faint);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 1.1rem;
  }
  button {
    padding: 0.45rem 0.85rem;
    font-size: 0.8rem;
    border-radius: var(--radius-sm, 6px);
    cursor: pointer;
    border: 1px solid var(--line);
  }
  .ghost {
    background: transparent;
    color: var(--ink-soft);
  }
  .ghost:hover {
    background: var(--surface-2);
    color: var(--ink);
  }
  .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-ink, #fff);
  }
  .primary:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
