use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::mem::size_of;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningWindow {
    pub hwnd: isize,
    pub title: String,
    pub exe_path: String,
    pub exe_name: String,
    pub is_focused: bool,
    pub is_minimized: bool,
    pub icon_data_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub modified_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashInfo {
    pub items: i64,
    pub size_bytes: i64,
}

static ICON_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

fn icon_cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    ICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(windows)]
fn hicon_to_png_data_url(hicon: windows::Win32::UI::WindowsAndMessaging::HICON) -> Option<String> {
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, SelectObject, BITMAP,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

    if hicon.0.is_null() {
        return None;
    }

    unsafe {
        let mut info = ICONINFO::default();
        if GetIconInfo(hicon, &mut info).is_err() {
            return None;
        }
        let bmp_handle = if !info.hbmColor.is_invalid() {
            info.hbmColor
        } else {
            info.hbmMask
        };
        let mut bm = BITMAP::default();
        if GetObjectW(
            bmp_handle,
            size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut BITMAP as *mut _),
        ) == 0
        {
            if !info.hbmColor.is_invalid() {
                let _ = DeleteObject(info.hbmColor);
            }
            if !info.hbmMask.is_invalid() {
                let _ = DeleteObject(info.hbmMask);
            }
            return None;
        }

        let w = bm.bmWidth.max(1);
        let h = bm.bmHeight.abs().max(1);
        let hdc = CreateCompatibleDC(None);
        let old = SelectObject(hdc, bmp_handle);

        let mut bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bgra = vec![0u8; (w * h * 4) as usize];
        let rows = GetDIBits(
            hdc,
            bmp_handle,
            0,
            h as u32,
            Some(bgra.as_mut_ptr() as *mut _),
            &mut bi,
            DIB_RGB_COLORS,
        );

        SelectObject(hdc, old);
        let _ = DeleteDC(hdc);
        if !info.hbmColor.is_invalid() {
            let _ = DeleteObject(info.hbmColor);
        }
        if !info.hbmMask.is_invalid() {
            let _ = DeleteObject(info.hbmMask);
        }
        if rows == 0 {
            return None;
        }

        // Convert BGRA -> RGBA and check if any alpha is non-zero
        let mut any_alpha = false;
        let (chunks, _) = bgra.as_chunks_mut::<4>();
        for px in chunks {
            px.swap(0, 2);
            if px[3] != 0 {
                any_alpha = true;
            }
        }
        if !any_alpha {
            let (chunks, _) = bgra.as_chunks_mut::<4>();
            for px in chunks {
                if px[0] != 0 || px[1] != 0 || px[2] != 0 {
                    px[3] = 255;
                }
            }
        }

        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, w as u32, h as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().ok()?;
            writer.write_image_data(&bgra).ok()?;
        }
        Some(format!("data:image/png;base64,{}", STANDARD.encode(&out)))
    }
}

#[cfg(windows)]
pub fn extract_path_icon_data_url(path: &str) -> Option<String> {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
    use windows::Win32::UI::Shell::{
        SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_USEFILEATTRIBUTES,
    };
    use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;

    let key = path.to_ascii_lowercase();
    if let Ok(guard) = icon_cache().lock() {
        if let Some(cached) = guard.get(&key) {
            return cached.clone();
        }
    }

    let resolved = resolve_exe_hint(path).unwrap_or_else(|| path.to_string());
    let hpath = HSTRING::from(&resolved);

    let result = unsafe {
        let mut sfi = SHFILEINFOW::default();
        let ok = SHGetFileInfoW(
            &hpath,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut sfi),
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if ok == 0 || sfi.hIcon.0.is_null() {
            let _ = SHGetFileInfoW(
                &hpath,
                FILE_FLAGS_AND_ATTRIBUTES(0x80),
                Some(&mut sfi),
                size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES,
            );
        }
        if sfi.hIcon.0.is_null() {
            None
        } else {
            let data = hicon_to_png_data_url(sfi.hIcon);
            let _ = DestroyIcon(sfi.hIcon);
            data
        }
    };

    if let Ok(mut guard) = icon_cache().lock() {
        if guard.len() >= 48 {
            let evict: Vec<String> = guard.keys().take(16).cloned().collect();
            for k in evict {
                guard.remove(&k);
            }
            guard.shrink_to_fit();
        }
        guard.insert(key, result.clone());
    }
    result
}

