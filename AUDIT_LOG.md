# Anti-Bar Stage 1 Audit Log (Read-Only)

## Baseline Checks
- `cd app && npx tsc --noEmit`: **PASS** (`0` errors, exit `0`)
- `cd app/src-tauri && cargo clippy --all-targets`: **PASS** (`0` errors, `0` warnings, exit `0`)
- `cd app/src-tauri && cargo test --bin anti-bar`: **PASS** (`13 passed; 0 failed; 0 ignored`, exit `0`; linker warning: `.rsrc merge failure: multiple non-default manifests`)

## Stage 1 Findings

| ID | Severity | File:Line | Category | Issue & Fix |
| :--- | :--- | :--- | :--- | :--- |
| S1-01 | HIGH | `app/src-tauri/src/lib.rs:702-708,865-869` | Crash / Cleanup | `TaskbarRestoreGuard` (`Drop`) and `RunEvent::Exit` restore `Shell_TrayWnd` on graceful exit/unwind, but abnormal crashes (`TerminateProcess`, access violations, panic=`abort`) leave the Windows taskbar hidden. **Fix:** Register `std::panic::set_hook` + Win32 `SetUnhandledExceptionFilter` calling `restore_taskbar_if_hidden()`, and track a persisted hidden-taskbar lockfile to auto-restore on recovery. |
| S1-02 | MEDIUM | `app/src-tauri/src/config.rs:193-213` | Config I/O | `save_config` uses non-atomic `fs::write` (truncates `config.json` on mid-write crash) and `load_config` silently overwrites/discards corrupted JSON via `unwrap_or_default()`. **Fix:** Write to `config.json.tmp` + `fs::rename`, and back up unparseable JSON to `config.corrupt.json` before falling back to defaults. |
| S1-03 | MEDIUM | `app/src-tauri/src/config.rs:17-32` | Serialization | `AppConfig` has `#[serde(default)]`, but nested `MemoTodoItem` and `AgendaItem` lack `#[serde(default)]`; a single missing nested field causes full `AppConfig` deserialization failure. **Fix:** Add `#[serde(default)]` + `Default` derive on `MemoTodoItem` and `AgendaItem`. |
| S1-04 | MEDIUM | `app/src-tauri/src/lib.rs:42-56,243-274` | IPC / Threading | `update_config` and `search_spotlight` are synchronous `#[tauri::command]` handlers performing disk I/O (`save_config`), Start Menu directory crawling, and icon extraction on the IPC thread. **Fix:** Convert both to `async fn` using `tauri::async_runtime::spawn_blocking`. |
| S1-05 | LOW | `app/src/types.ts:23` / `app/src-tauri/src/config.rs:37,190` | Type / Contract | `AppConfig.language` is typed `"auto" \| "en" \| "id" \| string` while `sanitize_config` forces `"en"`. **Fix:** Narrow TS `language: "en"` (or `string`) and update stale comment in `config.rs:37`. |
| S1-06 | LOW | `app/src-tauri/src/lib.rs:817` | Error Handling | `app.default_window_icon().unwrap()` in `TrayIconBuilder::new()` panics at startup if the embedded icon is missing. **Fix:** Handle `None` gracefully via `.ok_or("Missing app icon")?` or fallback icon. |

## Stage 2 Findings

