use crate::config::AppConfig;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, PartialEq)]
pub struct ClockWidgetParams {
    pub enabled: bool,
    pub style: String,
    pub align: String,
    pub size: u32,
    pub top_offset: u32,
}

static LAST_PARAMS: OnceLock<Mutex<Option<ClockWidgetParams>>> = OnceLock::new();

pub fn compute_clock_origin(screen_w: i32, widget_w: i32, align: &str, top_offset: u32) -> (i32, i32) {
    let x = match align {
        "left" => 48,
        "right" => (screen_w - widget_w - 48).max(0),
        _ => ((screen_w - widget_w) / 2).max(0),
    };
    (x, top_offset as i32)
}

pub fn sync_clock_widget(cfg: &AppConfig) {
    let params = ClockWidgetParams {
        enabled: cfg.clock_enabled,
        style: cfg.clock_style.clone(),
        align: cfg.clock_align.clone(),
        size: cfg.clock_size.clamp(56, 180),
        top_offset: cfg.clock_top_offset.clamp(24, 320),
    };
    let lock = LAST_PARAMS.get_or_init(|| Mutex::new(None));
    let mut guard = lock.lock().unwrap();
    if guard.as_ref() == Some(&params) {
        return;
    }
    *guard = Some(params.clone());

    #[cfg(windows)]
    spawn_or_update_clock_window(params);
}

#[cfg(windows)]
fn spawn_or_update_clock_window(params: ClockWidgetParams) {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use windows::core::{w, Interface};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
    use windows::Win32::Graphics::Direct2D::{
        D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1RenderTarget,
        D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_HWND_RENDER_TARGET_PROPERTIES,
        D2D1_RENDER_TARGET_PROPERTIES,
    };
    use windows::Win32::Graphics::DirectWrite::{
        DWriteCreateFactory, IDWriteFactory, DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL,
        DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER,
    };
    use windows::Win32::Graphics::Gdi::InvalidateRect;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, GetSystemMetrics, MoveWindow,
        PostMessageW, RegisterClassExW, SetTimer, ShowWindow, SM_CXSCREEN, SW_HIDE,
        SW_SHOWNOACTIVATE, WM_DESTROY, WM_PAINT, WM_TIMER, WNDCLASSEXW, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
    };

    static CLOCK_HWND: AtomicIsize = AtomicIsize::new(0);
    static ACTIVE_CFG: OnceLock<Mutex<ClockWidgetParams>> = OnceLock::new();
    let store = ACTIVE_CFG.get_or_init(|| Mutex::new(params.clone()));
    *store.lock().unwrap() = params.clone();

    let existing = CLOCK_HWND.load(Ordering::SeqCst);
    if existing != 0 {
        let hwnd = HWND(existing as *mut _);
        unsafe {
            if params.enabled {
                let sw = GetSystemMetrics(SM_CXSCREEN);
                let (x, y) = compute_clock_origin(sw, 620, &params.align, params.top_offset);
                let _ = MoveWindow(hwnd, x, y, 620, 220, true);
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                let _ = InvalidateRect(hwnd, None, true);
            } else {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
        return;
    }

    if !params.enabled {
        return;
    }

    std::thread::spawn(move || unsafe {
        unsafe extern "system" fn wnd_proc(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            match msg {
                WM_TIMER => {
                    let _ = InvalidateRect(hwnd, None, false);
                    LRESULT(0)
                }
                WM_PAINT => {
                    let cfg = ACTIVE_CFG
                        .get()
                        .and_then(|m| m.lock().ok())
                        .map(|g| g.clone());
                    if let Some(c) = cfg {
                        let _ = render_d2d_clock(hwnd, &c);
                    }
                    let _ = windows::Win32::Graphics::Gdi::ValidateRect(hwnd, None);
                    LRESULT(0)
                }
                WM_DESTROY => LRESULT(0),
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }

        unsafe fn render_d2d_clock(hwnd: HWND, cfg: &ClockWidgetParams) -> windows::core::Result<()> {
            let factory: ID2D1Factory =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dw: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let props = D2D1_RENDER_TARGET_PROPERTIES::default();
            let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd,
                pixelSize: windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_U {
                    width: 620,
                    height: 220,
                },
                ..Default::default()
            };
            let rt: ID2D1HwndRenderTarget = factory.CreateHwndRenderTarget(&props, &hwnd_props)?;
            let fmt = dw.CreateTextFormat(
                w!("Segoe UI Variable Display"),
                None,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                cfg.size as f32,
                w!("en-US"),
            )?;
            fmt.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;

            let st = windows::Win32::System::SystemInformation::GetLocalTime();
            let time_str = format!("{:02}:{:02}", st.wHour, st.wMinute);
            let wide: Vec<u16> = time_str.encode_utf16().collect();

            let base: ID2D1RenderTarget = rt.cast()?;
            base.BeginDraw();
            let bg = match cfg.style.as_str() {
                "glass" => D2D1_COLOR_F { r: 0.08, g: 0.1, b: 0.14, a: 0.72 },
                _ => D2D1_COLOR_F { r: 0.04, g: 0.05, b: 0.07, a: 0.55 },
            };
            base.Clear(Some(&bg));
            let brush = base.CreateSolidColorBrush(
                &D2D1_COLOR_F { r: 0.97, g: 0.98, b: 1.0, a: 0.96 },
                None,
            )?;
            let rect = D2D_RECT_F { left: 0.0, top: 24.0, right: 620.0, bottom: 200.0 };
            base.DrawText(&wide, &fmt, &rect, &brush, Default::default(), Default::default());
            let _ = base.EndDraw(None, None);
            Ok(())
        }

        let cls = w!("AntiBarDesktopClockClass");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wnd_proc),
            lpszClassName: cls,
            ..Default::default()
        };
        RegisterClassExW(&wc);
        let sw = GetSystemMetrics(SM_CXSCREEN);
        let (x, y) = compute_clock_origin(sw, 620, &params.align, params.top_offset);
        let Ok(hwnd) = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
            cls,
            w!("Anti-Bar Desktop Clock"),
            WS_POPUP,
            x,
            y,
            620,
            220,
            None,
            None,
            None,
            None,
        ) else {
            return;
        };
        CLOCK_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
        let _ = SetTimer(hwnd, 1, 15_000, None);
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = PostMessageW(hwnd, WM_PAINT, WPARAM(0), LPARAM(0));

        let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
        while windows::Win32::UI::WindowsAndMessaging::GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
            windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
        }
    });
}
