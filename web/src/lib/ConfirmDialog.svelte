<script lang="ts">
  let {
    title,
    message,
    confirmText = 'Confirm',
    danger = false,
    onConfirm,
    onClose,
  }: {
    title: string;
    message: string;
    confirmText?: string;
    danger?: boolean;
    onConfirm: () => Promise<void>;
    onClose: () => void;
  } = $props();

  let busy = $state(false);
  let error = $state('');

  async function confirm() {
    if (busy) return;
    busy = true;
    error = '';
    try {
      await onConfirm();
      onClose();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Operation failed';
    } finally {
      busy = false;
    }
  }
</script>

<svelte:window
  onkeydown={(event) => {
    if (event.key === 'Escape') onClose();
  }}
/>

<div
  class="modal-backdrop"
  role="presentation"
  onclick={(event) => {
    if (event.target === event.currentTarget) onClose();
  }}
>
  <div class="modal" role="dialog" aria-modal="true" aria-label={title}>
    <h2>{title}</h2>
    <p class="sub">{message}</p>
    {#if error}<p class="login-error" style="text-align:left">{error}</p>{/if}
    <div class="modal-actions">
      <button class="button ghost" onclick={onClose} disabled={busy}>Cancel</button>
      <button class="button {danger ? 'danger' : 'primary'}" onclick={() => void confirm()} disabled={busy}>
        {busy ? 'Working…' : confirmText}
      </button>
    </div>
  </div>
</div>
