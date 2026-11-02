export type RootKind = 'folder' | 'file';

export interface Root {
  label: string;
  path: string;
  kind: RootKind;
}

export interface DesktopConfig {
  port: number;
  deviceName: string;
  hostname: string;
  roots: Root[];
  closeToTray: boolean;
}

export interface DesktopSnapshot {
  running: boolean;
  url: string | null;
  urls: string[];
  hostnameUrl: string | null;
  pin: string | null;
  version: string;
  deviceName: string;
  port: number;
  roots: Root[];
  config: DesktopConfig;
}

export interface SettingsInput {
  port: number;
  deviceName: string;
  hostname: string;
  closeToTray: boolean;
}
