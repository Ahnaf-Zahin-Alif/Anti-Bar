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
    let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
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
    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
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
        CreateWindowExW, DefWindowProcW, GetSystemMetrics, MoveWindow, PostMessageW,
        RegisterClassExW, SetLayeredWindowAttributes, SetTimer, ShowWindow,
        SystemParametersInfoW, LWA_ALPHA, SM_CXSCREEN, SPI_GETWORKAREA,
        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SW_HIDE, SW_SHOWNOACTIVATE, WM_DESTROY, WM_PAINT,
        WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        WS_EX_TRANSPARENT, WS_POPUP,
    };

    static CLOCK_HWND: AtomicIsize = AtomicIsize::new(0);
    static ACTIVE_CFG: OnceLock<Mutex<ClockWidgetParams>> = OnceLock::new();
    let store = ACTIVE_CFG.get_or_init(|| Mutex::new(params.clone()));
    *store.lock().unwrap_or_else(|e| e.into_inner()) = params.clone();

    unsafe fn work_area_x(align: &str, top_offset: u32) -> (i32, i32) {
        let mut wa = RECT::default();
        if SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut wa as *mut RECT as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_ok()
        {
            let w = (wa.right - wa.left).max(620);
            let (rx, ry) = compute_clock_origin(w, 620, align, top_offset);
            return (wa.left + rx, wa.top + ry);
        }
        let sw = GetSystemMetrics(SM_CXSCREEN);
        compute_clock_origin(sw, 620, align, top_offset)
    }

    let existing = CLOCK_HWND.load(Ordering::SeqCst);
    if existing != 0 {
        let hwnd = HWND(existing as *mut _);
        unsafe {
            if params.enabled {
                let (x, y) = work_area_x(&params.align, params.top_offset);
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
        struct ClockDrawCache {
            factory: ID2D1Factory,
            dw: IDWriteFactory,
            rt: Option<ID2D1HwndRenderTarget>,
        }

        static DRAW_CACHE: OnceLock<Mutex<Option<ClockDrawCache>>> = OnceLock::new();

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
                WM_DESTROY => {
                    CLOCK_HWND.store(0, Ordering::SeqCst);
                    if let Some(cell) = DRAW_CACHE.get() {
                        if let Ok(mut g) = cell.lock() {
                            *g = None;
                        }
                    }
                    LRESULT(0)
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }

        unsafe fn render_d2d_clock(hwnd: HWND, cfg: &ClockWidgetParams) -> windows::core::Result<()> {
            let cell = DRAW_CACHE.get_or_init(|| Mutex::new(None));
            let mut guard = cell.lock().unwrap_or_else(|e| e.into_inner());
            if guard.is_none() {
                let factory: ID2D1Factory =
                    D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
                let dw: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
                *guard = Some(ClockDrawCache { factory, dw, rt: None });
            }
            let cache = guard.as_mut().unwrap();
            if cache.rt.is_none() {
                let props = D2D1_RENDER_TARGET_PROPERTIES::default();
                let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                    hwnd,
                    pixelSize: windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_U {
                        width: 620,
                        height: 220,
                    },
                    ..Default::default()
                };
                cache.rt = Some(cache.factory.CreateHwndRenderTarget(&props, &hwnd_props)?);
            }
            let fmt = cache.dw.CreateTextFormat(
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

            let base: ID2D1RenderTarget = cache.rt.as_ref().unwrap().cast()?;
            base.BeginDraw();
            let bg = match cfg.style.as_str() {
                "glass" => D2D1_COLOR_F { r: 0.08, g: 0.1, b: 0.14, a: 1.0 },
                _ => D2D1_COLOR_F { r: 0.04, g: 0.05, b: 0.07, a: 1.0 },
            };
            base.Clear(Some(&bg));
            let brush = base.CreateSolidColorBrush(
                &D2D1_COLOR_F { r: 0.97, g: 0.98, b: 1.0, a: 1.0 },
                None,
            )?;
            let rect = D2D_RECT_F { left: 0.0, top: 24.0, right: 620.0, bottom: 200.0 };
            base.DrawText(&wide, &fmt, &rect, &brush, Default::default(), Default::default());
            if base.EndDraw(None, None).is_err() {
                cache.rt = None;
            }
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
        let (x, y) = work_area_x(&params.align, params.top_offset);
        let Ok(hwnd) = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED,
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
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 210, LWA_ALPHA);
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
