pub mod audio_mixer;
pub mod clipboard;
pub mod config;
pub mod dock_win;
pub mod native_ui;
pub mod spotlight;
pub mod system_panels;
pub mod win_hook;

use config::{load_config, save_config, AppConfig};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder,
};

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub clipboard_history: Mutex<Vec<clipboard::ClipboardItem>>,
    pub spotlight_cache: Mutex<Vec<spotlight::SpotlightEntry>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpotlightSearchResult {
    math_answer: Option<String>,
    entries: Vec<spotlight::SpotlightEntry>,
}

fn lock_recover<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[tauri::command]
fn get_config(state: State<AppState>) -> AppConfig {
    lock_recover(&state.config).clone()
}

#[tauri::command]
async fn update_config(app: AppHandle, state: State<'_, AppState>, mut new_config: AppConfig) -> Result<AppConfig, String> {
    config::sanitize_config(&mut new_config);
    {
        let mut guard = lock_recover(&state.config);
        *guard = new_config.clone();
    }
    let cfg_save = new_config.clone();
    tauri::async_runtime::spawn_blocking(move || save_config(&cfg_save))
        .await
        .map_err(|e| e.to_string())??;
    let _ = app.emit("antibar://config-changed", &new_config);
    sync_notch_window(&app, new_config.notch_enabled);
    win_hook::set_win_key_hook_enabled(&app, new_config.use_win_key_for_launchpad);
    dock_win::set_windows_taskbar_hidden(new_config.hide_windows_taskbar);
    apply_dock_geometry(&app, None, None, new_config.hide_windows_taskbar);
    native_ui::sync_native_widgets(&new_config);
    Ok(new_config)
}

