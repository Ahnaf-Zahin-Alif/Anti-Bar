use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedApp {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub args: Option<String>,
    #[serde(default)]
    pub icon_data_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MemoTodoItem {
    pub id: String,
    pub text: String,
    pub done: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgendaItem {
    pub id: String,
    pub date: String,
    pub time: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub language: String,             // "en"
    pub theme: String,                // "dark" | "light"
    pub glass_material: String,       // "frosted" | "liquid" | "matte" | "classic"
    pub wallpaper_tint: bool,
    pub icon_style: String,           // "windows" | "squircle" | "normalized"
    pub icon_size: u32,               // 36..64
    pub magnification: f32,           // 1.0..1.65
    pub auto_hide_dock: bool,
    pub hide_in_fullscreen: bool,
    pub hide_windows_taskbar: bool,   // default true (restored on exit)
    #[serde(default)]
    pub monitor_mode: String,         // "primary" | "cursor" | "custom"
    #[serde(default)]
    pub monitor_name: String,
    #[serde(default)]
    pub show_dock_clock: bool,        // optional clock + notifications pill in Dock
    pub genie_effect: bool,
    pub use_win_key_for_launchpad: bool, // opt-in, default false
    pub notch_enabled: bool,
    pub show_lyrics: bool,
    pub downloads_view: String,       // "grid" | "list"
    pub pinned_apps: Vec<PinnedApp>,
    // Milestone 3 widget config
    pub clock_enabled: bool,
    pub clock_style: String,          // "liquid" | "glass" | "sonoma"
    pub clock_align: String,          // "left" | "center" | "right"
    pub clock_size: u32,
    pub clock_top_offset: u32,
    pub vinyl_enabled: bool,
    pub vinyl_style: String,          // "glass" | "nothingDark" | "nothingLight"
    pub calendar_enabled: bool,
    pub calendar_style: String,
    pub calendar_layout: String,      // "agenda" | "square"
    pub calendar_ics_url: String,
    pub memo_enabled: bool,
    pub memo_style: String,           // "yellow" | "glass" | "nothingDark" | "nothingLight"
    pub memo_title: String,
    pub memo_todos: Vec<MemoTodoItem>,
    pub agenda_items: Vec<AgendaItem>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: "en".into(),
            theme: "dark".into(),
            glass_material: "frosted".into(),
            wallpaper_tint: true,
            icon_style: "squircle".into(),
            icon_size: 48,
            magnification: 1.38,
            auto_hide_dock: false,
            hide_in_fullscreen: true,
            hide_windows_taskbar: true,
            monitor_mode: "primary".into(),
            monitor_name: String::new(),
            show_dock_clock: false,
            genie_effect: true,
            use_win_key_for_launchpad: false,
            notch_enabled: true,
            show_lyrics: true,
            downloads_view: "grid".into(),
            pinned_apps: vec![
                PinnedApp {
                    id: "explorer".into(),
                    name: "File Explorer".into(),
                    path: "explorer.exe".into(),
                    args: None,
                    icon_data_url: None,
                },
                PinnedApp {
                    id: "edge".into(),
                    name: "Microsoft Edge".into(),
                    path: "msedge.exe".into(),
                    args: None,
                    icon_data_url: None,
                },
                PinnedApp {
                    id: "terminal".into(),
                    name: "Terminal".into(),
                    path: "wt.exe".into(),
                    args: None,
                    icon_data_url: None,
                },
                PinnedApp {
                    id: "notepad".into(),
                    name: "Notepad".into(),
                    path: "notepad.exe".into(),
                    args: None,
                    icon_data_url: None,
                },
            ],
            clock_enabled: false,
            clock_style: "liquid".into(),
            clock_align: "center".into(),
            clock_size: 96,
            clock_top_offset: 64,
            vinyl_enabled: false,
            vinyl_style: "glass".into(),
            calendar_enabled: false,
            calendar_style: "glass".into(),
            calendar_layout: "agenda".into(),
            calendar_ics_url: String::new(),
            memo_enabled: false,
            memo_style: "yellow".into(),
            memo_title: "Quick Notes".into(),
            memo_todos: vec![
                MemoTodoItem {
                    id: "t1".into(),
                    text: "Review daily tasks".into(),
                    done: false,
                },
            ],
            agenda_items: vec![],
        }
    }
}

pub fn app_data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("com.antibar.app");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn config_path() -> PathBuf {
    app_data_dir().join("config.json")
}

pub fn sanitize_config(cfg: &mut AppConfig) {
    if cfg.theme != "dark" && cfg.theme != "light" {
        cfg.theme = "dark".into();
    }
    if !matches!(
        cfg.glass_material.as_str(),
        "frosted" | "liquid" | "matte" | "classic"
    ) {
        cfg.glass_material = "frosted".into();
    }
    if !matches!(
        cfg.icon_style.as_str(),
        "windows" | "squircle" | "normalized"
    ) {
        cfg.icon_style = "squircle".into();
    }
    if cfg.downloads_view != "grid" && cfg.downloads_view != "list" {
        cfg.downloads_view = "grid".into();
    }
    cfg.icon_size = cfg.icon_size.clamp(36, 64);
    cfg.magnification = if cfg.magnification.is_finite() {
        cfg.magnification.clamp(1.0, 1.65)
    } else {
        1.38
    };
    cfg.clock_size = cfg.clock_size.clamp(56, 180);
    cfg.clock_top_offset = cfg.clock_top_offset.clamp(24, 320);
    if !matches!(cfg.clock_align.as_str(), "left" | "center" | "right") {
        cfg.clock_align = "center".into();
    }
    if !matches!(cfg.monitor_mode.as_str(), "primary" | "cursor" | "custom") {
        cfg.monitor_mode = "primary".into();
    }
    cfg.language = "en".into();
}

pub fn load_config() -> AppConfig {
    let path = config_path();
    let mut cfg = match fs::read_to_string(&path) {
        Ok(raw) => match serde_json::from_str::<AppConfig>(&raw) {
            Ok(parsed) => parsed,
            Err(_) => {
                let backup = path.with_extension("corrupt.json");
                let _ = fs::write(&backup, &raw);
                AppConfig::default()
            }
        },
        Err(_) => {
            let c = AppConfig::default();
            let _ = save_config(&c);
            c
        }
    };
    sanitize_config(&mut cfg);
    cfg
}

pub fn save_config(cfg: &AppConfig) -> Result<(), String> {
    let mut clean = cfg.clone();
    sanitize_config(&mut clean);
    let path = config_path();
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(&clean).map_err(|e| e.to_string())?;
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    if fs::rename(&tmp, &path).is_err() {
        let _ = fs::remove_file(&path);
        fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_roundtrip() {
        let cfg = AppConfig::default();
        let json = serde_json::to_string(&cfg).expect("serialize");
        let parsed: AppConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.icon_size, 48);
        assert_eq!(parsed.glass_material, "frosted");
        assert!(parsed.hide_windows_taskbar);
        assert!(!parsed.use_win_key_for_launchpad);
        assert_eq!(parsed.pinned_apps.len(), 4);
    }
}
