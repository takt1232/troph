<script lang="ts">
  import { onMount } from 'svelte';
  import type { UploadTask } from './types';
  import { formatDuration, formatSpeed } from './format';
  import Icon from './Icon.svelte';

  let {
    tasks,
    collapsed,
    onToggle,
    onDismiss,
    onCancel,
    onCancelAll,
  }: {
    tasks: UploadTask[];
    collapsed: boolean;
    onToggle: () => void;
    onDismiss: () => void;
    onCancel: (id: string) => void;
    onCancelAll: () => void;
  } = $props();

  const statusRank: Record<UploadTask['status'], number> = {
    uploading: 0,
    queued: 1,
    error: 2,
    cancelled: 3,
    done: 4,
  };

  const uploading = $derived(tasks.filter((task) => task.status === 'uploading'));
  const queued = $derived(tasks.filter((task) => task.status === 'queued'));
  const failed = $derived(tasks.filter((task) => task.status === 'error'));
  const cancelled = $derived(tasks.filter((task) => task.status === 'cancelled'));
  const done = $derived(tasks.filter((task) => task.status === 'done'));
  const active = $derived(uploading.length + queued.length);
  const expected = $derived(tasks.length - cancelled.length);
  const pending = $derived([...uploading, ...queued]);
  const ordered = $derived(
    tasks
      .map((task, index) => ({ task, index }))
      .sort(
        (a, b) =>
          statusRank[a.task.status] - statusRank[b.task.status] || a.index - b.index,
      )
      .map(({ task }) => task),
  );
  const pendingLoaded = $derived(
    pending.reduce((sum, task) => sum + Math.min(task.loaded, task.total), 0),
  );
  const pendingTotal = $derived(
    pending.reduce((sum, task) => sum + Math.max(0, task.total), 0),
  );
  const progressBytes = $derived(
    done.reduce((sum, task) => sum + Math.max(0, task.total), 0) + pendingLoaded,
  );
  const totalBytes = $derived(
    tasks
      .filter((task) => task.status !== 'cancelled')
      .reduce((sum, task) => sum + Math.max(0, task.total), 0),
  );
  const overallPercent = $derived(
    totalBytes > 0
      ? Math.min(100, Math.round((progressBytes / totalBytes) * 100))
      : null,
  );

  let speed = $state(0);
  let lastSampleAt = 0;
  let lastSampleProgress = 0;
  let sampling = false;

  onMount(() => {
    const timer = window.setInterval(() => {
      if (uploading.length === 0) {
        speed = 0;
        sampling = false;
        return;
      }
      const now = performance.now();
      if (!sampling) {
        sampling = true;
        lastSampleAt = now;
        lastSampleProgress = progressBytes;
        return;
      }
      const elapsed = (now - lastSampleAt) / 1000;
      if (elapsed < 0.4) return;
      const instant = Math.max(0, (progressBytes - lastSampleProgress) / elapsed);
      speed = speed === 0 ? instant : speed * 0.7 + instant * 0.3;
      lastSampleAt = now;
      lastSampleProgress = progressBytes;
    }, 500);
    return () => window.clearInterval(timer);
  });

  const etaLabel = $derived(
    speed > 0 && pendingTotal > pendingLoaded
      ? formatDuration((pendingTotal - pendingLoaded) / speed)
      : null,
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
    if (task.status === 'cancelled') return 'Cancelled';
    const parts: string[] = [];
    parts.push(task.total > 0 ? `${progressPercent(task)}%` : 'Uploading');
    const speedLabel = formatSpeed(task.speed ?? 0);
    if (speedLabel) parts.push(speedLabel);
    return parts.join(' · ');
  }

  const summary = $derived.by(() => {
    if (uploading.length > 0) {
      const parts = [`Uploaded ${done.length} of ${expected}`];
      if (overallPercent !== null) parts.push(`${overallPercent}%`);
      if (etaLabel !== null) parts.push(`~${etaLabel} left`);
      return parts.join(' · ');
    }
    if (queued.length > 0) {
      return `Waiting to upload ${queued.length} ${queued.length === 1 ? 'file' : 'files'}`;
    }
    if (failed.length > 0) {
      return `${failed.length} upload${failed.length === 1 ? '' : 's'} failed`;
    }
    if (cancelled.length > 0) {
      const cancelledLabel = `${cancelled.length} cancelled`;
      if (done.length === 0) {
        return `${cancelled.length} upload${cancelled.length === 1 ? '' : 's'} cancelled`;
      }
      return `${done.length} uploaded · ${cancelledLabel}`;
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
          {#if active > 0}
            <button class="uploads-cancel-all" onclick={onCancelAll}>Cancel all</button>
          {/if}
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
            <div class="upload-row">
              <div class="upload-name">
                <span title={task.name}>{task.name}</span>
                <span>{statusLabel(task)}</span>
              </div>
              {#if task.status === 'queued' || task.status === 'uploading'}
                <button
                  class="upload-cancel"
                  onclick={() => onCancel(task.id)}
                  aria-label={`Cancel ${task.name}`}
                  title="Cancel upload"
                >
                  <Icon name="close" size={14} />
                </button>
              {/if}
            </div>
            <div class="progress" class:error={task.status === 'error'} class:cancelled={task.status === 'cancelled'}>
              <div style={`width:${barWidth(task)}%`}></div>
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/if}
{/if}
