<script lang="ts">
  import type { Entry } from './types';
  import { formatDate, formatSize, iconFor } from './format';
  import { thumbUrl } from './api';
  import Icon from './Icon.svelte';

  let {
    entry,
    selected,
    selectable = true,
    onToggle,
    onOpen,
    onRestricted,
  }: {
    entry: Entry;
    selected: boolean;
    selectable?: boolean;
    onToggle: () => void;
    onOpen: (entry: Entry) => void;
    onRestricted: () => void;
  } = $props();

  function activate() {
    if (entry.restricted) {
      onRestricted();
      return;
    }
    onOpen(entry);
  }
</script>

<div
  class="card"
  class:selected
  class:restricted={entry.restricted}
  role="button"
  tabindex="0"
  onclick={activate}
  onkeydown={(event) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      activate();
    }
  }}
>
  {#if selectable}
    <button
      class="check"
      class:checked={selected}
      aria-label={selected ? 'Deselect' : 'Select'}
      aria-pressed={selected}
      disabled={entry.restricted}
      onclick={(event) => {
        event.stopPropagation();
        onToggle();
      }}
    >
      {#if entry.restricted}<Icon name="lock" size={11} />{/if}
      {#if selected}<Icon name="check" size={14} />{/if}
    </button>
  {/if}
  <div class="thumb">
    <div class="thumb-icon"><Icon name={iconFor(entry)} size={entry.is_dir ? 46 : 38} /></div>
    {#if entry.has_thumb && !entry.restricted}
      <img
        src={thumbUrl(entry.path, 320)}
        alt=""
        loading="lazy"
        decoding="async"
        onerror={(event) => (event.currentTarget as HTMLImageElement).remove()}
      />
    {/if}
  </div>
  <div class="card-body">
    <div class="card-name" title={entry.name}>{entry.name}</div>
    <div class="card-meta">
      <span>{entry.is_dir ? 'Folder' : formatSize(entry.size)}</span>
      <span>{entry.restricted ? 'Restricted' : formatDate(entry.modified)}</span>
    </div>
  </div>
</div>
