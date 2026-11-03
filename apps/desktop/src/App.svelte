<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    disable as disableAutostart,
    enable as enableAutostart,
    isEnabled as isAutostartEnabled,
  } from '@tauri-apps/plugin-autostart';
  import QRCode from 'qrcode';
  import {
    addRoot,
    cancelTransfer,
    getState,
    openFileManager,
    removeRoot,
    startServer,
    stopServer,
    updateSettings,
  } from './lib/api';
  import type { DesktopSnapshot, Root, TransferSnapshot } from './lib/types';

  let snapshot = $state<DesktopSnapshot | null>(null);
  let transfers = $state<TransferSnapshot[]>([]);
  let loading = $state(true);
  let busy = $state(false);
  let error = $state('');
  let notice = $state('');
  let qr = $state('');
  let autostart = $state(false);
  let autostartBusy = $state(false);
  let noticeTimer: number | undefined;
  let unlisten: (() => void) | undefined;
  let settings = $state({
    port: 55432,
    deviceName: '',
    hostname: '',
    closeToTray: true,
  });

  let running = $derived(snapshot?.running ?? false);
  let address = $derived(snapshot?.url ?? '');
  let pin = $derived(snapshot?.pin ?? '');
  let roots = $derived(snapshot?.roots ?? []);
  let version = $derived(snapshot?.version ?? '');
  let hostname = $derived(snapshot?.hostnameUrl ?? '');

  function message(value: unknown): string {
    if (typeof value === 'string') return value;
    if (value instanceof Error) return value.message;
    return String(value);
  }

  function showNotice(value: string) {
    notice = value;
    if (noticeTimer !== undefined) window.clearTimeout(noticeTimer);
    noticeTimer = window.setTimeout(() => {
      notice = '';
      noticeTimer = undefined;
    }, 3200);
  }

  function applySnapshot(next: DesktopSnapshot) {
    snapshot = next;
    transfers = next.transfers ?? [];
    settings = {
      port: next.config.port,
      deviceName: next.config.deviceName,
      hostname: next.config.hostname,
      closeToTray: next.config.closeToTray,
    };
    void updateQr();
  }

  async function updateQr() {
    if (!snapshot?.running || !snapshot.url || !snapshot.pin) {
      qr = '';
      return;
    }
    try {
      qr = await QRCode.toDataURL(snapshot.url, {
        width: 320,
        margin: 1,
        color: { dark: '#0b0d10', light: '#ffffff' },
      });
    } catch {
      qr = '';
    }
  }

  async function init() {
    try {
      applySnapshot(await getState());
    } catch (value) {
      error = message(value);
    } finally {
      loading = false;
    }
    try {
      autostart = await isAutostartEnabled();
    } catch {
      autostart = false;
    }
  }

  onMount(() => {
    let disposed = false;

    const setup = async () => {
      const offState = await listen<DesktopSnapshot>('state-changed', (event) => {
        applySnapshot(event.payload);
      });
      const offError = await listen<string>('server-error', (event) => {
        error = event.payload;
        showNotice(event.payload);
      });
      const offTransfers = await listen<TransferSnapshot[]>('transfers-changed', (event) => {
        transfers = event.payload;
      });
      if (disposed) {
        offState();
        offError();
        offTransfers();
        return;
      }
      unlisten = () => {
        offState();
        offError();
        offTransfers();
      };
    };

    void init();
    void setup();

    return () => {
      disposed = true;
      unlisten?.();
      if (noticeTimer !== undefined) window.clearTimeout(noticeTimer);
    };
  });

  async function run(action: () => Promise<DesktopSnapshot>, success?: string) {
    busy = true;
    error = '';
    try {
      applySnapshot(await action());
      if (success) showNotice(success);
    } catch (value) {
      error = message(value);
    } finally {
      busy = false;
    }
  }

  function handleStart() {
    void run(startServer, 'Server started.');
  }

  function handleStop() {
    void run(stopServer, 'Server stopped. Start again to get a new PIN.');
  }

  async function addSharedItem(kind: 'folder' | 'file') {
    error = '';
    try {
      const selected = await open({
        title: kind === 'folder' ? 'Share a folder' : 'Share a file',
        directory: kind === 'folder',
        multiple: false,
      });
      if (typeof selected !== 'string') return;
      await run(
        () => addRoot(selected, null),
        kind === 'folder' ? 'Folder shared.' : 'File shared.',
      );
    } catch (value) {
      error = message(value);
    }
  }

  function handleRemove(root: Root) {
    void run(() => removeRoot(root.path), `${root.label} is no longer shared.`);
  }

  async function handleSaveSettings() {
    await run(
      () =>
        updateSettings({
          port: Number(settings.port),
          deviceName: settings.deviceName,
          hostname: settings.hostname,
          closeToTray: settings.closeToTray,
        }),
      running
        ? 'Settings saved. The server restarted with the same PIN.'
        : 'Settings saved.',
    );
  }

  async function handleOpen() {
    error = '';
    try {
      await openFileManager();
    } catch (value) {
      error = message(value);
    }
  }

  async function handleAutostart(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const next = input.checked;
    autostartBusy = true;
    try {
      if (next) {
        await enableAutostart();
      } else {
        await disableAutostart();
      }
      autostart = await isAutostartEnabled();
      showNotice(next ? 'Troph will launch at login.' : 'Launch at login disabled.');
    } catch (value) {
      autostart = !next;
      error = message(value);
    } finally {
      autostartBusy = false;
    }
  }

  async function copy(value: string, label: string) {
    try {
      await navigator.clipboard.writeText(value);
      showNotice(`${label} copied.`);
    } catch {
      const area = document.createElement('textarea');
      area.value = value;
      area.style.position = 'fixed';
      area.style.opacity = '0';
      document.body.appendChild(area);
      area.select();
      const copied = document.execCommand('copy');
      area.remove();
      if (copied) {
        showNotice(`${label} copied.`);
      } else {
        error = `Could not copy the ${label.toLowerCase()}.`;
      }
    }
  }

  function transferPercent(transfer: TransferSnapshot): number | null {
    if (!transfer.total || transfer.total <= 0) return null;
    return Math.min(100, Math.round((transfer.loaded / transfer.total) * 100));
  }

  function transferMeta(transfer: TransferSnapshot): string {
    const percent = transferPercent(transfer);
    return percent === null ? formatSize(transfer.loaded) : `${percent}%`;
  }

  async function handleCancelTransfer(id: string) {
    try {
      await cancelTransfer(id);
    } catch (error) {
      showNotice(message(error));
    }
  }

  function formatSize(bytes: number): string {
    if (!bytes) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
    const value = bytes / 1024 ** index;
    return `${value >= 100 || index === 0 ? Math.round(value) : value.toFixed(1)} ${units[index]}`;
  }
