import { invoke } from '@tauri-apps/api/core';
import type { DesktopSnapshot, SettingsInput } from './types';

export function getState(): Promise<DesktopSnapshot> {
  return invoke('get_state');
}

export function startServer(): Promise<DesktopSnapshot> {
  return invoke('start_server');
}

export function stopServer(): Promise<DesktopSnapshot> {
  return invoke('stop_server');
}

export function addRoot(path: string, label: string | null = null): Promise<DesktopSnapshot> {
  return invoke('add_root', { path, label });
}

export function removeRoot(path: string): Promise<DesktopSnapshot> {
  return invoke('remove_root', { path });
}

export function updateSettings(settings: SettingsInput): Promise<DesktopSnapshot> {
  return invoke('update_settings', { settings });
}

export function openFileManager(): Promise<void> {
  return invoke('open_file_manager');
}

export function cancelTransfer(id: string): Promise<void> {
  return invoke('cancel_transfer', { id });
}
