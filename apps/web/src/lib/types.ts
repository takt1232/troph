export type RootKind = 'folder' | 'file';

export interface Root {
  label: string;
  path: string;
  kind: RootKind;
}

export interface Entry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified: number;
  mime: string;
  kind: string;
  has_thumb: boolean;
  restricted: boolean;
}

export interface Listing {
  path: string;
  name: string;
  parent: string | null;
  root_label: string;
  entries: Entry[];
  is_file_root: boolean;
  is_virtual_root: boolean;
}

export interface Me {
  csrf: string;
  version: string;
  hostname: string;
  device_name: string;
  roots: Root[];
}

export interface OpResult {
  ok: boolean;
  paths?: string[];
  errors?: string[];
}

export interface UploadTask {
  id: string;
  name: string;
  loaded: number;
  total: number;
  speed?: number;
  status: 'queued' | 'uploading' | 'done' | 'error' | 'cancelled';
  message?: string;
}