</script>

{#if loading}
  <div class="loading"><span class="spinner"></span> Loading Troph…</div>
{:else if snapshot}
  <main class="shell">
    <header class="masthead">
      <div class="brand-mark">T</div>
      <div>
        <h1>Troph</h1>
        <p>Your computer's files, in your browser.</p>
      </div>
      <span class="version">v{version}</span>
    </header>

    <section class="card">
      <div class="status-line">
        <span class="dot" class:on={running} class:off={!running}></span>
        <div>
          <h2>{running ? 'Server running' : 'Server stopped'}</h2>
          <p>
            {running
              ? 'Anyone on your network with the address and PIN can browse the shared files.'
              : 'Start the server to make the shared items available on your local network.'}
          </p>
        </div>
      </div>
      {#if error}
        <div class="error">{error}</div>
      {/if}
      <button
        class="button primary wide"
        onclick={running ? handleStop : handleStart}
        disabled={busy || (!running && roots.length === 0)}
      >
        {busy ? 'Working…' : running ? 'Stop server' : 'Start server'}
      </button>
      {#if !running && roots.length === 0}
        <p class="hint">Add at least one folder or file below to start the server.</p>
      {/if}
    </section>

    {#if running && transfers.length > 0}
      <section class="card receiving-card" aria-live="polite">
        <div class="card-title">
          <div class="receiving-title">
            <span class="receiving-dot"></span>
            <h2>
              Receiving {transfers.length === 1 ? '1 file' : `${transfers.length} files`}
            </h2>
          </div>
          <span class="badge">Incoming</span>
        </div>
        <ul class="transfers">
          {#each transfers as transfer (transfer.id)}
            {@const percent = transferPercent(transfer)}
            <li>
              <div class="transfer-name">
                <strong title={transfer.name}>{transfer.name}</strong>
                <span>{transferMeta(transfer)}</span>
                <button
                  class="transfer-cancel"
                  onclick={() => handleCancelTransfer(transfer.id)}
                  aria-label={`Cancel ${transfer.name}`}
                  title="Cancel transfer"
                >
                  Cancel
                </button>
              </div>
              <div class="transfer-progress" class:indeterminate={percent === null}>
                <div style={`width:${percent ?? 35}%`}></div>
              </div>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if running}
      <section class="card">
        <div class="card-title">
          <h2>Connect</h2>
          <span class="badge">Same network</span>
        </div>
        <div class="field">
          <span class="field-label">Address</span>
          <div class="value-row">
            <code>{address}</code>
            <button class="button small" onclick={() => copy(address, 'Address')}>Copy</button>
          </div>
          {#if hostname}<p class="muted mono small-text">{hostname}</p>{/if}
        </div>
        <div class="field">
          <span class="field-label">PIN</span>
          <div class="value-row">
            <strong class="pin">{pin}</strong>
            <button class="button small" onclick={() => copy(pin, 'PIN')}>Copy</button>
          </div>
        </div>
        {#if qr}
          <img class="qr" src={qr} alt="QR code containing the Troph address" />
        {/if}
        <button class="button wide" onclick={handleOpen} disabled={busy}>
          Open file manager in browser
        </button>
      </section>
    {/if}

    <section class="card">
      <div class="card-title">
        <h2>Shared items</h2>
        <span class="badge">{roots.length}</span>
      </div>
      {#if roots.length === 0}
        <p class="empty">No folders or files are shared yet.</p>
      {:else}
        <ul class="roots">
          {#each roots as root (root.path)}
            <li>
              <div class="root-icon" class:file={root.kind === 'file'}>
                {root.kind === 'file' ? 'FILE' : 'DIR'}
              </div>
              <div class="root-copy">
                <strong>{root.label}</strong>
                <span>{root.path}</span>
              </div>
              <button
                class="button ghost small"
                onclick={() => handleRemove(root)}
                disabled={busy}
                aria-label={`Remove ${root.label}`}
              >
                Remove
              </button>
            </li>
          {/each}
        </ul>
      {/if}
      <div class="row-actions">
        <button class="button" onclick={() => addSharedItem('folder')} disabled={busy}>
          Add folder
        </button>
        <button class="button" onclick={() => addSharedItem('file')} disabled={busy}>
          Add file
        </button>
      </div>
    </section>

    <section class="card">
      <div class="card-title">
        <h2>Settings</h2>
      </div>
      <div class="grid-2">
        <label class="field">
          <span class="field-label">Port</span>
          <input type="number" min="1" max="65535" bind:value={settings.port} />
        </label>
        <label class="field">
          <span class="field-label">Device name</span>
          <input type="text" bind:value={settings.deviceName} placeholder="Troph" />
        </label>
      </div>
      <label class="field">
        <span class="field-label">Hostname</span>
        <input type="text" bind:value={settings.hostname} placeholder="troph-desktop" />
        <span class="muted small-text">
          Advertised as <code>{settings.hostname || 'automatic'}.local</code>
        </span>
      </label>
      <label class="toggle">
        <input type="checkbox" bind:checked={settings.closeToTray} />
        <span>
          <strong>Keep running in the tray</strong>
          <small>Closing the window hides Troph instead of stopping the server.</small>
        </span>
      </label>
      <label class="toggle">
        <input
          type="checkbox"
          checked={autostart}
          onchange={handleAutostart}
          disabled={autostartBusy}
        />
        <span>
          <strong>Launch at login</strong>
          <small>Start the control panel when you sign in.</small>
        </span>
      </label>
      <button class="button primary" onclick={handleSaveSettings} disabled={busy}>
        Save settings
      </button>
      {#if running}
        <p class="hint">
          Saving restarts the server with the same PIN, so open browser sessions stay signed in.
        </p>
      {/if}
    </section>

    <footer>
      <p>Files stay on your network. Troph does not use the cloud.</p>
    </footer>
  </main>
{/if}

{#if notice}
  <div class="toast">{notice}</div>
{/if}