| ID | Severity | File:Line | Category | Issue & Fix |
| :--- | :--- | :--- | :--- | :--- |
| S2-01 | HIGH | `app/src-tauri/src/clipboard.rs:213-217,353-397` | Memory / Resource Leak | `GlobalAlloc(GMEM_MOVEABLE, ...)` leaks `hmem` (up to `67 MB` for PNG) on `GlobalLock` failure, `_ => Err` (`393`), or `SetClipboardData` error (`217`, `396`) because Windows does not own `hmem` until `SetClipboardData` succeeds. **Fix:** Wrap `HGLOBAL` in a RAII guard calling `GlobalFree` on `Drop` unless disarmed after `SetClipboardData` succeeds. |
| S2-02 | HIGH | `app/src-tauri/src/audio_mixer.rs:339-388` | COM / ABI Safety | `set_default_output_device` relies on hardcoded undocumented `IPolicyConfig` vtable slot indices (`_slots: [usize; 10]`, slot 13); a Windows COM vtable layout change causes UB/crash. **Fix:** Document/isolate the undocumented ABI surface behind an explicit version/QI guard and validate endpoint existence first. |
| S2-03 | HIGH | `app/src-tauri/src/system_panels.rs:285-386` | Network / Threading | `winhttp_get` never calls `WinHttpSetTimeouts`; stalled LRCLIB or `.ics` servers can block `spawn_blocking` workers for `21s–60s+`. **Fix:** Configure bounded connect/send/receive timeouts via `WinHttpSetTimeouts(session, 3000, 4000, 4000, 6000)`. |
| S2-04 | MEDIUM | `app/src-tauri/src/clipboard.rs:193-197,262-265` | Edge Input / Panic | `read_clipboard_image_data_url` calls `h_raw.abs()` on untrusted `BITMAPINFOHEADER.biHeight` (`i32::MIN` panics in debug / overflows in release), and `read_clipboard_unicode_text` silently truncates at `65536` UTF-16 units. **Fix:** Use `h_raw.unsigned_abs()` for bounds validation and document/enforce explicit text limit. |
| S2-05 | MEDIUM | `app/src-tauri/src/dock_win.rs:386-423` | Win32 / Threading | `toggle_window_hwnd` calls `AttachThreadInput` against potentially hung foreground windows from a worker thread; `close_window_hwnd` skips `IsWindow` validation and ignores `PostMessageW` errors. **Fix:** Skip `AttachThreadInput` if `IsHungAppWindow(fg)` is true; add `IsWindow` check and return `Result` in `close_window_hwnd`. |
| S2-06 | MEDIUM | `app/src-tauri/src/dock_win.rs:628-662` | Performance / I/O | `list_downloads` stats every file in Downloads (`e.metadata()`) before `.truncate(24)` and skips folders/access errors silently. **Fix:** Cap directory iteration or collect names/mtimes cleanly without unbounded stat overhead. |
| S2-07 | MEDIUM | `app/src-tauri/src/system_panels.rs:97-229,625-733` | WinRT / Edge Input | WinRT media/notification helpers rely on implicit thread apartment init instead of explicit MTA guard, and `seek_media_position` converts raw `position_sec` without `is_finite()` / `clamp_seek_seconds` validation. **Fix:** Ensure MTA COM/WinRT init on worker entry and sanitize `position_sec` before converting to 100ns ticks. |
| S2-08 | MEDIUM | `app/src-tauri/src/win_hook.rs:8-29,77-94` | Race Condition | Rapidly toggling `set_win_key_hook_enabled(true)` then `false` before `HOOK_THREAD_ID.store(tid)` (`79`) skips `PostThreadMessageW(WM_QUIT)`, leaving `WH_KEYBOARD_LL` installed until next toggle. **Fix:** Re-check `HOOK_STARTED.load(Ordering::SeqCst)` after `HOOK_THREAD_ID.store(tid)` (or synchronize startup under a `Mutex`). |
| S2-09 | LOW | `app/src-tauri/src/spotlight.rs:137-309` | Edge Input / Stack | `eval_math_expr` allows unbounded parenthesis recursion (`((((...))))`), risking stack overflow on huge adversarial inputs. **Fix:** Reject `trimmed.len() > 256` or enforce a max recursion depth (`<= 32`). |

## Stage 3 Findings

