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

#[tauri::command]
fn get_config(state: State<AppState>) -> AppConfig {
    state.config.lock().map(|g| g.clone()).unwrap_or_default()
}

#[tauri::command]
fn update_config(app: AppHandle, state: State<AppState>, new_config: AppConfig) -> Result<AppConfig, String> {
    {
        let mut guard = state.config.lock().map_err(|e| e.to_string())?;
        *guard = new_config.clone();
    }
    save_config(&new_config)?;
    let _ = app.emit("antibar://config-changed", &new_config);
    sync_notch_window(&app, new_config.notch_enabled);
    win_hook::set_win_key_hook_enabled(&app, new_config.use_win_key_for_launchpad);
    dock_win::set_windows_taskbar_hidden(new_config.hide_windows_taskbar);
    reposition_dock(&app, new_config.hide_windows_taskbar);
    native_ui::sync_native_widgets(&new_config);
    Ok(new_config)
}

#[tauri::command]
fn fetch_ics_agenda(url: String) -> Vec<native_ui::desktop_widgets::IcsEvent> {
    let trimmed = url.trim();
    let Some(rest) = trimmed.strip_prefix("https://") else {
        return Vec::new();
    };
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let pq = format!("/{}", path);
    match system_panels::winhttp_get(host, &pq, "text/calendar,text/plain") {
        Ok(raw) => native_ui::desktop_widgets::parse_ics_events(&raw),
        Err(_) => Vec::new(),
    }
}

#[tauri::command]
fn get_audio_sessions() -> Vec<audio_mixer::AudioSessionInfo> {
    audio_mixer::list_audio_sessions()
}

#[tauri::command]
fn set_app_volume(pid: u32, is_master: bool, volume: f32, muted: bool) -> Result<(), String> {
    audio_mixer::set_session_volume(pid, is_master, volume, muted)
}

#[tauri::command]
fn get_audio_outputs() -> Vec<audio_mixer::AudioOutputDevice> {
    audio_mixer::list_output_devices()
}

#[tauri::command]
fn set_audio_output(device_id: String) -> Result<(), String> {
    audio_mixer::set_default_output_device(&device_id)
}

#[tauri::command]
fn get_notifications(request_access: Option<bool>) -> system_panels::NotificationsSnapshot {
    system_panels::list_notifications(request_access.unwrap_or(false))
}

#[tauri::command]
fn dismiss_notification(id: u32) -> Result<(), String> {
    system_panels::dismiss_notification(id)
}

#[tauri::command]
fn clear_notifications() -> Result<(), String> {
    system_panels::clear_all_notifications()
}

#[tauri::command]
fn get_wifi_networks() -> Vec<system_panels::WifiNetwork> {
    system_panels::list_wifi_networks()
}

#[tauri::command]
fn get_system_status() -> system_panels::SystemStatus {
    system_panels::get_system_status()
}

#[tauri::command]
fn run_power_action(action: String) -> Result<(), String> {
    system_panels::execute_power_action(&action)
}

#[tauri::command]
fn get_running_windows() -> Vec<dock_win::RunningWindow> {
    dock_win::enumerate_running_windows()
}

#[tauri::command]
fn toggle_window(hwnd: isize) {
    dock_win::toggle_window_hwnd(hwnd);
}

#[tauri::command]
fn close_window(hwnd: isize) {
    dock_win::close_window_hwnd(hwnd);
}

#[tauri::command]
fn get_window_preview(hwnd: isize) -> Option<String> {
    dock_win::capture_window_preview(hwnd)
}

#[tauri::command]
fn launch_app(path: String, args: Option<String>) -> Result<(), String> {
    dock_win::launch_path(&path, args.as_deref())
}

#[tauri::command]
fn get_downloads() -> Vec<dock_win::DownloadEntry> {
    dock_win::list_downloads()
}

#[tauri::command]
fn get_trash_info() -> dock_win::TrashInfo {
    dock_win::query_recycle_bin()
}

#[tauri::command]
fn empty_trash() -> Result<(), String> {
    dock_win::empty_recycle_bin()
}

#[tauri::command]
fn open_trash() -> Result<(), String> {
    dock_win::launch_path("shell:RecycleBinFolder", None)
}

#[tauri::command]
fn open_hidden_tray() {
    dock_win::toggle_windows_hidden_tray();
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
                let _ = wv19.SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL(1));
            }
        }
    });
}

#[cfg(not(windows))]
fn apply_webview_memory_policy(_win: &tauri::WebviewWindow, _hidden: bool) {}

#[tauri::command]
fn search_spotlight(state: State<AppState>, query: String) -> SpotlightSearchResult {
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
    let mut entries: Vec<spotlight::SpotlightEntry> = {
        let mut cache = state.spotlight_cache.lock().unwrap();
        if cache.is_empty() {
            *cache = spotlight::index_installed_apps();
        }
        if q.is_empty() {
            cache.iter().take(12).cloned().collect()
        } else {
            cache
                .iter()
                .filter(|e| e.name.to_ascii_lowercase().contains(&q))
                .take(12)
                .cloned()
                .collect()
        }
    };

    for e in &mut entries {
        if e.icon_data_url.is_none() {
            e.icon_data_url = dock_win::extract_path_icon_data_url(&e.path);
        }
    }

    SpotlightSearchResult {
        math_answer,
        entries,
    }
}

