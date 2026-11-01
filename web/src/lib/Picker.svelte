<script lang="ts">
  import { onMount } from 'svelte';
  import { loadListing } from './api';
  import type { Listing } from './types';
  import Icon from './Icon.svelte';

  let {
    title,
    confirmText,
    startPath,
    onConfirm,
    onClose,
  }: {
    title: string;
    confirmText: string;
    startPath: string;
    onConfirm: (path: string) => Promise<void>;
    onClose: () => void;
  } = $props();

  let listing = $state<Listing | null>(null);
  let loading = $state(true);
  let busy = $state(false);
  let error = $state('');

  onMount(() => {
    void navigate(startPath);
  });

  async function navigate(path: string) {
    loading = true;
    error = '';
    try {
      listing = await loadListing(path);
    } catch (err) {
      error = err instanceof Error ? err.message : 'Cannot open folder';
    } finally {
      loading = false;
    }
  }

  async function confirm() {
    if (!listing || busy) return;
    busy = true;
    error = '';
    try {
      await onConfirm(listing.path);
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
    <p class="sub">Choose a destination folder.</p>
    <div class="picker-path" title={listing?.path}>{listing?.path || startPath}</div>
    <div class="picker-list">
      {#if loading}
        <div class="picker-empty">Loading…</div>
      {:else if listing}
        {#if listing.parent}
          <button class="picker-item" onclick={() => void navigate(listing!.parent!)}>
            <Icon name="chevron-left" size={16} /> Up one level
          </button>
        {/if}
        {#each listing.entries.filter((entry) => entry.is_dir && !entry.restricted) as entry (entry.path)}
          <button class="picker-item" onclick={() => void navigate(entry.path)}>
            <Icon name="folder" size={18} /> <span class="picker-name">{entry.name}</span>
          </button>
        {/each}
        {#if listing.entries.filter((entry) => entry.is_dir && !entry.restricted).length === 0}
          <div class="picker-empty">No subfolders here</div>
        {/if}
      {:else}
        <div class="picker-empty">{error || 'Cannot open folder'}</div>
      {/if}
    </div>
    {#if error && listing}<p class="form-error">{error}</p>{/if}
    <div class="modal-actions">
      <button class="button ghost" onclick={onClose} disabled={busy}>Cancel</button>
      <button class="button primary" onclick={() => void confirm()} disabled={busy || !listing}>
        {busy ? 'Working…' : confirmText}
      </button>
    </div>
  </div>
</div>