| ID | Severity | File:Line | Category | Issue & Fix |
| :--- | :--- | :--- | :--- | :--- |
| S3-01 | MEDIUM | `app/src-tauri/src/native_ui/desktop_clock.rs:33,73-89,142,197-199` | Native UI / Positioning | `sync_clock_widget` uses `.lock().unwrap()` (`33`); `WM_DESTROY` (`142`) does not clear `CLOCK_HWND` or `DRAW_CACHE`; `work_area_x` (`73-89`) uses primary `SPI_GETWORKAREA` only without `WM_DISPLAYCHANGE` handling. **Fix:** Replace `.unwrap()` with `unwrap_or_else(|e| e.into_inner())`, reset `CLOCK_HWND`/`DRAW_CACHE` on `WM_DESTROY`, and handle `WM_DISPLAYCHANGE`. |
| S3-02 | MEDIUM | `app/src/windows/DockWindow.tsx:244-274,338,374,783,832` | UI Error Handling | `openSubPopover` awaits IPC without `try/catch`, and `close_window`/`launch_app`/`toggle_window`/`open_settings` lack `.catch(reportError)`. **Fix:** Wrap `openSubPopover` in `try/catch` calling `reportError` and attach `.catch(reportError)` to all click invocations. |
| S3-03 | MEDIUM | `app/src/windows/SpotlightWindow.tsx:18-44,74-88` | Focus / UX | Clicking outside Spotlight leaves the floating window visible (`tauri://blur` / `window.onblur` auto-hide missing). **Fix:** Listen for `blur` on the Spotlight window when visible and invoke `hide_spotlight`. |
| S3-04 | LOW | `app/src-tauri/src/native_ui/desktop_widgets.rs:20-46,63-112` | Parser / Bounds | `parse_ics_events` truncates to 20 events without sorting chronologically by `dtstart`, and `snap_widget_position` can snap negative/off-screen without screen-bounds clamping. **Fix:** Sort parsed events by `dtstart` before `.truncate(20)` and clamp snapped coordinates to work-area bounds. |
| S3-05 | LOW | `app/src/windows/NotchWindow.tsx:183-190,236` | UI State | `invoke("open_settings")` (`236`) omits `.catch(() => {})` and click-expanded Notch relies only on `onMouseLeave` to collapse when unpinned. **Fix:** Add `.catch` handler and collapse on `window` `blur` when unpinned. |

## Prioritized Fix Batches

1. **Batch 1 — Critical / Hangs / Resource & Crash Safety (`S1-01`, `S2-01`, `S2-02`, `S2-03`, `S2-08`):**
   - Add panic hook + `SetUnhandledExceptionFilter` + hidden-taskbar recovery marker (`lib.rs`, `dock_win.rs`).
   - Wrap clipboard `HGLOBAL` in a RAII drop guard (`clipboard.rs`), guard `winhttp_get` with `WinHttpSetTimeouts` (`system_panels.rs`), harden `IPolicyConfig` vtable guard (`audio_mixer.rs`), and close the `win_hook` startup/teardown race (`win_hook.rs`).
2. **Batch 2 — Positioning, Config Persistence & UX Polish (`S1-02`, `S1-03`, `S1-04`, `S1-05`, `S1-06`, `S2-05`, `S2-07`, `S3-01`..`S3-05`):**
   - Atomic `config.json` writes + corrupt backup + `#[serde(default)]` on nested items (`config.rs`); convert `update_config` and `search_spotlight` to `async fn` (`lib.rs`); add `IsHungAppWindow` / `IsWindow` guards (`dock_win.rs`); fix `desktop_clock.rs` cleanup + `desktop_widgets.rs` `.ics` sort/snap clamping; add Spotlight/Notch blur auto-hide and Dock `try/catch` error reporting.
3. **Batch 3 — Edge-Case & Regression Tests (`S2-04`, `S2-06`, `S2-09`):**
   - Fix `h_raw.unsigned_abs()` (`clipboard.rs`) + `eval_math_expr` recursion cap (`spotlight.rs`) + `seek_media_position` `is_finite()` clamp (`system_panels.rs`); add unit tests for `i32::MIN` DIB height, deep nested parens (`((...))`), corrupted config backup, `.ics` sorting, and `seek_media_position` bounds.

## Stage 2 Findings

