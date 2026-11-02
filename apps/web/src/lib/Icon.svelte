<script lang="ts">
  let {
    name,
    size = 20,
  }: {
    name: string;
    size?: number;
  } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === 'folder'}
    <path d="M3 7.5A2.5 2.5 0 0 1 5.5 5h4l2 2h7A2.5 2.5 0 0 1 21 9.5v7A2.5 2.5 0 0 1 18.5 19h-13A2.5 2.5 0 0 1 3 16.5z" />
  {:else if name === 'image'}
    <rect x="3" y="4" width="18" height="16" rx="2.5" />
    <circle cx="9" cy="9.5" r="1.6" />
    <path d="m21 15-4.5-4.5L7 20" />
  {:else if name === 'video'}
    <rect x="3" y="5" width="18" height="14" rx="2.5" />
    <path d="m10 9.2 5 2.8-5 2.8z" />
  {:else if name === 'audio'}
    <path d="M9 18V6l10-2v12" />
    <circle cx="6.5" cy="18" r="2.5" />
    <circle cx="16.5" cy="16" r="2.5" />
  {:else if name === 'pdf'}
    <path d="M6 3h8l4 4v14H6z" />
    <path d="M14 3v5h5" />
    <path d="M9 14h6M9 17h4" />
  {:else if name === 'text'}
    <path d="M6 3h8l4 4v14H6z" />
    <path d="M14 3v5h5M9 12h6M9 15h6M9 18h4" />
  {:else if name === 'archive'}
    <rect x="4" y="3" width="16" height="18" rx="2" />
    <path d="M10 3v4m4-4v4m-4 4v3m4-3v3" />
    <rect x="9.5" y="14" width="5" height="4" rx="1" />
  {:else if name === 'app'}
    <rect x="6" y="3" width="12" height="18" rx="3" />
    <path d="M10 18h4" />
  {:else if name === 'file'}
    <path d="M6 3h8l4 4v14H6zM14 3v5h5" />
  {:else if name === 'upload'}
    <path d="M12 16V4m0 0L7 9m5-5 5 5" />
    <path d="M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" />
  {:else if name === 'folder-plus'}
    <path d="M3 7.5A2.5 2.5 0 0 1 5.5 5h4l2 2h7A2.5 2.5 0 0 1 21 9.5v7A2.5 2.5 0 0 1 18.5 19h-13A2.5 2.5 0 0 1 3 16.5z" />
    <path d="M12 11v5m-2.5-2.5h5" />
  {:else if name === 'download'}
    <path d="M12 4v11m0 0 4-4m-4 4-4-4" />
    <path d="M4 17v1a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-1" />
  {:else if name === 'trash'}
    <path d="M4 7h16M9 7V5h6v2m-8 0 1 13h8l1-13" />
    <path d="M10 11v5m4-5v5" />
  {:else if name === 'move'}
    <path d="M12 3v18M3 12h18" />
    <path d="m8 7 4-4 4 4M8 17l4 4 4-4M7 8 3 12l4 4m10-8 4 4-4 4" />
  {:else if name === 'copy'}
    <rect x="8" y="8" width="12" height="13" rx="2" />
    <path d="M16 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h3" />
  {:else if name === 'close'}
    <path d="m6 6 12 12M18 6 6 18" />
  {:else if name === 'refresh'}
    <path d="M20 11a8 8 0 1 0-2.3 6.3" />
    <path d="M20 4v7h-7" />
  {:else if name === 'grid'}
    <rect x="3" y="3" width="7" height="7" rx="1.5" />
    <rect x="14" y="3" width="7" height="7" rx="1.5" />
    <rect x="3" y="14" width="7" height="7" rx="1.5" />
    <rect x="14" y="14" width="7" height="7" rx="1.5" />
  {:else if name === 'list'}
    <path d="M8 6h13M8 12h13M8 18h13" />
    <circle cx="4" cy="6" r="1" fill="currentColor" />
    <circle cx="4" cy="12" r="1" fill="currentColor" />
    <circle cx="4" cy="18" r="1" fill="currentColor" />
  {:else if name === 'check'}
    <path d="m5 12 4.5 4.5L19 7" />
  {:else if name === 'lock'}
    <rect x="4" y="10" width="16" height="11" rx="2.5" />
    <path d="M8 10V7a4 4 0 0 1 8 0v3" />
  {:else if name === 'logout'}
    <path d="M10 4H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h4" />
    <path d="M15 8l4 4-4 4m4-4H9" />
  {:else if name === 'chevron-left'}
    <path d="m14 6-6 6 6 6" />
  {:else if name === 'chevron-right'}
    <path d="m10 6 6 6-6 6" />
  {:else if name === 'chevron-up'}
    <path d="m6 15 6-6 6 6" />
  {:else if name === 'chevron-down'}
    <path d="m6 9 6 6 6-6" />
  {:else if name === 'server'}
    <rect x="3" y="4" width="18" height="7" rx="2" />
    <rect x="3" y="13" width="18" height="7" rx="2" />
    <path d="M7 7.5h.01M7 16.5h.01" />
  {:else if name === 'more'}
    <circle cx="12" cy="5" r="1.5" fill="currentColor" stroke="none" />
    <circle cx="12" cy="12" r="1.5" fill="currentColor" stroke="none" />
    <circle cx="12" cy="19" r="1.5" fill="currentColor" stroke="none" />
  {:else if name === 'pencil'}
    <path d="M4 20h4l10-10a2.8 2.8 0 0 0-4-4L4 16z" />
    <path d="m13.5 6.5 4 4" />
  {:else}
    <path d="M6 3h8l4 4v14H6zM14 3v5h5" />
  {/if}
</svg>