#[tauri::command]
async fn fetch_ics_agenda(url: String) -> Vec<native_ui::desktop_widgets::IcsEvent> {
    let Some((host, pq)) = native_ui::desktop_widgets::parse_https_url(&url) else {
        return Vec::new();
    };
    tauri::async_runtime::spawn_blocking(move || {
        match system_panels::winhttp_get(&host, &pq, "text/calendar,text/plain") {
            Ok(raw) => native_ui::desktop_widgets::parse_ics_events(&raw),
            Err(_) => Vec::new(),
        }
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
async fn get_audio_sessions() -> Vec<audio_mixer::AudioSessionInfo> {
    tauri::async_runtime::spawn_blocking(audio_mixer::list_audio_sessions)
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn set_app_volume(pid: u32, is_master: bool, volume: f32, muted: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        audio_mixer::set_session_volume(pid, is_master, volume, muted)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_audio_outputs() -> Vec<audio_mixer::AudioOutputDevice> {
    tauri::async_runtime::spawn_blocking(audio_mixer::list_output_devices)
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn set_audio_output(device_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || audio_mixer::set_default_output_device(&device_id))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_notifications(request_access: Option<bool>) -> system_panels::NotificationsSnapshot {
    let req = request_access.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || system_panels::list_notifications(req))
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn dismiss_notification(id: u32) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || system_panels::dismiss_notification(id))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn clear_notifications() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(system_panels::clear_all_notifications)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_wifi_networks() -> Vec<system_panels::WifiNetwork> {
    tauri::async_runtime::spawn_blocking(system_panels::list_wifi_networks)
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn get_system_status() -> system_panels::SystemStatus {
    tauri::async_runtime::spawn_blocking(system_panels::get_system_status)
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn run_power_action(action: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || system_panels::execute_power_action(&action))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_running_windows() -> Vec<dock_win::RunningWindow> {
    tauri::async_runtime::spawn_blocking(dock_win::enumerate_running_windows)
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn toggle_window(hwnd: isize) {
    let _ = tauri::async_runtime::spawn_blocking(move || dock_win::toggle_window_hwnd(hwnd)).await;
}

#[tauri::command]
async fn close_window(hwnd: isize) {
    let _ = tauri::async_runtime::spawn_blocking(move || dock_win::close_window_hwnd(hwnd)).await;
}

#[tauri::command]
async fn get_window_preview(hwnd: isize) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || dock_win::capture_window_preview(hwnd))
        .await
        .ok()
        .flatten()
}

#[tauri::command]
async fn launch_app(path: String, args: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || dock_win::launch_path(&path, args.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_downloads() -> Vec<dock_win::DownloadEntry> {
    tauri::async_runtime::spawn_blocking(dock_win::list_downloads)
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn get_trash_info() -> dock_win::TrashInfo {
    tauri::async_runtime::spawn_blocking(dock_win::query_recycle_bin)
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn empty_trash() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(dock_win::empty_recycle_bin)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn open_trash() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| dock_win::launch_path("shell:RecycleBinFolder", None))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn open_hidden_tray() {
    let _ = tauri::async_runtime::spawn_blocking(dock_win::toggle_windows_hidden_tray).await;
}

const BROWSER_ARGS: &str = "--renderer-process-limit=1 --in-process-gpu --disable-crash-reporter --disk-cache-size=1 --media-cache-size=1 --js-flags=--max-old-space-size=48 --disable-gpu-shader-disk-cache --disable-features=BackForwardCache,Translate,msWebOOUI,msPdfOOUI,AutofillServerCommunication,MediaRouter";

#[cfg(windows)]
fn apply_webview_memory_policy(win: &tauri::WebviewWindow, hidden: bool) {
    let _ = win.with_webview(move |wv| unsafe {
        use std::ffi::c_void;
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL,
        };
        use windows::core::{IUnknown, Interface, GUID};

        const IID_ICOREWEBVIEW2_19: GUID =
            GUID::from_u128(0x6921f954_79b0_437f_a997_c85811897c68);

        let ctrl = wv.controller();
        let _ = ctrl.SetIsVisible(!hidden);
        if let Ok(core) = ctrl.CoreWebView2() {
            let core_unk: &IUnknown = std::mem::transmute(&core);
            let mut raw_19: *mut c_void = std::ptr::null_mut();
            if core_unk.query(&IID_ICOREWEBVIEW2_19, &mut raw_19).is_ok() && !raw_19.is_null() {
                let wv19: ICoreWebView2_19 = std::mem::transmute(raw_19);
                let level = if hidden { 1 } else { 0 };
                let _ = wv19.SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL(level));
            }
        }
    });
}

#[cfg(not(windows))]
fn apply_webview_memory_policy(_win: &tauri::WebviewWindow, _hidden: bool) {}

#[tauri::command]
async fn search_spotlight(state: State<'_, AppState>, query: String) -> Result<SpotlightSearchResult, String> {
    let math_answer = spotlight::eval_math_expr(&query).map(|v| {
        if (v.fract()).abs() < 1e-9 {
            format!("{}", v as i64)
        } else {
            format!("{:.6}", v)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        }
    });

    let q = query.trim().to_ascii_lowercase();
    let needs_index = lock_recover(&state.spotlight_cache).is_empty();
    if needs_index {
        let indexed = tauri::async_runtime::spawn_blocking(spotlight::index_installed_apps)
            .await
            .unwrap_or_default();
        let mut cache = lock_recover(&state.spotlight_cache);
        if cache.is_empty() {
            *cache = indexed;
        }
    }

    let mut entries = {
        let cache = lock_recover(&state.spotlight_cache);
        spotlight::filter_spotlight_entries(&cache, &q, 12)
    };

    entries = tauri::async_runtime::spawn_blocking(move || {
        for e in &mut entries {
            if e.icon_data_url.is_none() {
                e.icon_data_url = dock_win::extract_path_icon_data_url(&e.path);
            }
        }
        entries
    })
    .await
    .unwrap_or_default();

    Ok(SpotlightSearchResult {
        math_answer,
        entries,
    })
}

#[tauri::command]
fn get_clipboard_history(state: State<AppState>) -> Vec<clipboard::ClipboardItem> {
    lock_recover(&state.clipboard_history).clone()
}

#[tauri::command]
fn copy_clipboard_item(text: String, image_data_url: Option<String>) -> Result<(), String> {
    if let Some(img) = image_data_url {
        if !img.is_empty() {
            return clipboard::write_clipboard_png_data_url(&img);
        }
    }
    clipboard::write_clipboard_unicode_text(&text)
}

#[tauri::command]
fn toggle_pin_clipboard_item(state: State<AppState>, id: String) -> Vec<clipboard::ClipboardItem> {
    let mut guard = lock_recover(&state.clipboard_history);
    if let Some(item) = guard.iter_mut().find(|i| i.id == id) {
        item.pinned = !item.pinned;
    }
    clipboard::save_history(&guard);
    guard.clone()
}

#[tauri::command]
fn clear_clipboard_history(state: State<AppState>) -> Vec<clipboard::ClipboardItem> {
    let mut guard = lock_recover(&state.clipboard_history);
    guard.retain(|i| i.pinned);
    clipboard::save_history(&guard);
    guard.clone()
}

#[tauri::command]
async fn get_media_state() -> system_panels::MediaState {
    tauri::async_runtime::spawn_blocking(system_panels::read_current_media)
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn control_media(action: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || system_panels::media_control(&action))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn seek_media(position_sec: f64) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || system_panels::seek_media_position(position_sec))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn fetch_lyrics(
    title: String,
    artist: String,
    album: String,
    duration_sec: f64,
) -> system_panels::LyricsResult {
    tauri::async_runtime::spawn_blocking(move || {
        system_panels::fetch_synced_lyrics(&title, &artist, &album, duration_sec)
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
async fn open_settings(app: AppHandle) {
    let _ = tauri::async_runtime::spawn_blocking(move || {
        if let Some(win) = app.get_webview_window("settings") {
            apply_webview_memory_policy(&win, false);
            let _ = win.show();
            let _ = win.set_focus();
            return;
        }
        if let Ok(win) = WebviewWindowBuilder::new(&app, "settings", WebviewUrl::App("index.html?view=settings".into()))
            .title("Anti-Bar Settings")
            .inner_size(940.0, 660.0)
            .resizable(true)
            .additional_browser_args(BROWSER_ARGS)
            .center()
            .build()
        {
            apply_webview_memory_policy(&win, false);
            let win_clone = win.clone();
            win.on_window_event(move |event| match event {
                tauri::WindowEvent::Resized(_) => {
                    let minimized = win_clone.is_minimized().unwrap_or(false);
                    apply_webview_memory_policy(&win_clone, minimized);
                    if minimized {
                        dock_win::trim_process_tree_memory();
                    }
                }
                tauri::WindowEvent::Destroyed => {
                    std::thread::spawn(|| {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        dock_win::trim_process_tree_memory();
                    });
                }
                _ => {}
            });
        }
    })
    .await;
}

pub fn spawn_toggle_spotlight(app: AppHandle) {
    tauri::async_runtime::spawn(toggle_spotlight(app));
}

#[tauri::command]
async fn toggle_spotlight(app: AppHandle) {
    let _ = tauri::async_runtime::spawn_blocking(move || {
        if let Some(win) = app.get_webview_window("spotlight") {
            if win.is_visible().unwrap_or(false) {
                hide_spotlight(app);
            } else {
                apply_webview_memory_policy(&win, false);
                let _ = win.center();
                let _ = win.show();
                let _ = win.set_focus();
                let _ = app.emit("antibar://spotlight-opened", ());
            }
            return;
        }
        if let Ok(win) = WebviewWindowBuilder::new(&app, "spotlight", WebviewUrl::App("index.html?view=spotlight".into()))
            .title("Anti-Bar Spotlight")
            .inner_size(640.0, 440.0)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .additional_browser_args(BROWSER_ARGS)
            .center()
            .build()
        {
            apply_webview_memory_policy(&win, false);
            let _ = win.set_focus();
        }
    })
    .await;
}

#[tauri::command]
fn hide_spotlight(app: AppHandle) {
    if let Some(win) = app.get_webview_window("spotlight") {
        let _ = win.hide();
        apply_webview_memory_policy(&win, true);
    }
    {
        let state = app.state::<AppState>();
        let mut cache = lock_recover(&state.spotlight_cache);
        cache.clear();
        cache.shrink_to_fit();
    }
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(120));
        dock_win::trim_process_tree_memory();
    });
}

#[tauri::command]
fn resize_overlay(app: AppHandle, state: State<AppState>, label: String, width: f64, height: f64) {
    if label == "dock" {
        let taskbar_hidden = lock_recover(&state.config).hide_windows_taskbar;
        apply_dock_geometry(&app, Some(width), Some(height), taskbar_hidden);
        return;
    }
    let app_for_main = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(win) = app_for_main.get_webview_window(&label) {
            if label == "notch" {
                let cw = width.max(120.0);
                let ch = height.max(32.0);
                let _ = win.set_size(tauri::LogicalSize::new(cw, ch));
                position_top_center(&win, cw);
            } else {
                let _ = win.set_size(tauri::LogicalSize::new(width.max(120.0), height.max(40.0)));
            }
        }
    });
}

/// Computes Dock `(x, y, width, height)` using full monitor bounds when `hide_windows_taskbar` is true,
/// or `work_area` bounds when false. `show_dock_clock` affects width only and never alters Y.
#[allow(clippy::too_many_arguments)]
pub fn compute_dock_bounds(
    mon_rect: (f64, f64, f64, f64),
    wa_rect: (f64, f64, f64, f64),
    width: f64,
    height: f64,
    hide_windows_taskbar: bool,
    _show_dock_clock: bool,
) -> (f64, f64, f64, f64) {
    let (left, top, area_w, area_h) = if hide_windows_taskbar { mon_rect } else { wa_rect };
    compute_bottom_center_rect(left, top, area_w, area_h, width, height)
}

fn apply_dock_geometry(
    app: &AppHandle,
    target_w: Option<f64>,
    target_h: Option<f64>,
    taskbar_hidden: bool,
) {
    let app_for_main = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(dock) = app_for_main.get_webview_window("dock") {
            let (w, h) = match (target_w, target_h) {
                (Some(w), Some(h)) => (w, h),
                _ => match (dock.inner_size(), dock.scale_factor()) {
                    (Ok(sz), Ok(scale)) if scale > 0.0 => (sz.width as f64 / scale, sz.height as f64 / scale),
                    _ => (860.0, 108.0),
                },
            };
            if let Ok(Some(mon)) = dock.primary_monitor() {
                let scale = mon.scale_factor();
                let p = mon.position();
                let s = mon.size();
                let wa = mon.work_area();
                let mon_rect = (
                    p.x as f64 / scale,
                    p.y as f64 / scale,
                    s.width as f64 / scale,
                    s.height as f64 / scale,
                );
                let wa_rect = (
                    wa.position.x as f64 / scale,
                    wa.position.y as f64 / scale,
                    wa.size.width as f64 / scale,
                    wa.size.height as f64 / scale,
                );
                let (x, y, cw, ch) = compute_dock_bounds(mon_rect, wa_rect, w, h, taskbar_hidden, false);
                if target_w.is_some() && target_h.is_some() {
                    let _ = dock.set_size(tauri::LogicalSize::new(cw, ch));
                }
                let _ = dock.set_position(tauri::LogicalPosition::new(x, y));
            }
        }
    });
}

fn sync_notch_window(app: &AppHandle, enabled: bool) {
    if enabled {
        if app.get_webview_window("notch").is_none() {
            if let Ok(win) = WebviewWindowBuilder::new(app, "notch", WebviewUrl::App("index.html?view=notch".into()))
                .title("Anti-Bar Notch")
                .inner_size(256.0, 46.0)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .shadow(false)
                .focused(false)
                .additional_browser_args(BROWSER_ARGS)
                .build()
            {
                position_top_center(&win, 256.0);
                apply_webview_memory_policy(&win, false);
            }
        }
    } else if let Some(win) = app.get_webview_window("notch") {
        let _ = win.hide();
        let _ = win.destroy();
        dock_win::trim_process_tree_memory();
    }
}

pub fn compute_top_center_xy(mon_left: f64, mon_top: f64, mon_w: f64, width: f64) -> (f64, f64, f64) {
    let max_w = (mon_w - 8.0).max(180.0);
    let cw = width.clamp(180.0, max_w);
    let x = mon_left + ((mon_w - cw) / 2.0).max(0.0);
    (x, mon_top + 6.0, cw)
}

pub fn compute_bottom_center_rect(
    area_left: f64,
    area_top: f64,
    area_w: f64,
    area_h: f64,
    width: f64,
    height: f64,
) -> (f64, f64, f64, f64) {
    let max_w = (area_w - 8.0).max(200.0);
    let max_h = (area_h - 8.0).max(60.0);
    let cw = width.clamp(200.0, max_w);
    let ch = height.clamp(60.0, max_h);
    let x = area_left + ((area_w - cw) / 2.0).max(0.0);
    let y = (area_top + area_h - ch - 2.0).max(area_top);
    (x, y, cw, ch)
}

fn position_top_center(win: &tauri::WebviewWindow, width: f64) {
    if let Ok(Some(mon)) = win.primary_monitor() {
        let scale = mon.scale_factor();
        let pos = mon.position();
        let mon_left = pos.x as f64 / scale;
        let mon_top = pos.y as f64 / scale;
        let mon_w = mon.size().width as f64 / scale;
        let (x, y, _) = compute_top_center_xy(mon_left, mon_top, mon_w, width);
        let _ = win.set_position(tauri::LogicalPosition::new(x, y));
    }
}

#[cfg(windows)]
fn start_hotkey_listener(app: AppHandle) {
    std::thread::spawn(move || {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
        };
        use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};

        const HK_ALT_SPACE: i32 = 1;
        const HK_CTRL_ALT_SPACE: i32 = 2;
        const VK_SPACE: u32 = 0x20;

        unsafe {
            let _ = RegisterHotKey(None, HK_ALT_SPACE, MOD_ALT | MOD_NOREPEAT, VK_SPACE);
            let _ = RegisterHotKey(
                None,
                HK_CTRL_ALT_SPACE,
                MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
                VK_SPACE,
            );
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                if msg.message == WM_HOTKEY {
                    spawn_toggle_spotlight(app.clone());
                }
            }
            let _ = UnregisterHotKey(None, HK_ALT_SPACE);
            let _ = UnregisterHotKey(None, HK_CTRL_ALT_SPACE);
        }
    });
}