| ID | Severity | File:Line | Category | Issue & Fix |
| :--- | :--- | :--- | :--- | :--- |
| S2-01 | HIGH | `app/src-tauri/src/clipboard.rs:213-217,353-397` | Memory / Resource Leak | `GlobalAlloc(GMEM_MOVEABLE, ...)` leaks `hmem` (up to `67 MB` for PNG) on `GlobalLock` failure, `_ => Err` (`393`), or `SetClipboardData` error (`217`, `396`) because Windows does not own `hmem` until `SetClipboardData` succeeds. **Fix:** Wrap `HGLOBAL` in a RAII guard calling `GlobalFree` on `Drop` unless disarmed after `SetClipboardData` succeeds. |
| S2-02 | HIGH | `app/src-tauri/src/audio_mixer.rs:339-388` | COM / ABI Safety | `set_default_output_device` relies on hardcoded undocumented `IPolicyConfig` vtable slot indices (`_slots: [usize; 10]`, slot 13); a Windows COM vtable layout change causes UB/crash. **Fix:** Document/isolate the undocumented ABI surface behind an explicit version/QI guard and validate endpoint existence first. |
| S2-03 | HIGH | `app/src-tauri/src/system_panels.rs:285-386` | Network / Threading | `winhttp_get` never calls `WinHttpSetTimeouts`; stalled LRCLIB or `.ics` servers can block `spawn_blocking` workers for `21s–60s+`. **Fix:** Configure bounded connect/send/receive timeouts via `WinHttpSetTimeouts(session, 3000, 4000, 4000, 6000)`. |
| S2-04 | MEDIUM | `app/src-tauri/src/clipboard.rs:193-197,262-265` | Edge Input / Panic | `read_clipboard_image_data_url` calls `h_raw.abs()` on untrusted `BITMAPINFOHEADER.biHeight` (`i32::MIN` panics in debug / overflows in release), and `read_clipboard_unicode_text` silently truncates at `65536` UTF-16 units. **Fix:** Use `h_raw.unsigned_abs()` for bounds validation and document/enforce explicit text limit. |
| S2-05 | MEDIUM | `app/src-tauri/src/dock_win.rs:386-423` | Win32 / Threading | `toggle_window_hwnd` calls `AttachThreadInput` against potentially hung foreground windows from a worker thread; `close_window_hwnd` skips `IsWindow` validation and ignores `PostMessageW` errors. **Fix:** Skip `AttachThreadInput` if `IsHungAppWindow(fg)` is true; add `IsWindow` check and return `Result` in `close_window_hwnd`. |
| S2-06 | MEDIUM | `app/src-tauri/src/dock_win.rs:628-662` | Performance / I/O | `list_downloads` stats every file in Downloads (`e.metadata()`) before `.truncate(24)` and skips folders/access errors silently. **Fix:** Cap directory iteration or collect names/mtimes cleanly without unbounded stat overhead. |
| S2-07 | MEDIUM | `app/src-tauri/src/system_panels.rs:97-229,625-733` | WinRT / Edge Input | WinRT media/notification helpers rely on implicit thread apartment init instead of explicit MTA guard, and `seek_media_position` converts raw `position_sec` without `is_finite()` / `clamp_seek_seconds` validation. **Fix:** Ensure MTA COM/WinRT init on worker entry and sanitize `position_sec` before converting to 100ns ticks. |
| S2-08 | MEDIUM | `app/src-tauri/src/win_hook.rs:8-29,77-94` | Race Condition | Rapidly toggling `set_win_key_hook_enabled(true)` then `false` before `HOOK_THREAD_ID.store(tid)` (`79`) skips `PostThreadMessageW(WM_QUIT)`, leaving `WH_KEYBOARD_LL` installed until next toggle. **Fix:** Re-check `HOOK_STARTED.load(Ordering::SeqCst)` after `HOOK_THREAD_ID.store(tid)` (or synchronize startup under a `Mutex`). |
| S2-09 | LOW | `app/src-tauri/src/spotlight.rs:137-309` | Edge Input / Stack | `eval_math_expr` allows unbounded parenthesis recursion (`((((...))))`), risking stack overflow on huge adversarial inputs. **Fix:** Reject `trimmed.len() > 256` or enforce a max recursion depth (`<= 32`). |

## Stage 3 Findings

