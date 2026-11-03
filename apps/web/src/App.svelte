<script lang="ts">
  import { onMount } from 'svelte';
  import {
    ApiError,
    deleteItems,
    downloadUrl,
    downloadZip,
    loadListing,
    loadMe,
    loadRootsListing,
    login,
    logout,
    makeFolder,
    rawUrl,
    renameItem,
    ROOTS_PATH,
    transferItems,
    uploadFile,
    UploadCancelledError,
  } from './lib/api';
  import type { Entry, Listing, Me, UploadTask } from './lib/types';
  import { collectDroppedFiles, type DroppedFile } from './lib/drop';
  import Icon from './lib/Icon.svelte';
  import Login from './lib/Login.svelte';
  import EntryCard from './lib/EntryCard.svelte';
  import EntryRow from './lib/EntryRow.svelte';
  import TextPrompt from './lib/TextPrompt.svelte';
  import ConfirmDialog from './lib/ConfirmDialog.svelte';
  import Picker from './lib/Picker.svelte';
  import UploadPanel from './lib/UploadPanel.svelte';
  import Menu from './lib/Menu.svelte';

  const appVersion = __APP_VERSION__;
  const MAX_PARALLEL_UPLOADS = 4;

  let me = $state<Me | null>(null);
  let listing = $state<Listing | null>(null);
  let loading = $state(true);
  let loadingListing = $state(false);
  let navId = 0;
  let navAbort: AbortController | null = null;
  let loginError = $state('');
  let submittingLogin = $state(false);
  let toast = $state('');
  let toastError = $state(false);
  let toastTimer: number | undefined;
  let selected = $state<string[]>([]);
  let view = $state<'grid' | 'list'>('grid');
  let showNewFolder = $state(false);
  let renameTarget = $state<Entry | null>(null);
  let showDelete = $state(false);
  let transfer = $state<{ op: 'move' | 'copy'; paths: string[] } | null>(null);
  let uploads = $state<UploadTask[]>([]);
  let uploadsCollapsed = $state(localStorage.getItem('pf-uploads-collapsed') === '1');
  let uploadQueue: { id: string; file: File; name: string; dir: string }[] = [];
  let activeUploads = 0;
  let uploadFailures = 0;
  let uploadCancellations = 0;
  let dirtyUploadDirs = new Set<string>();
  const uploadControllers = new Map<string, AbortController>();
  let dragActive = $state(false);
  let fileInput: HTMLInputElement | undefined = $state();
  let folderInput: HTMLInputElement | undefined = $state();

  let isFileRoot = $derived(listing?.is_file_root === true);
  let isVirtualRoot = $derived(listing?.is_virtual_root === true);
  let canBrowse = $derived(listing !== null && !isFileRoot && !isVirtualRoot);
  let selectedEntries = $derived(
    listing ? listing.entries.filter((entry) => selected.includes(entry.path)) : [],
  );
  let selectableEntries = $derived(
    listing && !listing.is_file_root && !listing.is_virtual_root
      ? listing.entries.filter((entry) => !entry.restricted)
      : [],
  );
  let allSelected = $derived(
    selectableEntries.length > 0 && selectableEntries.every((entry) => selected.includes(entry.path)),
  );
  let crumbs = $derived.by(() => {
    if (!listing || !me) return [] as { name: string; path: string }[];
    if (listing.is_virtual_root) {
      return [{ name: listing.name, path: ROOTS_PATH }];
    }
    const root =
      me.roots.find((candidate) => isUnder(listing!.path, candidate.path)) ?? me.roots[0];
    if (!root) return [];
    const base = root.path.endsWith('/') ? root.path.slice(0, -1) : root.path;
    const relative = listing.path.slice(base.length).split('/').filter(Boolean);
    const result = [] as { name: string; path: string }[];
    if (me.roots.length > 1) {
      result.push({ name: 'Shared items', path: ROOTS_PATH });
    }
    result.push({ name: root.label, path: root.path });
    let accumulator = base;
    for (const segment of relative) {
      accumulator = `${accumulator}/${segment}`;
      result.push({ name: segment, path: accumulator });
    }
    return result;
  });
  let menuItems = $derived.by(() => [
    {
      label: 'New folder',
      icon: 'folder-plus',
      action: () => (showNewFolder = true),
      disabled: !canBrowse,
    },
    {
      label: 'Upload folder',
      icon: 'folder',
      action: chooseFolder,
      disabled: !canBrowse,
    },
    {
      label: allSelected ? 'Clear selection' : 'Select all',
      icon: 'check',
      action: toggleAll,
      disabled: selectableEntries.length === 0,
    },
    {
      label: view === 'grid' ? 'List view' : 'Grid view',
      icon: view === 'grid' ? 'list' : 'grid',
      action: () => setView(view === 'grid' ? 'list' : 'grid'),
    },
    { label: 'Refresh', icon: 'refresh', action: reload },
    { label: 'Sign out', icon: 'logout', action: handleLogout, danger: true },
  ]);
  let selectionItems = $derived.by(() => [
    {
      label: 'Move to…',
      icon: 'move',
      action: () => (transfer = { op: 'move', paths: [...selected] }),
    },
    {
      label: 'Copy to…',
      icon: 'copy',
      action: () => (transfer = { op: 'copy', paths: [...selected] }),
    },
    ...(selectedEntries.length === 1
      ? [
          {
            label: 'Rename',
            icon: 'pencil',
            action: () => (renameTarget = selectedEntries[0]),
          },
        ]
      : []),
  ]);

  onMount(() => {
    const params = new URLSearchParams(location.search);
    if (params.get('error') === 'pin') {
      loginError = 'Incorrect PIN. Try again.';
    } else if (params.get('error') === 'rate') {
      loginError = 'Too many attempts. Wait a minute.';
    }
    const savedView = localStorage.getItem('pf-view');
    if (savedView === 'grid' || savedView === 'list') {
      view = savedView;
    }
    void bootstrap();
    const onPopState = () => {
      const path = pathFromHash();
      if (path && me && me.roots.length > 0 && path !== listing?.path) {
        void navigate(path, false);
      }
    };
    window.addEventListener('popstate', onPopState);
    return () => window.removeEventListener('popstate', onPopState);
  });

  async function bootstrap() {
    loading = true;
    try {
      me = await loadMe();
      await openInitialListing();
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) {
        me = null;
      } else {
        showToast(errorMessage(error, 'Cannot reach the server'), true);
      }
    } finally {
      loading = false;
    }
  }

  async function openInitialListing() {
    if (!me || me.roots.length === 0) {
      listing = null;
      selected = [];
      history.replaceState(null, '', location.pathname);
      return;
    }
    const hash = pathFromHash();
    const candidates = [
      ...(hash && (hash !== ROOTS_PATH || me.roots.length > 1) ? [hash] : []),
      ...(me.roots.length > 1 ? [ROOTS_PATH] : []),
      ...me.roots.map((root) => root.path),
    ].filter((candidate): candidate is string => Boolean(candidate));
    for (const candidate of [...new Set(candidates)]) {
      await navigate(candidate, false);
      if (listing) break;
    }
    if (listing) {
      history.replaceState(null, '', `#path=${encodeURIComponent(listing.path)}`);
    } else {
      history.replaceState(null, '', location.pathname);
    }
  }

  function pathFromHash(): string | null {
    const hash = location.hash.replace(/^#/, '');
    if (!hash.startsWith('path=')) return null;
    try {
      return decodeURIComponent(hash.slice(5));
    } catch {
      return null;
    }
  }

  function isUnder(path: string, root: string): boolean {
    return path === root || path.startsWith(root.endsWith('/') ? root : `${root}/`);
  }

  async function handleLogin(pin: string) {
    submittingLogin = true;
    loginError = '';
    try {
      me = await login(pin);
      await openInitialListing();
    } catch (error) {
      loginError = errorMessage(error, 'Login failed');
      throw error;
    } finally {
      submittingLogin = false;
    }
  }

  async function navigate(path: string, push = true) {
    if (!me || me.roots.length === 0) return;
    const id = ++navId;
    navAbort?.abort();
    const controller = new AbortController();
    navAbort = controller;
    loadingListing = true;
    try {
      const data =
        path === ROOTS_PATH
          ? await loadRootsListing(controller.signal)
          : await loadListing(path, controller.signal);
      if (id !== navId) return;
      listing = data;
      selected = [];
      if (push) {
        history.pushState(null, '', `#path=${encodeURIComponent(path)}`);
      }
    } catch (error) {
      if (id !== navId || isAbortError(error)) return;
      if (error instanceof ApiError && error.status === 401) {
        me = null;
        loginError = 'Your session expired. Sign in again.';
      } else {
        showToast(errorMessage(error, 'Cannot open folder'), true);
      }
    } finally {
      if (id === navId) {
        loadingListing = false;
        navAbort = null;
      }
    }
  }

  async function refresh() {
    if (listing) {
      const current = listing.path;
      await navigate(current, false);
    }
  }

  async function reload() {
    try {
      me = await loadMe();
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) {
        me = null;
        loginError = 'Your session expired. Sign in again.';
      } else {
        showToast(errorMessage(error, 'Cannot reach the server'), true);
      }
      return;
    }
    if (!me || me.roots.length === 0) {
      listing = null;
      selected = [];
      history.replaceState(null, '', location.pathname);
      return;
    }
    const current = listing?.path ?? pathFromHash();
    if (current && current !== ROOTS_PATH) {
      await navigate(current, false);
      if (listing) return;
    }
    await openInitialListing();
  }

  function openEntry(entry: Entry) {
    if (entry.restricted) {
      showToast('Android/data and Android/obb are not accessible.', true);
      return;
    }
    if (entry.is_dir) {
      void navigate(entry.path);
    } else {
      window.open(rawUrl(entry.path), '_blank', 'noopener');
    }
  }

  function toggleEntry(entry: Entry) {
    if (!listing || listing.is_file_root || listing.is_virtual_root) return;
    if (entry.restricted) {
      showToast('This folder is restricted.', true);
      return;
    }
    if (selected.includes(entry.path)) {
      selected = selected.filter((path) => path !== entry.path);
    } else {
      selected = [...selected, entry.path];
    }
  }

  function toggleAll() {
    if (!listing || listing.is_file_root || listing.is_virtual_root) return;
    if (allSelected) {
      selected = [];
    } else {
      selected = selectableEntries.map((entry) => entry.path);
    }
  }

  function setView(next: 'grid' | 'list') {
    view = next;
    localStorage.setItem('pf-view', next);
  }

  async function handleLogout() {
    try {
      await logout();
    } catch {
      // The local session is dropped regardless.
    }
    me = null;
    listing = null;
    selected = [];
    uploads = [];
    history.replaceState(null, '', location.pathname);
  }

  async function createFolder(name: string) {
    if (!listing) return;
    try {
      await makeFolder(listing.path, name);
      showToast('Folder created');
      await refresh();
    } catch (error) {
      throw new Error(errorMessage(error, 'Cannot create folder'));
    }
  }

  async function renameItemNow(name: string) {
    if (!renameTarget) return;
    try {
      await renameItem(renameTarget.path, name);
      showToast('Renamed');
      await refresh();
    } catch (error) {
      throw new Error(errorMessage(error, 'Cannot rename'));
    }
  }

  async function deleteSelected() {
    try {
      const result = await deleteItems(selected);
      if (result.errors?.length) {
        showToast(result.errors[0], true);
      } else {
        showToast(`Deleted ${selected.length} item${selected.length === 1 ? '' : 's'}`);
      }
      selected = [];
      await refresh();
    } catch (error) {
      throw new Error(errorMessage(error, 'Cannot delete'));
    }
  }

  async function transferSelected(destination: string) {
    if (!transfer) return;
    try {
      const result = await transferItems(transfer.op, transfer.paths, destination);
      if (result.errors?.length) {
        showToast(result.errors[0], true);
      } else {
        showToast(`${transfer.op === 'move' ? 'Moved' : 'Copied'} ${result.paths?.length ?? transfer.paths.length} item(s)`);
      }
      selected = [];
      await refresh();
    } catch (error) {
      throw new Error(errorMessage(error, transfer.op === 'move' ? 'Cannot move' : 'Cannot copy'));
    }
  }

  function downloadSelected() {
    if (!listing || listing.is_file_root || listing.is_virtual_root || selected.length === 0) return;
    if (selectedEntries.length === 1 && !selectedEntries[0].is_dir) {
      const entry = selectedEntries[0];
      const anchor = document.createElement('a');
      anchor.href = downloadUrl(entry.path);
      anchor.download = entry.name;
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      return;
    }
    downloadZip(listing.path, selected);
    showToast('Building ZIP archive…');
  }

  function chooseFiles() {
    fileInput?.click();
  }

  function chooseFolder() {
    folderInput?.click();
  }

  function onFileInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    if (input.files) enqueueFiles(input.files);
    input.value = '';
  }

  function enqueueFiles(files: FileList | File[]) {
    const items = Array.from(files).map((file) => {
      const withPath = file as File & { webkitRelativePath?: string };
      return { file, name: withPath.webkitRelativePath || file.name };
    });
    enqueueUploads(items);
  }

  function enqueueUploads(items: DroppedFile[]) {
    if (!listing || listing.is_file_root || listing.is_virtual_root) return;
    const dir = listing.path;
    if (items.length === 0) return;
    if (activeUploads === 0 && uploadQueue.length === 0) {
      uploadFailures = 0;
      uploadCancellations = 0;
      dirtyUploadDirs = new Set();
    }
    for (const item of items) {
      const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
      uploads = [
        ...uploads,
        {
          id,
          name: item.name,
          loaded: 0,
          total: item.file.size,
          status: 'queued',
        },
      ];
      uploadQueue.push({ id, file: item.file, name: item.name, dir });
    }
    setUploadsCollapsed(false);
    pumpUploads();
  }

  function setUploadsCollapsed(next: boolean) {
    uploadsCollapsed = next;
    localStorage.setItem('pf-uploads-collapsed', next ? '1' : '0');
  }

  function pumpUploads() {
    while (activeUploads < MAX_PARALLEL_UPLOADS && uploadQueue.length > 0) {
      const item = uploadQueue.shift()!;
      activeUploads += 1;
      void runUpload(item);
    }
    if (activeUploads === 0 && uploadQueue.length === 0) {
      void finishUploadBatch();
    }
  }

  async function runUpload(item: { id: string; file: File; name: string; dir: string }) {
    const controller = new AbortController();
    uploadControllers.set(item.id, controller);
    patchUpload(item.id, { status: 'uploading', message: undefined, speed: 0 });
    let lastSampleAt = 0;
    let lastSampleLoaded = 0;
    let hasSample = false;
    let speed = 0;
    try {
      await uploadFile(
        item.file,
        item.name,
        item.dir,
        (loaded, total) => {
          const now = performance.now();
          if (!hasSample) {
            hasSample = true;
            lastSampleAt = now;
            lastSampleLoaded = loaded;
          }
          const elapsed = (now - lastSampleAt) / 1000;
          if (elapsed >= 0.4) {
            const instant = Math.max(0, (loaded - lastSampleLoaded) / elapsed);
            speed = speed === 0 ? instant : speed * 0.7 + instant * 0.3;
            lastSampleAt = now;
            lastSampleLoaded = loaded;
          }
          patchUpload(item.id, { loaded, total, speed });
        },
        controller.signal,
      );
      patchUpload(item.id, {
        status: 'done',
        loaded: item.file.size,
        total: item.file.size,
        speed: 0,
      });
      dirtyUploadDirs.add(item.dir);
    } catch (error) {
      if (error instanceof UploadCancelledError || isAbortError(error)) {
        uploadCancellations += 1;
        patchUpload(item.id, { status: 'cancelled', message: undefined, speed: 0 });
      } else {
        uploadFailures += 1;
        patchUpload(item.id, {
          status: 'error',
          message: errorMessage(error, 'Upload failed'),
          speed: 0,
        });
        if (error instanceof ApiError && error.status === 401) {
          me = null;
          loginError = 'Your session expired. Sign in again.';
        }
      }
    } finally {
      uploadControllers.delete(item.id);
      activeUploads -= 1;
      pumpUploads();
    }
  }

  async function finishUploadBatch() {
    const cancelled = uploadCancellations;
    if (uploadFailures > 0) {
      showToast(
        `${uploadFailures} upload${uploadFailures === 1 ? '' : 's'} failed`,
        true,
      );
    } else if (cancelled > 0) {
      showToast(
        `${cancelled} upload${cancelled === 1 ? '' : 's'} cancelled`,
      );
    } else {
      showToast('Uploads finished');
    }
    const shouldRefresh = listing ? dirtyUploadDirs.has(listing.path) : false;
    dirtyUploadDirs = new Set();
    if (shouldRefresh && me) {
      await refresh();
    }
  }

  function cancelUpload(id: string) {
    const task = uploads.find((item) => item.id === id);
    if (!task) return;
    if (task.status === 'queued') {
      uploadQueue = uploadQueue.filter((item) => item.id !== id);
      uploadCancellations += 1;
      patchUpload(id, { status: 'cancelled', message: undefined, speed: 0 });
      pumpUploads();
    } else if (task.status === 'uploading') {
      uploadControllers.get(id)?.abort();
    }
  }

  function cancelAllUploads() {
    uploadQueue = [];
    for (const task of uploads) {
      if (task.status === 'queued') {
        uploadCancellations += 1;
        patchUpload(task.id, { status: 'cancelled', message: undefined, speed: 0 });
      }
    }
    for (const controller of uploadControllers.values()) {
      controller.abort();
    }
    pumpUploads();
  }

  function patchUpload(id: string, patch: Partial<UploadTask>) {
    uploads = uploads.map((task) => (task.id === id ? { ...task, ...patch } : task));
  }

  async function onDrop(event: DragEvent) {
    event.preventDefault();
    dragActive = false;
    if (!event.dataTransfer) return;
    try {
      const items = await collectDroppedFiles(event.dataTransfer);
      enqueueUploads(items);
    } catch {
      showToast('Could not read the dropped files', true);
    }
  }

  function isAbortError(error: unknown): boolean {
    return error instanceof Error && error.name === 'AbortError';
  }

  function errorMessage(error: unknown, fallback: string): string {
    if (error instanceof ApiError) return error.message;
    if (error instanceof Error && error.message) return error.message;
    return fallback;
  }

  function showToast(message: string, isError = false) {
    toast = message;
    toastError = isError;
    if (toastTimer) window.clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => {
      toast = '';
    }, 4000);
  }

