<script lang="ts">
  import type { UploadTask } from './types';
  import Icon from './Icon.svelte';

  let {
    tasks,
    onDismiss,
  }: {
    tasks: UploadTask[];
    onDismiss: () => void;
  } = $props();

  const active = $derived(tasks.filter((task) => task.status === 'queued' || task.status === 'uploading'));
  const failed = $derived(tasks.filter((task) => task.status === 'error'));
  const done = $derived(tasks.filter((task) => task.status === 'done'));
</script>

{#if tasks.length > 0}
  <div class="uploads">
    <div class="uploads-head">
      <span>
        {#if active.length > 0}
          Uploading {active.length} {active.length === 1 ? 'file' : 'files'}…
        {:else if failed.length > 0}
          {failed.length} upload{failed.length === 1 ? '' : 's'} failed
        {:else}
          {done.length} upload{done.length === 1 ? '' : 's'} complete
        {/if}
      </span>
      <button class="icon-button uploads-close" onclick={onDismiss} aria-label="Dismiss uploads">
        <Icon name="close" size={16} />
      </button>
    </div>
    {#each tasks.slice(-8) as task (task.id)}
      <div class="upload-task">
        <div class="upload-name">
          <span title={task.name}>{task.name}</span>
          <span>
            {#if task.status === 'error'}
              Failed
            {:else if task.status === 'done'}
              Done
            {:else if task.status === 'queued'}
              Queued
            {:else if task.total > 0}
              {Math.round((task.loaded / task.total) * 100)}%
            {:else}
              Uploading
            {/if}
          </span>
        </div>
        <div class="progress" class:error={task.status === 'error'}>
          <div
            style={`width:${task.status === 'done' ? 100 : task.total > 0 ? Math.min(100, (task.loaded / task.total) * 100) : task.status === 'uploading' ? 8 : 0}%`}
          ></div>
        </div>
      </div>
    {/each}
  </div>
{/if}