| ID | Severity | File:Line | Category | Issue & Fix |
| :--- | :--- | :--- | :--- | :--- |
| S3-01 | MEDIUM | `app/src-tauri/src/native_ui/desktop_clock.rs:33,73-89,142,197-199` | Native UI / Positioning | `sync_clock_widget` uses `.lock().unwrap()` (`33`); `WM_DESTROY` (`142`) does not clear `CLOCK_HWND` or `DRAW_CACHE`; `work_area_x` (`73-89`) uses primary `SPI_GETWORKAREA` only without `WM_DISPLAYCHANGE` handling. **Fix:** Replace `.unwrap()` with `unwrap_or_else(|e| e.into_inner())`, reset `CLOCK_HWND`/`DRAW_CACHE` on `WM_DESTROY`, and handle `WM_DISPLAYCHANGE`. |
| S3-02 | MEDIUM | `app/src/windows/DockWindow.tsx:244-274,338,374,783,832` | UI Error Handling | `openSubPopover` awaits IPC without `try/catch`, and `close_window`/`launch_app`/`toggle_window`/`open_settings` lack `.catch(reportError)`. **Fix:** Wrap `openSubPopover` in `try/catch` calling `reportError` and attach `.catch(reportError)` to all click invocations. |
| S3-03 | MEDIUM | `app/src/windows/SpotlightWindow.tsx:18-44,74-88` | Focus / UX | Clicking outside Spotlight leaves the floating window visible (`tauri://blur` / `window.onblur` auto-hide missing). **Fix:** Listen for `blur` on the Spotlight window when visible and invoke `hide_spotlight`. |
| S3-04 | LOW | `app/src-tauri/src/native_ui/desktop_widgets.rs:20-46,63-112` | Parser / Bounds | `parse_ics_events` truncates to 20 events without sorting chronologically by `dtstart`, and `snap_widget_position` can snap negative/off-screen without screen-bounds clamping. **Fix:** Sort parsed events by `dtstart` before `.truncate(20)` and clamp snapped coordinates to work-area bounds. |
| S3-05 | LOW | `app/src/windows/NotchWindow.tsx:183-190,236` | UI State | `invoke("open_settings")` (`236`) omits `.catch(() => {})` and click-expanded Notch relies only on `onMouseLeave` to collapse when unpinned. **Fix:** Add `.catch` handler and collapse on `window` `blur` when unpinned. |

## Prioritized Fix Batches

1. **Batch 1 — Critical / Hangs / Resource & Crash Safety (`S1-01`, `S2-01`, `S2-02`, `S2-03`, `S2-08`):**
   - Add panic hook + `SetUnhandledExceptionFilter` + hidden-taskbar recovery marker (`lib.rs`, `dock_win.rs`).
   - Wrap clipboard `HGLOBAL` in a RAII drop guard (`clipboard.rs`), guard `winhttp_get` with `WinHttpSetTimeouts` (`system_panels.rs`), harden `IPolicyConfig` vtable guard (`audio_mixer.rs`), and close the `win_hook` startup/teardown race (`win_hook.rs`).
2. **Batch 2 — Positioning, Config Persistence & UX Polish (`S1-02`, `S1-03`, `S1-04`, `S1-05`, `S1-06`, `S2-05`, `S2-07`, `S3-01`..`S3-05`):**
   - Atomic `config.json` writes + corrupt backup + `#[serde(default)]` on nested items (`config.rs`); convert `update_config` and `search_spotlight` to `async fn` (`lib.rs`); add `IsHungAppWindow` / `IsWindow` guards (`dock_win.rs`); fix `desktop_clock.rs` cleanup + `desktop_widgets.rs` `.ics` sort/snap clamping; add Spotlight/Notch blur auto-hide and Dock `try/catch` error reporting.
3. **Batch 3 — Edge-Case & Regression Tests (`S2-04`, `S2-06`, `S2-09`):**
   - Fix `h_raw.unsigned_abs()` (`clipboard.rs`) + `eval_math_expr` recursion cap (`spotlight.rs`) + `seek_media_position` `is_finite()` clamp (`system_panels.rs`); add unit tests for `i32::MIN` DIB height, deep nested parens (`((...))`), corrupted config backup, `.ics` sorting, and `seek_media_position` bounds.
