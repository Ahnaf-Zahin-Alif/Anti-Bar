import assert from "node:assert/strict";
import test from "node:test";
import {
  buildMonthCells,
  clampSelectionIndex,
  computeDockDimensions,
  filterClipboardItems,
  formatBytes,
  formatClipTime,
  formatMediaTime,
  isValidHttpsUrl,
  toLocalDateKey,
} from "../src/utils.ts";

test("toLocalDateKey formats local dates without UTC rollover", () => {
  assert.equal(toLocalDateKey(new Date(2026, 10, 25, 23, 59, 59)), "2026-11-25");
  assert.equal(toLocalDateKey(new Date(2024, 1, 29, 0, 5, 0)), "2024-02-29");
  assert.equal(toLocalDateKey(new Date(2026, 0, 1, 0, 0, 0)), "2026-01-01");
});

test("buildMonthCells covers indices 0..41 and Monday-first boundaries", () => {
  const nov2026 = buildMonthCells(2026, 10);
  assert.equal(nov2026.length, 42);
  // Nov 1, 2026 is Sunday -> Monday-first offset is 6 -> indices 0..5 are Oct 26..31
  assert.deepEqual(
    { day: nov2026[0].day, inMonth: nov2026[0].inMonth, dateStr: nov2026[0].dateStr },
    { day: 26, inMonth: false, dateStr: "2026-10-26" },
  );
  assert.deepEqual(
    { day: nov2026[6].day, inMonth: nov2026[6].inMonth, dateStr: nov2026[6].dateStr },
    { day: 1, inMonth: true, dateStr: "2026-11-01" },
  );
  assert.equal(nov2026[35].day, 30);
  assert.equal(nov2026[35].inMonth, true);
  assert.equal(nov2026[41].day, 6);
  assert.equal(nov2026[41].inMonth, false);
});

test("clampSelectionIndex handles empty, bounds, negatives, and NaN", () => {
  assert.equal(clampSelectionIndex(0, 0), 0);
  assert.equal(clampSelectionIndex(-1, 0), 0);
  assert.equal(clampSelectionIndex(5, 1), 0);
  assert.equal(clampSelectionIndex(-3, 5), 0);
  assert.equal(clampSelectionIndex(2, 5), 2);
  assert.equal(clampSelectionIndex(4, 5), 4);
  assert.equal(clampSelectionIndex(99, 5), 4);
  assert.equal(clampSelectionIndex(Number.NaN, 5), 0);
  assert.equal(clampSelectionIndex(Number.POSITIVE_INFINITY, 5), 0);
});

test("computeDockDimensions clamps cleanly across 0..25 items and small viewports", () => {
  const emptyDock = computeDockDimensions({
    itemCount: 0,
    iconSize: 44,
    measuredPillWidth: 0,
    showDockClock: false,
    popover: "none",
    hasPreview: false,
    maxScreenWidth: 1920,
  });
  assert.equal(emptyDock.height, 108);
  assert.ok(emptyDock.width >= 320 && emptyDock.width <= 1040);

  const crowdedSmall = computeDockDimensions({
    itemCount: 25,
    iconSize: 64,
    measuredPillWidth: 1800,
    showDockClock: true,
    popover: "calendar",
    hasPreview: false,
    maxScreenWidth: 800,
  });
  assert.equal(crowdedSmall.width, 784);
  assert.equal(crowdedSmall.height, 416);

  const previewDock = computeDockDimensions({
    itemCount: 4,
    iconSize: 44,
    measuredPillWidth: 380,
    showDockClock: false,
    popover: "none",
    hasPreview: true,
    maxScreenWidth: 1920,
  });
  assert.equal(previewDock.height, 274);
});

test("filterClipboardItems handles kinds, special characters, and case folding", () => {
  const items = [
    { id: "1", kind: "text" as const, content: "const x = [a + b] * (c);", timestampMs: 1700000000000 },
    { id: "2", kind: "image" as const, content: "data:image/png;base64,AAA=", timestampMs: 1700000001000 },
  ];
  assert.equal(filterClipboardItems(items, "all", "").length, 2);
  assert.equal(filterClipboardItems(items, "image", "").length, 1);
  assert.equal(filterClipboardItems(items, "text", "[A + B]").length, 1);
  assert.equal(filterClipboardItems([], "all", "abc").length, 0);
});

test("formatting and https url validators guard malformed and boundary values", () => {
  assert.equal(formatMediaTime(-5), "0:00");
  assert.equal(formatMediaTime(Number.NaN), "0:00");
  assert.equal(formatMediaTime(65.9), "1:05");

  assert.equal(formatClipTime(0), "Recent");
  assert.equal(formatClipTime(Number.NaN), "Recent");
  assert.notEqual(formatClipTime(1700000000000), "Recent");

  assert.equal(formatBytes(-10), "0 B");
  assert.equal(formatBytes(512), "512 B");
  assert.equal(formatBytes(1536), "1.5 KB");
  assert.equal(formatBytes(3 * 1024 * 1024), "3.0 MB");

  assert.equal(isValidHttpsUrl(""), false);
  assert.equal(isValidHttpsUrl("http://example.com/a.ics"), false);
  assert.equal(isValidHttpsUrl("https://"), false);
  assert.equal(isValidHttpsUrl("https://bad host/a.ics"), false);
  assert.equal(isValidHttpsUrl("https://calendar.google.com/calendar/ical/basic.ics"), true);
});
