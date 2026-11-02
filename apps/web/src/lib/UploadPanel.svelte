<script lang="ts">
  import type { UploadTask } from './types';
  import Icon from './Icon.svelte';

  let {
    tasks,
    collapsed,
    onToggle,
    onDismiss,
  }: {
    tasks: UploadTask[];
    collapsed: boolean;
    onToggle: () => void;
    onDismiss: () => void;
  } = $props();

  const statusRank: Record<UploadTask['status'], number> = {
    uploading: 0,
    queued: 1,
    error: 2,
    done: 3,
  };

  const uploading = $derived(tasks.filter((task) => task.status === 'uploading'));
  const queued = $derived(tasks.filter((task) => task.status === 'queued'));
  const failed = $derived(tasks.filter((task) => task.status === 'error'));
  const done = $derived(tasks.filter((task) => task.status === 'done'));
  const active = $derived(uploading.length + queued.length);
  const ordered = $derived(
    tasks
      .map((task, index) => ({ task, index }))
      .sort(
        (a, b) =>
          statusRank[a.task.status] - statusRank[b.task.status] || a.index - b.index,
      )
      .map(({ task }) => task),
  );
  const current = $derived(uploading[0]);
  const currentPercent = $derived(
    current && current.total > 0 ? progressPercent(current) : null,
  );

  function progressPercent(task: UploadTask): number {
    if (task.total <= 0) return 0;
    return Math.min(100, Math.round((task.loaded / task.total) * 100));
  }

  function barWidth(task: UploadTask): number {
    if (task.status === 'done') return 100;
    if (task.status === 'uploading' && task.total <= 0) return 8;
    return progressPercent(task);
  }

  function statusLabel(task: UploadTask): string {
    if (task.status === 'error') return 'Failed';
    if (task.status === 'done') return 'Done';
    if (task.status === 'queued') return 'Queued';
    if (task.total > 0) return `${progressPercent(task)}%`;
    return 'Uploading';
  }

  const summary = $derived.by(() => {
    if (uploading.length > 0) {
      const base = `Uploading ${uploading.length} of ${active}`;
      return currentPercent === null ? base : `${base} · ${currentPercent}%`;
    }
    if (queued.length > 0) {
      return `Waiting to upload ${queued.length} ${queued.length === 1 ? 'file' : 'files'}`;
    }
    if (failed.length > 0) {
      return `${failed.length} upload${failed.length === 1 ? '' : 's'} failed`;
    }
    return `${done.length} upload${done.length === 1 ? '' : 's'} complete`;
  });
</script>

{#if tasks.length > 0}
  {#if collapsed}
    <button
      class="uploads-pill"
      onclick={onToggle}
      aria-expanded="false"
      aria-controls="uploads-panel"
    >
      {#if active > 0}
        <span class="spinner small"></span>
      {:else if failed.length > 0}
        <span class="uploads-pill-dot error"></span>
      {:else}
        <Icon name="check" size={15} />
      {/if}
      <span class="uploads-pill-label">{summary}</span>
      <Icon name="chevron-up" size={15} />
    </button>
  {:else}
    <div class="uploads" id="uploads-panel">
      <div class="uploads-head">
        <span class="uploads-title">{summary}</span>
        <div class="uploads-actions">
          <button
            class="icon-button uploads-action"
            onclick={onToggle}
            aria-label="Collapse uploads"
            aria-expanded="true"
            aria-controls="uploads-list"
          >
            <Icon name="chevron-down" size={16} />
          </button>
          {#if active === 0}
            <button
              class="icon-button uploads-action"
              onclick={onDismiss}
              aria-label="Dismiss uploads"
            >
              <Icon name="close" size={16} />
            </button>
          {/if}
        </div>
      </div>
      <div class="uploads-list" id="uploads-list">
        {#each ordered as task (task.id)}
          <div class="upload-task">
            <div class="upload-name">
              <span title={task.name}>{task.name}</span>
              <span>{statusLabel(task)}</span>
            </div>
            <div class="progress" class:error={task.status === 'error'}>
              <div style={`width:${barWidth(task)}%`}></div>
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/if}
{/if}