#[cfg(not(windows))]
pub fn extract_path_icon_data_url(_path: &str) -> Option<String> {
    None
}

fn resolve_exe_hint(path: &str) -> Option<String> {
    if Path::new(path).exists() {
        return Some(path.to_string());
    }
    if path.eq_ignore_ascii_case("explorer.exe") {
        return Some(r"C:\Windows\explorer.exe".into());
    }
    if path.eq_ignore_ascii_case("notepad.exe") {
        return Some(r"C:\Windows\System32\notepad.exe".into());
    }
    if path.eq_ignore_ascii_case("calc.exe") {
        return Some(r"C:\Windows\System32\calc.exe".into());
    }
    if path.eq_ignore_ascii_case("cmd.exe") {
        return Some(r"C:\Windows\System32\cmd.exe".into());
    }
    if path.eq_ignore_ascii_case("taskmgr.exe") {
        return Some(r"C:\Windows\System32\Taskmgr.exe".into());
    }
    if path.eq_ignore_ascii_case("control.exe") {
        return Some(r"C:\Windows\System32\control.exe".into());
    }
    if path.eq_ignore_ascii_case("mspaint.exe") {
        return Some(r"C:\Windows\System32\mspaint.exe".into());
    }
    if path.eq_ignore_ascii_case("msedge.exe") {
        let p = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe";
        if Path::new(p).exists() {
            return Some(p.into());
        }
    }
    None
}

#[cfg(windows)]
pub fn enumerate_running_windows() -> Vec<RunningWindow> {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowLongW, GetWindowTextLengthW,
        GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, GWL_EXSTYLE,
        WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let list = &mut *(lparam.0 as *mut Vec<(isize, String, String, bool, bool)>);
        if !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1);
        }
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if (ex_style & WS_EX_TOOLWINDOW.0) != 0 && (ex_style & WS_EX_APPWINDOW.0) == 0 {
            return BOOL(1);
        }
        let mut cloaked: u32 = 0;
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut _,
            size_of::<u32>() as u32,
        )
        .is_ok()
            && cloaked != 0
        {
            return BOOL(1);
        }

        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return BOOL(1);
        }
        let mut title_buf = vec![0u16; (len + 1) as usize];
        let copied = GetWindowTextW(hwnd, &mut title_buf);
        if copied <= 0 {
            return BOOL(1);
        }
        let title = String::from_utf16_lossy(&title_buf[..copied as usize]);

        let mut class_buf = [0u16; 128];
        let clen = GetClassNameW(hwnd, &mut class_buf);
        let class_name = String::from_utf16_lossy(&class_buf[..clen as usize]);
        if matches!(
            class_name.as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Windows.UI.Core.CoreWindow"
        ) {
            return BOOL(1);
        }
        if title.starts_with("Anti-Bar") {
            return BOOL(1);
        }

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == std::process::id() {
            return BOOL(1);
        }

        let mut exe_path = String::new();
        if let Ok(proc_h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let mut path_buf = [0u16; 512];
            let mut plen = path_buf.len() as u32;
            if QueryFullProcessImageNameW(
                proc_h,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(path_buf.as_mut_ptr()),
                &mut plen,
            )
            .is_ok()
            {
                exe_path = String::from_utf16_lossy(&path_buf[..plen as usize]);
            }
            let _ = windows::Win32::Foundation::CloseHandle(proc_h);
        }
        if exe_path.is_empty() {
            return BOOL(1);
        }

        let fg = GetForegroundWindow();
        let is_focused = fg.0 == hwnd.0;
        let is_minimized = IsIconic(hwnd).as_bool();
        list.push((hwnd.0 as isize, title, exe_path, is_focused, is_minimized));
        BOOL(1)
    }

    let mut raw: Vec<(isize, String, String, bool, bool)> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut raw as *mut _ as isize));
    }

    raw.into_iter()
        .map(|(hwnd, title, exe_path, is_focused, is_minimized)| {
            let exe_name = Path::new(&exe_path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(&title)
                .to_string();
            let icon_data_url = extract_path_icon_data_url(&exe_path);
            RunningWindow {
                hwnd,
                title,
                exe_path,
                exe_name,
                is_focused,
                is_minimized,
                icon_data_url,
            }
        })
        .collect()
}