fn start_background_pollers(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_clip_seq = clipboard::clipboard_sequence_number();
        let mut last_clip = String::new();
        let mut last_img = String::new();
        let mut last_fs = false;
        let mut last_media_sig = String::new();
        let mut last_track_key = String::new();
        let mut tick_count: u64 = 0;
        loop {
            let sleep_ms = if last_fs { 2500 } else { 1500 };
            std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
            tick_count += 1;

            let seq = clipboard::clipboard_sequence_number();
            if seq != last_clip_seq || seq == 0 {
                last_clip_seq = seq;
                if let Some(txt) = clipboard::read_clipboard_unicode_text() {
                    if txt != last_clip {
                        last_clip = txt.clone();
                        let state = app.state::<AppState>();
                        let updated = {
                            let mut guard = lock_recover(&state.clipboard_history);
                            if clipboard::push_clipboard_text(&mut guard, txt) {
                                Some(guard.clone())
                            } else {
                                None
                            }
                        };
                        if let Some(list) = updated {
                            let _ = app.emit("antibar://clipboard-updated", &list);
                        }
                    }
                } else if let Some(img) = clipboard::read_clipboard_image_data_url() {
                    if img != last_img {
                        last_img = img.clone();
                        let state = app.state::<AppState>();
                        let updated = {
                            let mut guard = lock_recover(&state.clipboard_history);
                            if clipboard::push_clipboard_image(&mut guard, img) {
                                Some(guard.clone())
                            } else {
                                None
                            }
                        };
                        if let Some(list) = updated {
                            let _ = app.emit("antibar://clipboard-updated", &list);
                        }
                    }
                }
            }

            let mut media = system_panels::read_current_media();
            let sig = format!(
                "{}|{}|{}|{}|{}|{}",
                media.has_session,
                media.is_playing,
                media.title,
                media.artist,
                media.position_sec.floor() as i64,
                media.duration_sec.floor() as i64
            );
            if sig != last_media_sig {
                last_media_sig = sig;
                let tkey = format!("{}::{}::{}", media.title, media.artist, media.album);
                if tkey == last_track_key {
                    // Skip re-sending the base64 cover art every timeline tick.
                    media.thumbnail_data_url = None;
                } else {
                    last_track_key = tkey;
                }
                let _ = app.emit("antibar://media-updated", &media);
            }

            let fs_now = dock_win::is_fullscreen_active();
            if fs_now != last_fs {
                last_fs = fs_now;
                let _ = app.emit("antibar://fullscreen-changed", fs_now);
                if let Some(dock) = app.get_webview_window("dock") {
                    apply_webview_memory_policy(&dock, fs_now);
                }
                if fs_now {
                    dock_win::trim_process_tree_memory();
                }
            }

            if tick_count == 3 || tick_count.is_multiple_of(20) {
                dock_win::trim_process_tree_memory();
            }
        }
    });
}

