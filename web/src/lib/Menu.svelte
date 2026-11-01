<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from './Icon.svelte';

  type MenuItem = {
    label: string;
    icon?: string;
    action: () => void;
    disabled?: boolean;
    danger?: boolean;
  };

  let {
    label = 'More actions',
    icon = 'more',
    items,
    up = false,
  }: {
    label?: string;
    icon?: string;
    items: MenuItem[];
    up?: boolean;
  } = $props();

  let open = $state(false);
  let root: HTMLDivElement | undefined = $state();

  function toggle() {
    open = !open;
  }

  function close() {
    open = false;
  }

  onMount(() => {
    const onPointer = (event: PointerEvent) => {
      if (open && root && !root.contains(event.target as Node)) close();
    };
    document.addEventListener('pointerdown', onPointer);
    return () => document.removeEventListener('pointerdown', onPointer);
  });
</script>

<svelte:window
  onkeydown={(event) => {
    if (event.key === 'Escape') close();
  }}
/>

<div class="menu" bind:this={root}>
  <button
    class="icon-button"
    aria-haspopup="menu"
    aria-expanded={open}
    aria-label={label}
    title={label}
    onclick={toggle}
  >
    <Icon name={icon} size={20} />
  </button>
  {#if open}
    <div class="menu-panel" class:up role="menu" aria-label={label}>
      {#each items as item (item.label)}
        <button
          class="menu-item"
          class:danger={item.danger}
          role="menuitem"
          disabled={item.disabled}
          onclick={() => {
            close();
            item.action();
          }}
        >
          {#if item.icon}<Icon name={item.icon} size={16} />{:else}<span class="menu-icon"></span>{/if}
          <span>{item.label}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>
