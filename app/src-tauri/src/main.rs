#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    anti_bar_lib::run();
}

#[cfg(test)]
mod tests {
    use anti_bar_lib::{audio_mixer, clipboard, config, dock_win, native_ui, spotlight, system_panels};

    #[test]
    fn milestone1_config_defaults_and_roundtrip() {
        let cfg = config::AppConfig::default();
        let json = serde_json::to_string(&cfg).expect("serialize");
        let parsed: config::AppConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.icon_size, 48);
        assert_eq!(parsed.glass_material, "frosted");
        assert!(parsed.hide_windows_taskbar);
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

    #[test]
    fn audit_io_config_sanitize_and_corruption() {
        let mut cfg = config::AppConfig {
            icon_size: 999,
            magnification: 12.0,
            clock_size: 4,
            clock_top_offset: 9000,
            theme: "neon".into(),
            glass_material: "plastic".into(),
            icon_style: "3d".into(),
            downloads_view: "carousel".into(),
            clock_align: "diagonal".into(),
            language: "id".into(),
            ..Default::default()
        };
        config::sanitize_config(&mut cfg);
        assert_eq!(cfg.icon_size, 64);
        assert!((cfg.magnification - 1.65).abs() < 1e-6);
        assert_eq!(cfg.clock_size, 56);
        assert_eq!(cfg.clock_top_offset, 320);
        assert_eq!(cfg.theme, "dark");
        assert_eq!(cfg.glass_material, "frosted");
        assert_eq!(cfg.icon_style, "squircle");
        assert_eq!(cfg.downloads_view, "grid");
        assert_eq!(cfg.clock_align, "center");
        assert_eq!(cfg.language, "en");

        let mut min_cfg = config::AppConfig {
            icon_size: 0,
            magnification: f32::NAN,
            clock_size: 500,
            clock_top_offset: 0,
            ..Default::default()
        };
        config::sanitize_config(&mut min_cfg);
        assert_eq!(min_cfg.icon_size, 36);
        assert!((min_cfg.magnification - 1.38).abs() < 1e-6);
        assert_eq!(min_cfg.clock_size, 180);
        assert_eq!(min_cfg.clock_top_offset, 24);

        // Corrupted / partial JSON must fail parse or deserialize missing fields cleanly via default()
        assert!(serde_json::from_str::<config::AppConfig>("{bad json").is_err());
        let partial: config::AppConfig =
            serde_json::from_str(r#"{"iconSize":52,"theme":"light"}"#).expect("partial config");
        assert_eq!(partial.icon_size, 52);
        assert_eq!(partial.theme, "light");
        assert!(partial.language == "en");
    }

    #[test]
    fn audit_io_ics_and_https_url() {
        use native_ui::desktop_widgets::{parse_https_url, parse_ics_events};

        assert_eq!(parse_https_url("http://example.com/a.ics"), None);
        assert_eq!(parse_https_url("https://"), None);
        assert_eq!(parse_https_url("https://bad host/a.ics"), None);
        assert_eq!(
            parse_https_url("  https://calendar.example.com/u/1/basic.ics?t=1  "),
            Some(("calendar.example.com".into(), "/u/1/basic.ics?t=1".into()))
        );

        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nDTSTART;TZID=Asia/Dhaka:20261125T093000\r\nSUMMARY;LANGUAGE=en-US:Quarterly\\, Q4\\; Architecture \r\n Review 🚀\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nSUMMARY:Unclosed Event\r\nBEGIN:VEVENT\r\nDTSTART:20261126\r\nSUMMARY:\r\nEND:VEVENT\r\nEND:VCALENDAR";
        let events = parse_ics_events(ics);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].dtstart, "20261125T093000");
        assert!(events[0].summary.contains("Quarterly, Q4; Architecture Review 🚀"));
    }

    #[test]
    fn audit_io_spotlight_eval_and_filter() {
        assert_eq!(spotlight::eval_math_expr(""), None);
        assert_eq!(spotlight::eval_math_expr("   "), None);
        assert_eq!(spotlight::eval_math_expr("42"), None);
        assert_eq!(spotlight::eval_math_expr("1 / 0"), None);
        assert_eq!(spotlight::eval_math_expr("(1 + 2"), None);
        assert_eq!(spotlight::eval_math_expr("1 + alert(1)"), None);
        assert_eq!(spotlight::eval_math_expr("1000^1000"), None);
        let res = spotlight::eval_math_expr("(2 + 3) * 4^2 / 5 - 1.5").expect("math");
        assert!((res - 14.5).abs() < 1e-9);
        assert_eq!(spotlight::eval_math_expr("-3 + 5"), Some(2.0));

        let entries = vec![
            spotlight::SpotlightEntry {
                id: "1".into(),
                name: "Visual Studio Code".into(),
                subtitle: "Microsoft Dev Editor".into(),
                kind: "app".into(),
                path: "code.exe".into(),
                icon_data_url: None,
            },
            spotlight::SpotlightEntry {
                id: "2".into(),
                name: "Term".into(),
                subtitle: "Windows Terminal • 日本語".into(),
                kind: "app".into(),
                path: "wt.exe".into(),
                icon_data_url: None,
            },
        ];
        assert!(spotlight::filter_spotlight_entries(&[], "a", 10).is_empty());
        assert!(spotlight::filter_spotlight_entries(&entries, "", 0).is_empty());
        assert_eq!(spotlight::filter_spotlight_entries(&entries, "dev editor", 5).len(), 1);
        assert_eq!(spotlight::filter_spotlight_entries(&entries, "日本語", 5).len(), 1);
    }

    #[test]
    fn audit_io_lrc_wifi_filetime_and_clipboard_validation() {
        // FILETIME underflow guard (< Jan 1 1970) and valid post-epoch timestamp
        assert_eq!(system_panels::filetime_ticks_to_unix_ms(-500), 0);
        assert_eq!(system_panels::filetime_ticks_to_unix_ms(0), 0);
        assert_eq!(system_panels::filetime_ticks_to_unix_ms(116_444_736_000_000_000), 0);
        assert_eq!(system_panels::filetime_ticks_to_unix_ms(116_444_736_000_000_000 + 25_000), 2);

        // Multi-tag or out-of-order LRC sorting, negative-ts rejection, and blank-line filtering
        let lrc = "[01:10.00][00:08.50] Late\n[bad] Ignore\n[-01:00.00] Neg\n[00:02.250] Early\n[00:05.00] ";
        let parsed = system_panels::parse_lrc(lrc);
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].text, "Early");
        assert_eq!(parsed[1].text, "Late");
        assert!((parsed[1].time_sec - 8.5).abs() < 1e-6);
        assert_eq!(parsed[2].text, "Late");
        assert!((parsed[2].time_sec - 70.0).abs() < 1e-6);

        // Clipboard push/dedup/35-cap bounds
        let mut clips = Vec::new();
        assert!(!clipboard::push_clipboard_text(&mut clips, "   \n\t ".into()));
        for i in 0..45 {
            assert!(clipboard::push_clipboard_text(&mut clips, format!("clip-{i}")));
        }
        assert_eq!(clips.len(), 35);
        assert_eq!(clips[0].content, "clip-44");
        // Re-pushing existing item returns false (duplicate guard)
        assert!(!clipboard::push_clipboard_text(&mut clips, "clip-44".into()));

        // Adversarial PNG payloads must be rejected before opening clipboard
        assert!(clipboard::write_clipboard_png_data_url("").is_err());
        assert!(clipboard::write_clipboard_png_data_url("data:image/jpeg;base64,/9j/").is_err());
        assert!(clipboard::write_clipboard_png_data_url("data:image/png;base64,!!!").is_err());
        assert!(clipboard::write_clipboard_png_data_url("data:image/png;base64,AAAA").is_err());
    }

    #[test]
    fn audit_position_multi_monitor_and_bounds() {
        // Primary monitor at origin
        let (x0, y0, w0) = anti_bar_lib::compute_top_center_xy(0.0, 0.0, 1920.0, 260.0);
        assert!((x0 - 830.0).abs() < 1e-6 && (y0 - 6.0).abs() < 1e-6 && (w0 - 260.0).abs() < 1e-6);

        // Negative left monitor at (-1920, -120)
        let (x1, y1, _) = anti_bar_lib::compute_top_center_xy(-1920.0, -120.0, 1920.0, 380.0);
        assert!((x1 - (-1150.0)).abs() < 1e-6 && (y1 - (-114.0)).abs() < 1e-6);

        // Narrow/small DPI-scaled work area must clamp dock inside monitor bounds
        let (bx, by, bw, bh) =
            anti_bar_lib::compute_bottom_center_rect(-1280.0, 40.0, 800.0, 480.0, 1040.0, 500.0);
        assert!(bx >= -1280.0);
        assert!(bw <= 792.0);
        assert!(by >= 40.0);
        assert!(bh <= 472.0);
        assert!(by + bh <= 40.0 + 480.0);

        // Clock narrow screen guard
        let (cx, cy) = native_ui::desktop_clock::compute_clock_origin(400, 620, "right", 40);
        assert_eq!((cx, cy), (0, 40));

        // Magnetic snap edge cases: empty siblings & out-of-threshold
        let dragged = native_ui::desktop_widgets::WidgetRect { x: 200, y: 300, w: 300, h: 150 };
        assert_eq!(native_ui::desktop_widgets::snap_widget_position(dragged, &[], 14), (200, 300));
    }

    #[test]
    fn audit_live_win32_readonly_and_error_guards() {
        let windows = dock_win::enumerate_running_windows();
        for w in &windows {
            assert_ne!(w.hwnd, 0);
            assert!(!w.title.trim().is_empty());
        }
        let trash = dock_win::query_recycle_bin();
        assert!(trash.items < 10_000_000);
        assert!(dock_win::list_downloads().len() <= 12);
        let _ = dock_win::is_fullscreen_active();

        // Invalid HWND preview returns None cleanly without panic or crash
        assert!(dock_win::capture_window_preview(0).is_none());
        assert!(dock_win::capture_window_preview(-999_999).is_none());

        // Error validation on mixer & power endpoints
        assert!(audio_mixer::set_default_output_device("   ").is_err());
        assert!(system_panels::execute_power_action("format_c").is_err());

        // Live read-only audio + system metrics sanity
        let _ = audio_mixer::list_output_devices();
        let _ = audio_mixer::list_audio_sessions();
        let status = system_panels::get_system_status();
        let _ = status.ac_online;
        if status.has_battery {
            assert!(status.battery_percent <= 100);
        }
    }

    #[test]
    fn audit_batch3_edge_regressions() {
        // 1. Huge/deeply-nested spotlight math (>256 chars) rejected cleanly without stack overflow
        let deep = format!("{}1{}", "(".repeat(200), ")".repeat(200));
        assert_eq!(spotlight::eval_math_expr(&deep), None);

        // 2. Non-finite seek bounds clamped safely
        assert_eq!(system_panels::clamp_seek_seconds(f64::NAN, 120.0), 0.0);
        assert_eq!(system_panels::clamp_seek_seconds(f64::INFINITY, 120.0), 0.0);
        assert_eq!(system_panels::clamp_seek_seconds(999.0, 120.0), 120.0);

        // 3. Partial nested MemoTodoItem & AgendaItem deserialize cleanly with defaults
        let partial_nested: config::AppConfig = serde_json::from_str(
            r#"{"memoTodos":[{"text":"Hello"}],"agendaItems":[{"title":"Standup"}]}"#,
        )
        .expect("nested partial config");
        assert_eq!(partial_nested.memo_todos[0].text, "Hello");
        assert!(!partial_nested.memo_todos[0].done);
        assert_eq!(partial_nested.agenda_items[0].title, "Standup");

        // 4. .ics events sorted chronologically by dtstart
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20261201T090000\nSUMMARY:Later\nEND:VEVENT\nBEGIN:VEVENT\nDTSTART:20261125T080000\nSUMMARY:Earlier\nEND:VEVENT\nEND:VCALENDAR";
        let sorted = native_ui::desktop_widgets::parse_ics_events(ics);
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].summary, "Earlier");
        assert_eq!(sorted[1].summary, "Later");
    }

    #[test]
    fn audit_dock_y_all_taskbar_and_clock_combinations() {
        let mon = (0.0, 0.0, 1920.0, 1080.0);
        let wa = (0.0, 0.0, 1920.0, 1032.0);

        let (_, y_ff, _, _) = anti_bar_lib::compute_dock_bounds(mon, wa, 540.0, 108.0, false, false);
        let (_, y_ft, _, _) = anti_bar_lib::compute_dock_bounds(mon, wa, 650.0, 108.0, false, true);
        let (_, y_tf, _, _) = anti_bar_lib::compute_dock_bounds(mon, wa, 540.0, 108.0, true, false);
        let (_, y_tt, _, _) = anti_bar_lib::compute_dock_bounds(mon, wa, 650.0, 108.0, true, true);

        // taskbar visible -> anchors to work_area bottom (1032 - 108 - 2 = 922) regardless of show_dock_clock
        assert!((y_ff - 922.0).abs() < 1e-6);
        assert!((y_ft - 922.0).abs() < 1e-6);
        // taskbar hidden -> anchors to full monitor bottom (1080 - 108 - 2 = 970) regardless of show_dock_clock
        assert!((y_tf - 970.0).abs() < 1e-6);
        assert!((y_tt - 970.0).abs() < 1e-6);
    }
}
