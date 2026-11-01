<script lang="ts">
  import type { Entry } from './types';
  import { formatDate, formatSize, iconFor, kindLabel } from './format';
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
  class="row"
  class:selected
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
  <div class="row-thumb">
    <Icon name={iconFor(entry)} size={23} />
    {#if entry.has_thumb && !entry.restricted}
      <img
        src={thumbUrl(entry.path, 160)}
        alt=""
        loading="lazy"
        decoding="async"
        style="position:absolute;inset:0"
        onerror={(event) => (event.currentTarget as HTMLImageElement).remove()}
      />
    {/if}
  </div>
  <div class="row-main">
    <div class="row-name" title={entry.name}>{entry.name}</div>
    <div class="row-sub">
      {entry.restricted ? 'Restricted' : `${kindLabel(entry)}${entry.is_dir ? '' : ` · ${formatSize(entry.size)}`}`}
    </div>
  </div>
  <div class="row-sub desktop-only" style="flex:0 0 86px;text-align:right">
    {formatDate(entry.modified)}
  </div>
</div>
