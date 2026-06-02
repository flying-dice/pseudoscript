<script lang="ts">
  // The destructive-action confirm (file delete). The parent mounts it with a
  // `title`, a `message`, and a `confirmLabel`; confirming calls `onconfirm`,
  // and Escape / overlay / Cancel calls `oncancel`. Built on the shadcn Dialog
  // for focus-trap, scroll-lock, and Escape handling.
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { Button } from "$lib/components/ui/button/index.js";

  type Props = {
    title?: string;
    message?: string;
    confirmLabel?: string;
    onconfirm?: () => void;
    oncancel?: () => void;
  };

  let { title = "", message = "", confirmLabel = "Delete", onconfirm, oncancel }: Props = $props();

  // The component is mounted only while open, so any close intent cancels.
  function onOpenChange(open: boolean) {
    if (!open) oncancel?.();
  }
</script>

<Dialog.Root open onOpenChange={onOpenChange}>
  <Dialog.Content class="sm:max-w-sm" data-testid="confirm-dialog">
    <Dialog.Header>
      <Dialog.Title>{title}</Dialog.Title>
      <Dialog.Description>{message}</Dialog.Description>
    </Dialog.Header>
    <Dialog.Footer>
      <Button type="button" variant="ghost" size="sm" onclick={() => oncancel?.()}>Cancel</Button>
      <Button type="button" variant="destructive" size="sm" onclick={() => onconfirm?.()}>{confirmLabel}</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
