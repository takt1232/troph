<script lang="ts">
  import { onMount } from 'svelte';

  let {
    title,
    label,
    initial = '',
    confirmText = 'Save',
    onSubmit,
    onClose,
  }: {
    title: string;
    label: string;
    initial?: string;
    confirmText?: string;
    onSubmit: (value: string) => Promise<void>;
    onClose: () => void;
  } = $props();

  let value = $state('');
  let busy = $state(false);
  let error = $state('');
  let input: HTMLInputElement | undefined = $state();

  onMount(() => {
    value = initial;
    input?.focus();
    input?.select();
  });

  async function submit() {
    if (busy) return;
    if (!value.trim()) {
      error = 'Name cannot be empty';
      return;
    }
    busy = true;
    error = '';
    try {
      await onSubmit(value.trim());
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
  <form
    class="modal"
    onsubmit={(event) => {
      event.preventDefault();
      void submit();
    }}
  >
    <h2>{title}</h2>
    <p class="sub">{label}</p>
    <input bind:this={input} bind:value={value} class="text-input" autocomplete="off" />
    {#if error}<p class="form-error">{error}</p>{/if}
    <div class="modal-actions">
      <button class="button ghost" type="button" onclick={onClose} disabled={busy}>Cancel</button>
      <button class="button primary" type="submit" disabled={busy}>
        {busy ? 'Working…' : confirmText}
      </button>
    </div>
  </form>
</div>