#[cfg(not(windows))]
pub fn enumerate_running_windows() -> Vec<RunningWindow> {
    Vec::new()
}

#[cfg(windows)]
pub fn toggle_window_hwnd(hwnd_val: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsHungAppWindow,
        IsIconic, IsWindow, SetForegroundWindow, ShowWindow, SW_MINIMIZE, SW_RESTORE, SW_SHOW,
    };

    let hwnd = HWND(hwnd_val as *mut _);
    unsafe {
        if !IsWindow(hwnd).as_bool() {
            return;
        }
        let fg = GetForegroundWindow();
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        } else if fg.0 == hwnd.0 {
            let _ = ShowWindow(hwnd, SW_MINIMIZE);
            return;
        } else {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }

        let fg_tid = if !fg.0.is_null() && !IsHungAppWindow(fg).as_bool() {
            GetWindowThreadProcessId(fg, None)
        } else {
            0
        };
        let cur_tid = GetCurrentThreadId();
        let attached = fg_tid != 0 && fg_tid != cur_tid && AttachThreadInput(cur_tid, fg_tid, true).as_bool();
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        if attached {
            let _ = AttachThreadInput(cur_tid, fg_tid, false);
        }
    }
}

#[cfg(windows)]
pub fn close_window_hwnd(hwnd_val: isize) {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{IsWindow, PostMessageW, WM_CLOSE};

    let hwnd = HWND(hwnd_val as *mut _);
    unsafe {
        if IsWindow(hwnd).as_bool() {
            let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
}

#[cfg(not(windows))]
pub fn close_window_hwnd(_hwnd_val: isize) {}

#[cfg(windows)]
pub fn capture_window_preview(hwnd_val: isize) -> Option<String> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits,
        SelectObject, SetStretchBltMode, StretchBlt, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS, HALFTONE, SRCCOPY,
    };
    use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, IsIconic, IsWindow};

    let hwnd = HWND(hwnd_val as *mut _);
    unsafe {
        if !IsWindow(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return None;
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return None;
        }
        let src_w = (rect.right - rect.left).max(1);
        let src_h = (rect.bottom - rect.top).max(1);
        if src_w < 32 || src_h < 32 {
            return None;
        }

        let max_w = 184.0_f64;
        let max_h = 108.0_f64;
        let ratio = (max_w / src_w as f64).min(max_h / src_h as f64).min(1.0);
        let dst_w = ((src_w as f64 * ratio).round() as i32).max(48);
        let dst_h = ((src_h as f64 * ratio).round() as i32).max(32);

        let screen_dc = windows::Win32::Graphics::Gdi::GetDC(None);
        let src_dc = CreateCompatibleDC(screen_dc);
        let dst_dc = CreateCompatibleDC(screen_dc);
        let src_bmp = CreateCompatibleBitmap(screen_dc, src_w, src_h);
        let dst_bmp = CreateCompatibleBitmap(screen_dc, dst_w, dst_h);
        windows::Win32::Graphics::Gdi::ReleaseDC(None, screen_dc);

        if src_bmp.is_invalid() || dst_bmp.is_invalid() {
            let _ = DeleteDC(src_dc);
            let _ = DeleteDC(dst_dc);
            if !src_bmp.is_invalid() {
                let _ = DeleteObject(src_bmp);
            }
            if !dst_bmp.is_invalid() {
                let _ = DeleteObject(dst_bmp);
            }
            return None;
        }

        let old_src = SelectObject(src_dc, src_bmp);
        let old_dst = SelectObject(dst_dc, dst_bmp);

        // PW_RENDERFULLCONTENT (2) captures GPU/DComp/WebView2 surfaces cleanly.
        let ok = PrintWindow(hwnd, src_dc, PRINT_WINDOW_FLAGS(2)).as_bool();
        if !ok {
            let _ = PrintWindow(hwnd, src_dc, PRINT_WINDOW_FLAGS(0));
        }

        let _ = SetStretchBltMode(dst_dc, HALFTONE);
        let _ = StretchBlt(dst_dc, 0, 0, dst_w, dst_h, src_dc, 0, 0, src_w, src_h, SRCCOPY);

        let mut bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: dst_w,
                biHeight: -dst_h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bgra = vec![0u8; (dst_w * dst_h * 4) as usize];
        let rows = GetDIBits(
            dst_dc,
            dst_bmp,
            0,
            dst_h as u32,
            Some(bgra.as_mut_ptr() as *mut _),
            &mut bi,
            DIB_RGB_COLORS,
        );

        SelectObject(src_dc, old_src);
        SelectObject(dst_dc, old_dst);
        let _ = DeleteObject(src_bmp);
        let _ = DeleteObject(dst_bmp);
        let _ = DeleteDC(src_dc);
        let _ = DeleteDC(dst_dc);

        if rows == 0 {
            return None;
        }
        let (chunks, _) = bgra.as_chunks_mut::<4>();
        for px in chunks {
            px.swap(0, 2);
            px[3] = 255;
        }

        let mut png_bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png_bytes, dst_w as u32, dst_h as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().ok()?;
            writer.write_image_data(&bgra).ok()?;
        }
        Some(format!("data:image/png;base64,{}", STANDARD.encode(&png_bytes)))
    }
}

