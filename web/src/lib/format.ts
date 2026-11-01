import type { Entry } from './types';

export function formatSize(bytes: number): string {
  if (!bytes) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** index;
  return `${value >= 100 || index === 0 ? Math.round(value) : value.toFixed(1)} ${units[index]}`;
}

export function formatDate(seconds: number): string {
  if (!seconds) return '';
  const date = new Date(seconds * 1000);
  const now = Date.now();
  const diff = now - date.getTime();
  if (diff < 86_400_000 && diff > -86_400_000) {
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }
  if (date.getFullYear() === new Date().getFullYear()) {
    return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
  }
  return date.toLocaleDateString([], { year: 'numeric', month: 'short', day: 'numeric' });
}

export function kindLabel(entry: Entry): string {
  if (entry.is_dir) return 'Folder';
  const labels: Record<string, string> = {
    image: 'Image',
    video: 'Video',
    audio: 'Audio',
    pdf: 'PDF',
    text: 'Text',
    archive: 'Archive',
    app: 'App',
  };
  return labels[entry.kind] || 'File';
}

export function iconFor(entry: Entry): string {
  if (entry.is_dir) return 'folder';
  return entry.kind in { image: 1, video: 1, audio: 1, pdf: 1, text: 1, archive: 1, app: 1 }
    ? entry.kind
    : 'file';
}
