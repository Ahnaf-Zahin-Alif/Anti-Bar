export interface PinnedApp {
  id: string;
  name: string;
  path: string;
  args?: string | null;
  iconDataUrl?: string | null;
}

export interface MemoTodoItem {
  id: string;
  text: string;
  done: boolean;
}

export interface AgendaItem {
  id: string;
  date: string;
  time: string;
  title: string;
}

export interface AppConfig {
  language: "en" | string;
  theme: "dark" | "light" | string;
  glassMaterial: "frosted" | "liquid" | "matte" | "classic" | string;
  wallpaperTint: boolean;
  iconStyle: "windows" | "squircle" | "normalized" | string;
  iconSize: number;
  magnification: number;
  autoHideDock: boolean;
  hideInFullscreen: boolean;
  hideWindowsTaskbar: boolean;
  monitorMode: "primary" | "cursor" | "custom" | string;
  monitorName: string;
  showDockClock: boolean;
  genieEffect: boolean;
  useWinKeyForLaunchpad: boolean;
  notchEnabled: boolean;
  showLyrics: boolean;
  downloadsView: "grid" | "list" | string;
  pinnedApps: PinnedApp[];
  clockEnabled: boolean;
  clockStyle: "liquid" | "glass" | "sonoma" | string;
  clockAlign: "left" | "center" | "right" | string;
  clockSize: number;
  clockTopOffset: number;
  vinylEnabled: boolean;
  vinylStyle: "glass" | "nothingDark" | "nothingLight" | string;
  calendarEnabled: boolean;
  calendarStyle: "glass" | "nothingDark" | "nothingLight" | string;
  calendarLayout: "agenda" | "square" | string;
  calendarIcsUrl: string;
  memoEnabled: boolean;
  memoStyle: "yellow" | "glass" | "nothingDark" | "nothingLight" | string;
  memoTitle: string;
  memoTodos: MemoTodoItem[];
  agendaItems: AgendaItem[];
}

export interface RunningWindow {
  hwnd: number;
  title: string;
  exePath: string;
  exeName: string;
  isFocused: boolean;
  isMinimized: boolean;
  iconDataUrl?: string | null;
}

export interface DownloadEntry {
  name: string;
  path: string;
  isDir: boolean;
  sizeBytes: number;
  modifiedMs: number;
}

export interface TrashInfo {
  items: number;
  sizeBytes: number;
}

export interface SpotlightEntry {
  id: string;
  name: string;
  subtitle: string;
  path: string;
  kind: "app" | "system" | "math" | string;
  iconDataUrl?: string | null;
}

export interface SpotlightSearchResult {
  mathAnswer?: string | null;
  entries: SpotlightEntry[];
}

export interface ClipboardItem {
  id: string;
  kind: "text" | "url" | "image" | string;
  content: string;
  imageDataUrl?: string | null;
  pinned: boolean;
  timestampMs: number;
}

export interface MediaState {
  title: string;
  artist: string;
  album: string;
  isPlaying: boolean;
  positionSec: number;
  durationSec: number;
  hasSession: boolean;
  thumbnailDataUrl?: string | null;
}

export interface AudioOutputDevice {
  id: string;
  name: string;
  isDefault: boolean;
}

export interface WindowsNotificationItem {
  id: number;
  appName: string;
  title: string;
  body: string;
  timestampMs: number;
}

export interface NotificationsSnapshot {
  status: "allowed" | "denied" | "unspecified" | "unsupported" | string;
  items: WindowsNotificationItem[];
}

export interface LyricLine {
  timeSec: number;
  text: string;
}

export interface LyricsResult {
  found: boolean;
  synced: LyricLine[];
  plain: string;
}

export interface AudioSessionInfo {
  pid: number;
  name: string;
  volume: number;
  muted: boolean;
  isMaster: boolean;
}

export interface WifiNetwork {
  ssid: string;
  signalQuality: number;
  secured: boolean;
  connected: boolean;
}

export interface SystemStatus {
  batteryPercent: number;
  acOnline: boolean;
  hasBattery: boolean;
}

export interface IcsEvent {
  summary: string;
  dtstart: string;
}

export interface MonitorInfo {
  name: string;
  width: number;
  height: number;
  isPrimary: boolean;
}

export interface BluetoothStatus {
  available: boolean;
  enabled: boolean;
}

export interface TrayIconInfo {
  id: string;
  title: string;
  rectLeft: number;
  rectTop: number;
  rectRight: number;
  rectBottom: number;
}

export interface HitboxRect {
  x: number;
  y: number;
  width: number;
  height: number;
}
