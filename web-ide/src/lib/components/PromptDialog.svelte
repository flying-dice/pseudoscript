<script lang="ts">
  // A small modal text prompt shared by the FileTree create/rename flows. The
  // parent mounts it (via `{#if dialog}`) with a `title`, an input `label`/
  // `placeholder`, an initial `value`, and a `validate(value)` returning an error
  // string (or null when valid). On confirm it calls `onconfirm(trimmedValue)`;
  // Escape, the overlay, or the close button cancels. Built on the shadcn Dialog
  // so it gets focus-trap, scroll-lock, and Escape handling for free.
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";

  type Props = {
    title?: string;
    label?: string;
    placeholder?: string;
    value?: string;
    confirmLabel?: string;
    hint?: string;
    validate?: (value: string) => string | null;
    onconfirm?: (value: string) => void;
    oncancel?: () => void;
  };

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
  }: Props = $props();

  let draft = $state<string>(value);
  let touched = $state<boolean>(false);

  const error = $derived(touched ? validate(draft.trim()) : null);
  const canSubmit = $derived(draft.trim().length > 0 && !validate(draft.trim()));

  function submit() {
    touched = true;
    if (validate(draft.trim())) return;
    onconfirm?.(draft.trim());
  }

  // The component is mounted only while open, so any close intent cancels.
  function onOpenChange(open: boolean) {
    if (!open) oncancel?.();
  }
</script>

<Dialog.Root open onOpenChange={onOpenChange}>
  <Dialog.Content class="sm:max-w-md" data-testid="prompt-dialog">
    <Dialog.Header>
      <Dialog.Title>{title}</Dialog.Title>
    </Dialog.Header>
    <form
      class="field"
      onsubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <span class="lbl">{label}</span>
      <!-- svelte-ignore a11y_autofocus -->
      <Input
        bind:value={draft}
        {placeholder}
        autofocus
        aria-label={label}
        spellcheck="false"
        autocomplete="off"
        oninput={() => (touched = true)}
        aria-invalid={error ? "true" : undefined}
        class="font-mono"
      />
      {#if error}
        <p class="err">{error}</p>
      {:else if hint}
        <p class="hint">{hint}</p>
      {/if}
      <Dialog.Footer>
        <Button type="button" variant="ghost" size="sm" onclick={() => oncancel?.()}>Cancel</Button>
        <Button type="submit" size="sm" disabled={!canSubmit}>{confirmLabel}</Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .lbl {
    font-family: var(--font-mono);
    font-size: 0.6rem;
    font-weight: 600;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .err {
    margin: 0.1rem 0 0;
    font-size: 0.76rem;
    color: var(--err);
  }
  .hint {
    margin: 0.1rem 0 0;
    font-size: 0.76rem;
    color: var(--ink-faint);
  }
</style>