#[cfg(not(windows))]
pub fn capture_window_preview(_hwnd_val: isize) -> Option<String> {
    None
}

#[cfg(windows)]
pub fn launch_path(path: &str, args: Option<&str>) -> Result<(), String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let verb = HSTRING::from("open");
    let target = HSTRING::from(path);
    let params = args.map(HSTRING::from);
    let res = unsafe {
        ShellExecuteW(
            None,
            &verb,
            &target,
            params.as_ref().map(|p| PCWSTR(p.as_ptr())).unwrap_or(PCWSTR::null()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if (res.0 as isize) <= 32 {
        Err(format!("ShellExecuteW failed with code {}", res.0 as isize))
    } else {
        Ok(())
    }
}

#[cfg(windows)]
pub fn is_fullscreen_active() -> bool {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_RUNNING_D3D_FULL_SCREEN};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowRect, IsWindowVisible,
    };

    unsafe {
        if let Ok(state) = SHQueryUserNotificationState() {
            if state == QUNS_RUNNING_D3D_FULL_SCREEN {
                return true;
            }
        }
        let fg = GetForegroundWindow();
        if fg.0.is_null() || !IsWindowVisible(fg).as_bool() {
            return false;
        }
        let mut cls = [0u16; 64];
        let clen = GetClassNameW(fg, &mut cls);
        let cname = String::from_utf16_lossy(&cls[..clen as usize]);
        if matches!(
            cname.as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
        ) {
            return false;
        }
        let mut wr = RECT::default();
        if GetWindowRect(fg, &mut wr).is_err() {
            return false;
        }
        let mon = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(mon, &mut mi).as_bool() {
            let mr = mi.rcMonitor;
            return wr.left <= mr.left
                && wr.top <= mr.top
                && wr.right >= mr.right
                && wr.bottom >= mr.bottom;
        }
    }
    false
}

#[cfg(not(windows))]
pub fn is_fullscreen_active() -> bool {
    false
}

pub fn list_downloads() -> Vec<DownloadEntry> {
    let Some(dir) = dirs::download_dir() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut list: Vec<DownloadEntry> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let name = p.file_name()?.to_string_lossy().to_string();
            if name.starts_with('.') || name.eq_ignore_ascii_case("desktop.ini") {
                return None;
            }
            let meta = e.metadata().ok()?;
            let modified_ms = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            Some(DownloadEntry {
                name,
                path: p.to_string_lossy().to_string(),
                is_dir: meta.is_dir(),
                size_bytes: if meta.is_dir() { 0 } else { meta.len() },
                modified_ms,
            })
        })
        .collect();
    list.sort_by_key(|a| std::cmp::Reverse(a.modified_ms));
    list.truncate(24);
    list
}

#[cfg(windows)]
pub fn query_recycle_bin() -> TrashInfo {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO};
    unsafe {
        let mut info = SHQUERYRBINFO {
            cbSize: size_of::<SHQUERYRBINFO>() as u32,
            ..Default::default()
        };
        if SHQueryRecycleBinW(PCWSTR::null(), &mut info).is_ok() {
            TrashInfo {
                items: info.i64NumItems,
                size_bytes: info.i64Size,
            }
        } else {
            TrashInfo {
                items: 0,
                size_bytes: 0,
            }
        }
    }
}

