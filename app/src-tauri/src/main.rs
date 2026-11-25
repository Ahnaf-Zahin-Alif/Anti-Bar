#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    anti_bar_lib::run();
}

#[cfg(test)]
mod tests {
    use anti_bar_lib::{audio_mixer, clipboard, config, native_ui, spotlight, system_panels};

    #[test]
    fn milestone1_config_defaults_and_roundtrip() {
        let cfg = config::AppConfig::default();
        let json = serde_json::to_string(&cfg).expect("serialize");
        let parsed: config::AppConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.icon_size, 48);
        assert_eq!(parsed.glass_material, "frosted");
        assert!(!parsed.hide_windows_taskbar);
        assert!(!parsed.use_win_key_for_launchpad);
        assert_eq!(parsed.pinned_apps.len(), 4);
    }

    #[test]
    fn milestone1_spotlight_math_eval() {
        assert_eq!(spotlight::eval_math_expr("24 * 18"), Some(432.0));
        assert_eq!(spotlight::eval_math_expr("(2 + 3) * 4"), Some(20.0));
        assert_eq!(spotlight::eval_math_expr("2^3^2"), Some(512.0));
        assert_eq!(spotlight::eval_math_expr("10 / 0"), None);
        assert_eq!(spotlight::eval_math_expr("42"), None);
    }

    #[test]
    fn milestone1_clipboard_dedup_and_url() {
        let mut items = Vec::new();
        assert!(clipboard::push_clipboard_text(&mut items, "https://example.com".into()));
        assert_eq!(items[0].kind, "url");
        assert!(!clipboard::push_clipboard_text(&mut items, "https://example.com".into()));
        assert!(clipboard::push_clipboard_text(&mut items, "hello world".into()));
        assert_eq!(items[0].kind, "text");
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn milestone1_lrc_parsing() {
        let lines = system_panels::parse_lrc("[00:12.50] First\n[01:02.00] Second\ninvalid");
        assert_eq!(lines.len(), 2);
        assert!((lines[0].time_sec - 12.5).abs() < 0.01);
        assert_eq!(lines[1].text, "Second");
    }

    #[test]
    fn milestone2_audio_clamp_and_wifi_dedup() {
        assert_eq!(audio_mixer::clamp_volume(1.4), 1.0);
        assert_eq!(audio_mixer::clamp_volume(-0.2), 0.0);
        assert_eq!(audio_mixer::clamp_volume(f32::NAN), 0.0);

        let nets = system_panels::dedup_sort_wifi(vec![
            system_panels::WifiNetwork {
                ssid: "Office".into(),
                signal_quality: 40,
                secured: true,
                connected: false,
            },
            system_panels::WifiNetwork {
                ssid: "Office".into(),
                signal_quality: 85,
                secured: true,
                connected: true,
            },
            system_panels::WifiNetwork {
                ssid: "".into(),
                signal_quality: 90,
                secured: false,
                connected: false,
            },
        ]);
        assert_eq!(nets.len(), 1);
        assert!(nets[0].connected);
        assert_eq!(nets[0].signal_quality, 85);
        assert!(system_panels::validate_power_action("lock"));
        assert!(!system_panels::validate_power_action("rm_rf"));
    }

    #[test]
    fn milestone3_widgets_ics_snap_and_hlsl() {
        let (x, y) = native_ui::desktop_clock::compute_clock_origin(1920, 620, "center", 80);
        assert_eq!((x, y), (650, 80));

        let anchor = native_ui::desktop_widgets::WidgetRect { x: 48, y: 120, w: 332, h: 148 };
        let dragged = native_ui::desktop_widgets::WidgetRect { x: 54, y: 290, w: 332, h: 200 };
        let (sx, sy) = native_ui::desktop_widgets::snap_widget_position(dragged, &[anchor], 14);
        assert_eq!((sx, sy), (48, 284));

        let events = native_ui::desktop_widgets::parse_ics_events(
            "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20261125T100000Z\nSUMMARY:Design Review\nEND:VEVENT\nEND:VCALENDAR",
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].summary, "Design Review");

        assert!(native_ui::genie::genie_squeeze_width(1.0, 1.0) < 0.2);
        let bytecode_len = native_ui::genie::verify_d3d11_shader_compile().expect("HLSL compile");
        assert!(bytecode_len > 0);
    }

    #[test]
    fn milestone4_wizzly_parity_helpers() {
        assert_eq!(system_panels::clamp_seek_seconds(-4.0, 120.0), 0.0);
        assert_eq!(system_panels::clamp_seek_seconds(140.0, 120.0), 120.0);
        assert_eq!(system_panels::clamp_seek_seconds(45.5, 120.0), 45.5);

        let mut clips = Vec::new();
        assert!(clipboard::push_clipboard_image(
            &mut clips,
            "data:image/png;base64,AAA=".into()
        ));
        assert_eq!(clips[0].kind, "image");
        assert!(!clipboard::push_clipboard_image(
            &mut clips,
            "data:image/png;base64,AAA=".into()
        ));

        let dev = audio_mixer::AudioOutputDevice {
            id: "dev-1".into(),
            name: "Headphones".into(),
            is_default: true,
        };
        let json = serde_json::to_string(&dev).expect("serialize device");
        assert!(json.contains("isDefault"));
    }
}