</script>

<svelte:head>
  <title>{listing ? `${listing.name} · Troph` : 'Troph'}</title>
</svelte:head>

{#if loading}
  <div class="loading-screen"><span class="spinner"></span> Loading Troph…</div>
{:else if !me}
  <Login onSubmit={handleLogin} error={loginError} busy={submittingLogin} />
{:else}
  <div
    class="app"
    class:drop-active={dragActive}
    role="presentation"
    ondragover={(event) => {
      event.preventDefault();
      dragActive = true;
    }}
    ondragleave={(event) => {
      if (event.target === event.currentTarget) dragActive = false;
    }}
    ondrop={onDrop}
  >
    {#if loadingListing && listing}
      <div class="loading-bar" role="status" aria-live="polite">
        <span class="bar-fill"></span>
        <span class="sr-only">Loading folder…</span>
      </div>
    {/if}
    <header class="topbar">
      <div class="bar">
        <div class="brand"><Icon name="server" size={22} /><span>Troph</span></div>
        <nav class="breadcrumbs" aria-label="Breadcrumb">
          {#each crumbs as crumb, index (crumb.path)}
            {#if index > 0}<span class="crumb-sep">/</span>{/if}
            <button
              class="crumb"
              class:current={index === crumbs.length - 1}
              onclick={() => void navigate(crumb.path)}
              title={crumb.name}
            >{crumb.name}</button>
          {/each}
        </nav>
        <div class="topbar-actions">
          <button class="button primary" onclick={chooseFiles} disabled={!canBrowse}>
            <Icon name="upload" size={16} /> Upload
          </button>
          <Menu label="More actions" items={menuItems} />
        </div>
      </div>
    </header>

    <main class="content" class:busy={loadingListing && listing !== null} aria-busy={loadingListing}>
      {#if !listing}
        {#if me.roots.length === 0}
          <div class="empty">
            <strong>Nothing is shared yet</strong>
            <p>On the host, open Troph and choose folders or files to share, then restart the server.</p>
            <button class="button small" onclick={reload}>Refresh</button>
          </div>
        {:else if loadingListing}
          <div class="empty"><span class="spinner"></span> Loading…</div>
        {:else}
          <div class="empty">
            <strong>Cannot open shared items</strong>
            <p>The shared folders or files may have moved or been removed. Check the sharing settings on the host.</p>
            <button class="button small" onclick={reload}>Refresh</button>
          </div>
        {/if}
      {:else if listing.entries.length === 0}
        <div class="empty">
          <strong>This folder is empty</strong>
          Upload files or create a folder to get started.
        </div>
      {:else if view === 'grid'}
        <div class="file-grid">
          {#each listing.entries as entry (entry.path)}
            <EntryCard
              {entry}
              selected={selected.includes(entry.path)}
              selectable={!isFileRoot}
              onToggle={() => toggleEntry(entry)}
              onOpen={openEntry}
              onRestricted={() => showToast('This folder is restricted by Android', true)}
            />
          {/each}
        </div>
      {:else}
        <div class="list">
          {#each listing.entries as entry (entry.path)}
            <EntryRow
              {entry}
              selected={selected.includes(entry.path)}
              selectable={!isFileRoot}
              onToggle={() => toggleEntry(entry)}
              onOpen={openEntry}
              onRestricted={() => showToast('This folder is restricted by Android', true)}
            />
          {/each}
        </div>
      {/if}
    </main>

    {#if selected.length > 0 && !isFileRoot && !isVirtualRoot}
      <div class="selection-bar">
        <span class="selection-count"><strong>{selected.length}</strong> selected</span>
        <button class="button small" onclick={downloadSelected} title="Download">
          <Icon name="download" size={15} /> Download
        </button>
        <Menu label="Selection actions" items={selectionItems} up />
        <button class="button small danger" onclick={() => (showDelete = true)} title="Delete">
          <Icon name="trash" size={15} /> Delete
        </button>
        <button class="icon-button" onclick={() => (selected = [])} aria-label="Clear selection">
          <Icon name="close" size={17} />
        </button>
      </div>
    {/if}

    <footer class="footer">
      {me.device_name} · {me.hostname} · v{appVersion}
    </footer>

    <input
      bind:this={fileInput}
      type="file"
      multiple
      class="sr-only"
      onchange={onFileInput}
      tabindex="-1"
    />
    <input
      bind:this={folderInput}
      type="file"
      multiple
      webkitdirectory
      class="sr-only"
      onchange={onFileInput}
      tabindex="-1"
    />

    {#if showNewFolder}
      <TextPrompt
        title="New folder"
        label="Folder name"
        confirmText="Create"
        onSubmit={createFolder}
        onClose={() => (showNewFolder = false)}
      />
    {/if}

    {#if renameTarget}
      <TextPrompt
        title="Rename"
        label={renameTarget.name}
        initial={renameTarget.name}
        confirmText="Rename"
        onSubmit={renameItemNow}
        onClose={() => (renameTarget = null)}
      />
    {/if}

    {#if showDelete}
      <ConfirmDialog
        title="Delete items?"
        message={`Permanently delete ${selected.length} selected item${selected.length === 1 ? '' : 's'}? This cannot be undone.`}
        confirmText="Delete"
        danger
        onConfirm={deleteSelected}
        onClose={() => (showDelete = false)}
      />
    {/if}

    {#if transfer}
      <Picker
        title={transfer.op === 'move' ? 'Move to folder' : 'Copy to folder'}
        confirmText={transfer.op === 'move' ? 'Move here' : 'Copy here'}
        startPath={listing?.path || transfer.paths[0]}
        onConfirm={transferSelected}
        onClose={() => (transfer = null)}
      />
    {/if}

    <UploadPanel
      tasks={uploads}
      collapsed={uploadsCollapsed}
      onToggle={() => setUploadsCollapsed(!uploadsCollapsed)}
      onDismiss={() => (uploads = [])}
      onCancel={cancelUpload}
      onCancelAll={cancelAllUploads}
    />

    {#if toast}
      <div class="toast" class:error={toastError}>{toast}</div>
    {/if}
  </div>
{/if}