#[cfg(windows)]
pub fn empty_recycle_bin() -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{
        SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND,
    };
    unsafe {
        SHEmptyRecycleBinW(
            None,
            PCWSTR::null(),
            SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
        )
        .map_err(|e| e.to_string())
    }
}

static TASKBAR_HIDDEN_BY_US: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

type SavedWorkArea = (i32, i32, i32, i32);
static ORIGINAL_WORK_AREA: OnceLock<Mutex<Option<SavedWorkArea>>> = OnceLock::new();

fn taskbar_marker_path() -> std::path::PathBuf {
    crate::config::app_data_dir().join(".taskbar_hidden")
}

#[cfg(windows)]
pub fn set_windows_taskbar_hidden(hidden: bool) {
    use std::sync::atomic::Ordering;
    use windows::core::w;
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM, POINT, RECT};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, MONITORINFO,
        MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, FindWindowW, GetClassNameW, GetWindowRect, IsWindowVisible, IsZoomed,
        SetWindowPos, ShowWindow, SystemParametersInfoW, SPIF_SENDCHANGE, SPI_GETWORKAREA,
        SPI_SETWORKAREA, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE, SW_SHOWNA,
    };

    unsafe extern "system" fn taskbar_enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let mut class_buf = [0u16; 64];
        let len = GetClassNameW(hwnd, &mut class_buf);
        if len > 0 {
            let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
            if class_name == "Shell_TrayWnd" || class_name == "Shell_SecondaryTrayWnd" {
                let hide = lparam.0 != 0;
                let _ = ShowWindow(hwnd, if hide { SW_HIDE } else { SW_SHOWNA });
            }
        }
        BOOL(1)
    }

    TASKBAR_HIDDEN_BY_US.store(hidden, Ordering::SeqCst);
    let marker = taskbar_marker_path();
    if hidden {
        let _ = fs::write(&marker, b"1");
    } else {
        let _ = fs::remove_file(&marker);
    }

    unsafe {
        if hidden {
            let cell = ORIGINAL_WORK_AREA.get_or_init(|| Mutex::new(None));
            if let Ok(mut guard) = cell.lock() {
                if guard.is_none() {
                    let mut cur_wa = RECT::default();
                    if SystemParametersInfoW(
                        SPI_GETWORKAREA,
                        0,
                        Some(&mut cur_wa as *mut RECT as *mut _),
                        Default::default(),
                    )
                    .is_ok()
                    {
                        *guard = Some((cur_wa.left, cur_wa.top, cur_wa.right, cur_wa.bottom));
                    }
                }
            }
        }

        let _ = EnumWindows(Some(taskbar_enum_proc), LPARAM(if hidden { 1 } else { 0 }));

        let mon = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut mi = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(mon, &mut mi).as_bool() {
            if hidden {
                let mut full = mi.rcMonitor;
                let _ = SystemParametersInfoW(
                    SPI_SETWORKAREA,
                    0,
                    Some(&mut full as *mut RECT as *mut _),
                    SPIF_SENDCHANGE,
                );
            } else {
                let saved = ORIGINAL_WORK_AREA
                    .get_or_init(|| Mutex::new(None))
                    .lock()
                    .ok()
                    .and_then(|mut g| g.take());
                let mut restore = if let Some((l, t, r, b)) = saved {
                    RECT { left: l, top: t, right: r, bottom: b }
                } else {
                    let mut fallback = mi.rcMonitor;
                    if let Ok(tray) = FindWindowW(w!("Shell_TrayWnd"), None) {
                        let mut tr = RECT::default();
                        if !tray.0.is_null() && GetWindowRect(tray, &mut tr).is_ok() {
                            let th = (tr.bottom - tr.top).clamp(32, 96);
                            if tr.top >= fallback.bottom - 120 {
                                fallback.bottom -= th;
                            }
                        }
                    }
                    fallback
                };
                let _ = SystemParametersInfoW(
                    SPI_SETWORKAREA,
                    0,
                    Some(&mut restore as *mut RECT as *mut _),
                    SPIF_SENDCHANGE,
                );
            }

            unsafe extern "system" fn refit_zoomed_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
                if !IsWindowVisible(hwnd).as_bool() || !IsZoomed(hwnd).as_bool() {
                    return BOOL(1);
                }
                let mut cls = [0u16; 64];
                let len = GetClassNameW(hwnd, &mut cls);
                let cname = String::from_utf16_lossy(&cls[..len as usize]);
                if matches!(
                    cname.as_str(),
                    "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
                ) {
                    return BOOL(1);
                }
                let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                let mut m = MONITORINFO {
                    cbSize: size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                if GetMonitorInfoW(mon, &mut m).as_bool() {
                    let use_full = lparam.0 != 0;
                    let r = if use_full { m.rcMonitor } else { m.rcWork };
                    let _ = SetWindowPos(
                        hwnd,
                        None,
                        r.left,
                        r.top,
                        r.right - r.left,
                        r.bottom - r.top,
                        SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
                    );
                }
                BOOL(1)
            }
            let _ = EnumWindows(Some(refit_zoomed_proc), LPARAM(if hidden { 1 } else { 0 }));
        }
    }
}