#[tauri::command]
fn get_clipboard_history(state: State<AppState>) -> Vec<clipboard::ClipboardItem> {
    state
        .clipboard_history
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
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
    let mut guard = state.clipboard_history.lock().unwrap();
    if let Some(item) = guard.iter_mut().find(|i| i.id == id) {
        item.pinned = !item.pinned;
    }
    clipboard::save_history(&guard);
    guard.clone()
}

#[tauri::command]
fn clear_clipboard_history(state: State<AppState>) -> Vec<clipboard::ClipboardItem> {
    let mut guard = state.clipboard_history.lock().unwrap();
    guard.retain(|i| i.pinned);
    clipboard::save_history(&guard);
    guard.clone()
}

#[tauri::command]
fn get_media_state() -> system_panels::MediaState {
    system_panels::read_current_media()
}

#[tauri::command]
fn control_media(action: String) -> Result<(), String> {
    system_panels::media_control(&action)
}

#[tauri::command]
fn seek_media(position_sec: f64) -> Result<(), String> {
    system_panels::seek_media_position(position_sec)
}

#[tauri::command]
fn fetch_lyrics(title: String, artist: String, album: String, duration_sec: f64) -> system_panels::LyricsResult {
    system_panels::fetch_synced_lyrics(&title, &artist, &album, duration_sec)
}

#[tauri::command]
fn open_settings(app: AppHandle) {
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
}

#[tauri::command]
fn toggle_spotlight(app: AppHandle) {
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
}

#[tauri::command]
fn hide_spotlight(app: AppHandle) {
    if let Some(win) = app.get_webview_window("spotlight") {
        let _ = win.hide();
        apply_webview_memory_policy(&win, true);
    }
    if let Ok(mut cache) = app.state::<AppState>().spotlight_cache.lock() {
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
    if let Some(win) = app.get_webview_window(&label) {
        let _ = win.set_size(tauri::LogicalSize::new(width, height));
        if label == "dock" {
            let taskbar_hidden = state
                .config
                .lock()
                .map(|g| g.hide_windows_taskbar)
                .unwrap_or(false);
            position_bottom_center(&win, width, height, taskbar_hidden);
        } else if label == "notch" {
            position_top_center(&win, width);
        }
    }
}

fn reposition_dock(app: &AppHandle, taskbar_hidden: bool) {
    if let Some(dock) = app.get_webview_window("dock") {
        let (w, h) = match (dock.inner_size(), dock.scale_factor()) {
            (Ok(sz), Ok(scale)) if scale > 0.0 => (sz.width as f64 / scale, sz.height as f64 / scale),
            _ => (860.0, 118.0),
        };
        position_bottom_center(&dock, w, h, taskbar_hidden);
    }
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

fn position_top_center(win: &tauri::WebviewWindow, width: f64) {
    if let Ok(Some(mon)) = win.primary_monitor() {
        let scale = mon.scale_factor();
        let mon_w = mon.size().width as f64 / scale;
        let x = ((mon_w - width) / 2.0).max(0.0);
        let _ = win.set_position(tauri::LogicalPosition::new(x, 6.0));
    }
}

fn position_bottom_center(
    win: &tauri::WebviewWindow,
    width: f64,
    height: f64,
    taskbar_hidden: bool,
) {
    if let Ok(Some(mon)) = win.primary_monitor() {
        let scale = mon.scale_factor();
        let (left, top, area_w, area_h) = if taskbar_hidden {
            let pos = mon.position();
            let size = mon.size();
            (
                pos.x as f64 / scale,
                pos.y as f64 / scale,
                size.width as f64 / scale,
                size.height as f64 / scale,
            )
        } else {
            let wa = mon.work_area();
            (
                wa.position.x as f64 / scale,
                wa.position.y as f64 / scale,
                wa.size.width as f64 / scale,
                wa.size.height as f64 / scale,
            )
        };
        let x = left + ((area_w - width) / 2.0).max(0.0);
        let y = (top + area_h - height - 2.0).max(top);
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
                    toggle_spotlight(app.clone());
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
                            let mut guard = state.clipboard_history.lock().unwrap();
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
                            let mut guard = state.clipboard_history.lock().unwrap();
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

            if tick_count == 3 || tick_count % 20 == 0 {
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
                position_bottom_center(&dock, 860.0, 108.0, hide_taskbar_on_start);
                apply_webview_memory_policy(&dock, false);
            }
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

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
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
                            if let Ok(mut guard) = state.config.lock() {
                                guard.hide_windows_taskbar = !guard.hide_windows_taskbar;
                                Some(guard.clone())
                            } else {
                                None
                            }
                        };
                        if let Some(cfg) = updated {
                            let _ = save_config(&cfg);
                            let _ = app.emit("antibar://config-changed", &cfg);
                            dock_win::set_windows_taskbar_hidden(cfg.hide_windows_taskbar);
                            reposition_dock(app, cfg.hide_windows_taskbar);
                        }
                    }
                    "spotlight" => toggle_spotlight(app.clone()),
                    "settings" => open_settings(app.clone()),
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