struct TaskbarRestoreGuard;

impl Drop for TaskbarRestoreGuard {
    fn drop(&mut self) {
        dock_win::restore_taskbar_if_hidden();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(windows)]
    let _singleton_guard = unsafe {
        use windows::core::w;
        use windows::Win32::Foundation::GetLastError;
        use windows::Win32::System::Threading::CreateMutexW;
        let m = CreateMutexW(None, true, w!("Local\\AntiBarSingletonMutex"));
        if GetLastError().0 == 183 {
            // ERROR_ALREADY_EXISTS
            return;
        }
        m
    };

    dock_win::restore_taskbar_if_orphaned();

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        dock_win::restore_taskbar_if_hidden();
        prev_hook(info);
    }));

    let _taskbar_restore_guard = TaskbarRestoreGuard;

    let initial_cfg = load_config();
    let initial_clips = clipboard::load_history();
    let show_notch = initial_cfg.notch_enabled;
    let enable_win_hook = initial_cfg.use_win_key_for_launchpad;
    let hide_taskbar_on_start = initial_cfg.hide_windows_taskbar;
    let cfg_for_widgets = initial_cfg.clone();

    let app_state = AppState {
        config: Mutex::new(initial_cfg),
        clipboard_history: Mutex::new(initial_clips),
        spotlight_cache: Mutex::new(Vec::new()),
    };

    let _arc_guard = Arc::new(());

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            get_config,
            update_config,
            fetch_ics_agenda,
            get_audio_sessions,
            set_app_volume,
            get_audio_outputs,
            set_audio_output,
            get_notifications,
            dismiss_notification,
            clear_notifications,
            get_wifi_networks,
            get_system_status,
            run_power_action,
            get_running_windows,
            toggle_window,
            close_window,
            get_window_preview,
            launch_app,
            get_downloads,
            get_trash_info,
            empty_trash,
            open_trash,
            open_hidden_tray,
            search_spotlight,
            get_clipboard_history,
            copy_clipboard_item,
            toggle_pin_clipboard_item,
            clear_clipboard_history,
            get_media_state,
            control_media,
            seek_media,
            fetch_lyrics,
            open_settings,
            toggle_spotlight,
            hide_spotlight,
            resize_overlay,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            dock_win::set_windows_taskbar_hidden(hide_taskbar_on_start);
            if let Some(dock) = app.get_webview_window("dock") {
                apply_webview_memory_policy(&dock, false);
            }
            apply_dock_geometry(&handle, Some(860.0), Some(108.0), hide_taskbar_on_start);
            sync_notch_window(&handle, show_notch);
            win_hook::set_win_key_hook_enabled(&handle, enable_win_hook);
            native_ui::sync_native_widgets(&cfg_for_widgets);

            let toggle_dock_i = MenuItem::with_id(app, "toggle_dock", "Show / Hide Dock", true, None::<&str>)?;
            let toggle_taskbar_i = MenuItem::with_id(
                app,
                "toggle_taskbar",
                "Hide / Show Windows Taskbar",
                true,
                None::<&str>,
            )?;
            let spotlight_i = MenuItem::with_id(app, "spotlight", "Spotlight (Alt+Space)", true, None::<&str>)?;
            let settings_i = MenuItem::with_id(app, "settings", "Anti-Bar Settings", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit Anti-Bar", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &toggle_dock_i,
                    &toggle_taskbar_i,
                    &spotlight_i,
                    &settings_i,
                    &quit_i,
                ],
            )?;

            let tray_icon = app
                .default_window_icon()
                .cloned()
                .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))?;
            let _tray = TrayIconBuilder::new()
                .icon(tray_icon)
                .menu(&menu)
                .tooltip("Anti-Bar")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "toggle_dock" => {
                        if let Some(w) = app.get_webview_window("dock") {
                            if w.is_visible().unwrap_or(true) {
                                let _ = w.hide();
                                apply_webview_memory_policy(&w, true);
                                dock_win::trim_process_tree_memory();
                            } else {
                                apply_webview_memory_policy(&w, false);
                                let _ = w.show();
                            }
                        }
                    }
                    "toggle_taskbar" => {
                        let state = app.state::<AppState>();
                        let updated = {
                            let mut guard = lock_recover(&state.config);
                            guard.hide_windows_taskbar = !guard.hide_windows_taskbar;
                            guard.clone()
                        };
                        let _ = save_config(&updated);
                        let _ = app.emit("antibar://config-changed", &updated);
                        dock_win::set_windows_taskbar_hidden(updated.hide_windows_taskbar);
                        apply_dock_geometry(app, None, None, updated.hide_windows_taskbar);
                    }
                    "spotlight" => spawn_toggle_spotlight(app.clone()),
                    "settings" => {
                        tauri::async_runtime::spawn(open_settings(app.clone()));
                    }
                    "quit" => {
                        dock_win::restore_taskbar_if_hidden();
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            #[cfg(windows)]
            start_hotkey_listener(handle.clone());
            start_background_pollers(handle);

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Anti-Bar")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                dock_win::restore_taskbar_if_hidden();
            }
        });
}
