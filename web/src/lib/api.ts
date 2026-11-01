import type { Listing, Me, OpResult } from './types';

export const ROOTS_PATH = '@roots';

export class ApiError extends Error {
  status: number;

  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

let csrfToken = '';

export function setCsrf(token: string) {
  csrfToken = token;
}

async function request<T>(url: string, options: RequestInit = {}): Promise<T> {
  const response = await fetch(url, {
    credentials: 'same-origin',
    ...options,
  });
  if (response.status === 401) {
    throw new ApiError(401, 'Login required');
  }
  const text = await response.text();
  let data: unknown = null;
  if (text) {
    try {
      data = JSON.parse(text);
    } catch {
      data = null;
    }
  }
  if (!response.ok) {
    const message =
      data && typeof data === 'object' && 'error' in data
        ? String((data as { error: unknown }).error)
        : text || `Request failed (${response.status})`;
    throw new ApiError(response.status, message);
  }
  return data as T;
}

function jsonPost<T>(url: string, body: unknown): Promise<T> {
  return request<T>(url, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      'x-csrf-token': csrfToken,
    },
    body: JSON.stringify(body),
  });
}

export async function login(pin: string): Promise<Me> {
  await request<{ ok: boolean }>('/api/login', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ pin }),
  });
  return loadMe();
}

export async function loadMe(): Promise<Me> {
  const me = await request<Me>('/api/me');
  setCsrf(me.csrf);
  return me;
}

export async function loadListing(path: string, signal?: AbortSignal): Promise<Listing> {
  return request<Listing>(`/api/list?path=${encodeURIComponent(path)}`, { signal });
}

export async function loadRootsListing(signal?: AbortSignal): Promise<Listing> {
  return request<Listing>('/api/list/roots', { signal });
}

export function thumbUrl(path: string, size = 320): string {
  return `/api/thumb?path=${encodeURIComponent(path)}&size=${size}`;
}

export function rawUrl(path: string): string {
  return `/api/raw?path=${encodeURIComponent(path)}`;
}

export function downloadUrl(path: string): string {
  return `/api/raw?path=${encodeURIComponent(path)}&dl=1`;
}

export async function makeFolder(path: string, name: string): Promise<OpResult> {
  return jsonPost<OpResult>('/api/mkdir', { path, name });
}

export async function renameItem(path: string, name: string): Promise<OpResult> {
  return jsonPost<OpResult>('/api/rename', { path, name });
}

export async function deleteItems(paths: string[]): Promise<OpResult> {
  return jsonPost<OpResult>('/api/delete', { paths });
}

export async function transferItems(
  operation: 'move' | 'copy',
  paths: string[],
  dest: string,
): Promise<OpResult> {
  return jsonPost<OpResult>(`/api/${operation}`, { paths, dest });
}

export async function logout(): Promise<void> {
  await jsonPost('/api/logout', {});
}

export function uploadFile(
  file: File,
  relativeName: string,
  dir: string,
  onProgress: (loaded: number, total: number) => void,
): Promise<{ ok: boolean; path: string; name: string; size: number }> {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open(
      'POST',
      `/api/upload?dir=${encodeURIComponent(dir)}&name=${encodeURIComponent(relativeName)}`,
    );
    xhr.setRequestHeader('x-csrf-token', csrfToken);
    xhr.upload.onprogress = (event) => {
      if (event.lengthComputable) {
        onProgress(event.loaded, event.total);
      }
    };
    xhr.onerror = () => reject(new ApiError(0, 'Network error during upload'));
    xhr.onload = () => {
      let data: any = null;
      try {
        data = JSON.parse(xhr.responseText || '{}');
      } catch {
        data = null;
      }
      if (xhr.status >= 200 && xhr.status < 300) {
        resolve(data);
      } else if (xhr.status === 401) {
        reject(new ApiError(401, 'Login required'));
      } else {
        reject(new ApiError(xhr.status, data?.error || `Upload failed (${xhr.status})`));
      }
    };
    xhr.send(file);
  });
}

export function downloadZip(base: string, paths: string[]) {
  const frameName = `troph-zip-${Date.now()}`;
  const frame = document.createElement('iframe');
  frame.name = frameName;
  frame.style.display = 'none';
  document.body.appendChild(frame);

  const form = document.createElement('form');
  form.method = 'POST';
  form.action = '/api/zip';
  form.target = frameName;
  form.style.display = 'none';

  const add = (name: string, value: string) => {
    const input = document.createElement('input');
    input.type = 'hidden';
    input.name = name;
    input.value = value;
    form.appendChild(input);
  };
  add('base', base);
  add('csrf', csrfToken);
  for (const path of paths) {
    add('path', path);
  }

  document.body.appendChild(form);
  form.submit();
  form.remove();
  window.setTimeout(() => frame.remove(), 120_000);
}
