import type { ClipboardItem } from "./types";

export interface CalendarCell {
  day: number;
  inMonth: boolean;
  key: string;
  dateStr: string;
}

/** Formats a Date into local YYYY-MM-DD without UTC timezone shift. */
export function toLocalDateKey(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

/** Builds a 6x7 Monday-first calendar grid for (year, zero-based month). */
export function buildMonthCells(year: number, month: number): CalendarCell[] {
  const firstDay = new Date(year, month, 1);
  const startOffset = (firstDay.getDay() + 6) % 7;
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  const prevMonthDays = new Date(year, month, 0).getDate();
  const cells: CalendarCell[] = [];

  for (let i = 0; i < 42; i++) {
    const dayNum = i - startOffset + 1;
    if (dayNum <= 0) {
      const d = prevMonthDays + dayNum;
      const dt = new Date(year, month - 1, d);
      cells.push({ day: d, inMonth: false, key: `p-${i}`, dateStr: toLocalDateKey(dt) });
    } else if (dayNum > daysInMonth) {
      const d = dayNum - daysInMonth;
      const dt = new Date(year, month + 1, d);
      cells.push({ day: d, inMonth: false, key: `n-${i}`, dateStr: toLocalDateKey(dt) });
    } else {
      const dt = new Date(year, month, dayNum);
      cells.push({ day: dayNum, inMonth: true, key: `c-${dayNum}`, dateStr: toLocalDateKey(dt) });
    }
  }
  return cells;
}

export function clampSelectionIndex(idx: number, len: number): number {
  if (len <= 0 || !Number.isFinite(idx)) return 0;
  return Math.max(0, Math.min(Math.trunc(idx), len - 1));
}

export function computeDockDimensions(params: {
  itemCount: number;
  iconSize: number;
  measuredPillWidth: number;
  showDockClock: boolean;
  popover: "none" | "mixer" | "hidden" | "downloads" | "control" | "calendar";
  hasPreview: boolean;
  maxScreenWidth: number;
}): { width: number; height: number } {
  const maxW = Math.max(300, Math.min(1040, Math.floor(params.maxScreenWidth - 16)));
  const slotCount = Math.max(params.itemCount, 1) + 3;
  const extraClock = params.showDockClock ? 110 : 0;
  const formulaWidth = slotCount * (params.iconSize + 10) + 110 + extraClock;
  const domWidth = params.measuredPillWidth > 0 ? params.measuredPillWidth + 72 : 0;
  const baseWidth = Math.max(formulaWidth, domWidth);

  let width: number;
  if (params.popover === "calendar") {
    width = Math.min(Math.max(baseWidth, 550), maxW);
  } else if (params.popover !== "none") {
    width = Math.min(Math.max(baseWidth, 400), maxW);
  } else if (params.hasPreview) {
    width = Math.min(Math.max(baseWidth, 360), maxW);
  } else {
    width = Math.min(Math.max(baseWidth, 320), maxW);
  }

  const height = params.popover !== "none" ? 416 : params.hasPreview ? 274 : 108;
  return { width, height };
}

export function filterClipboardItems(
  clips: ClipboardItem[],
  filter: "all" | "text" | "image",
  query: string
): ClipboardItem[] {
  const q = query.trim().toLowerCase();
  return clips.filter((c) => {
    if (filter === "image" && c.kind !== "image") return false;
    if (filter === "text" && c.kind === "image") return false;
    if (!q) return true;
    return c.content.toLowerCase().includes(q);
  });
}

export function formatMediaTime(sec: number): string {
  if (!Number.isFinite(sec) || sec < 0) return "0:00";
  const safe = Math.floor(sec);
  const m = Math.floor(safe / 60);
  const s = safe % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

export function formatClipTime(timestampMs: number): string {
  if (!Number.isFinite(timestampMs) || timestampMs <= 1_000_000_000_000) return "Recent";
  return new Date(timestampMs).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  if (bytes < 1024) return `${Math.round(bytes)} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function isValidHttpsUrl(url: string): boolean {
  const trimmed = url.trim();
  if (!trimmed.startsWith("https://")) return false;
  const rest = trimmed.slice(8);
  const host = rest.split("/")[0]?.trim() ?? "";
  return host.length > 0 && !/\s/.test(host);
}