#[cfg(not(windows))]
pub fn set_windows_taskbar_hidden(_hidden: bool) {}

pub fn restore_taskbar_if_hidden() {
    if TASKBAR_HIDDEN_BY_US.swap(false, std::sync::atomic::Ordering::SeqCst) {
        set_windows_taskbar_hidden(false);
    }
}

pub fn restore_taskbar_if_orphaned() {
    if taskbar_marker_path().exists() {
        set_windows_taskbar_hidden(false);
    }
}

#[cfg(windows)]
pub fn toggle_windows_hidden_tray() {
    use windows::core::w;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, IsWindowVisible, SetForegroundWindow, ShowWindow, SW_HIDE, SW_SHOW,
    };

    unsafe {
        if let Ok(overflow) = FindWindowW(w!("NotifyIconOverflowWindow"), None) {
            if !overflow.0.is_null() {
                if IsWindowVisible(overflow).as_bool() {
                    let _ = ShowWindow(overflow, SW_HIDE);
                } else {
                    let _ = ShowWindow(overflow, SW_SHOW);
                    let _ = SetForegroundWindow(overflow);
                }
                return;
            }
        }

        // Fall back to Win+B to focus the system notification area on Windows 10/11.
        const VK_LWIN: VIRTUAL_KEY = VIRTUAL_KEY(0x5B);
        const VK_B: VIRTUAL_KEY = VIRTUAL_KEY(0x42);
        let mk = |vk: VIRTUAL_KEY, up: bool| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let inputs = [
            mk(VK_LWIN, false),
            mk(VK_B, false),
            mk(VK_B, true),
            mk(VK_LWIN, true),
        ];
        let _ = SendInput(&inputs, size_of::<INPUT>() as i32);
    }
}

#[cfg(not(windows))]
pub fn toggle_windows_hidden_tray() {}

#[cfg(windows)]
pub fn trim_process_tree_memory() {
    use std::collections::HashSet;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::ProcessStatus::K32EmptyWorkingSet;
    use windows::Win32::System::Threading::{
        GetCurrentProcess, OpenProcess, SetProcessWorkingSetSize, PROCESS_QUERY_LIMITED_INFORMATION,
        PROCESS_SET_QUOTA,
    };

    unsafe {
        let me = GetCurrentProcess();
        let _ = SetProcessWorkingSetSize(me, usize::MAX, usize::MAX);
        let _ = K32EmptyWorkingSet(me);

        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return;
        };
        let mut pairs: Vec<(u32, u32)> = Vec::new();
        let mut pe = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snap, &mut pe).is_ok() {
            loop {
                pairs.push((pe.th32ProcessID, pe.th32ParentProcessID));
                if Process32NextW(snap, &mut pe).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);

        let mut tree = HashSet::new();
        tree.insert(std::process::id());
        loop {
            let before = tree.len();
            for &(pid, ppid) in &pairs {
                if tree.contains(&ppid) {
                    tree.insert(pid);
                }
            }
            if tree.len() == before {
                break;
            }
        }
        tree.remove(&std::process::id());

        for child_pid in tree {
            if let Ok(h) = OpenProcess(
                PROCESS_SET_QUOTA | PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                child_pid,
            ) {
                let _ = SetProcessWorkingSetSize(h, usize::MAX, usize::MAX);
                let _ = K32EmptyWorkingSet(h);
                let _ = CloseHandle(h);
            }
        }
    }
}

#[cfg(not(windows))]
pub fn trim_process_tree_memory() {}
