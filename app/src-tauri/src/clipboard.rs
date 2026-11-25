use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardItem {
    pub id: String,
    pub kind: String, // "text" | "url" | "image"
    pub content: String,
    #[serde(default)]
    pub image_data_url: Option<String>,
    pub pinned: bool,
    pub timestamp_ms: u64,
}

fn history_path() -> PathBuf {
    crate::config::app_data_dir().join("clipboard-history.json")
}

pub fn load_history() -> Vec<ClipboardItem> {
    match fs::read_to_string(history_path()) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save_history(items: &[ClipboardItem]) {
    if let Ok(json) = serde_json::to_string_pretty(items) {
        let _ = fs::write(history_path(), json);
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn push_clipboard_text(items: &mut Vec<ClipboardItem>, text: String) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    if items.first().map(|x| x.content == text).unwrap_or(false) {
        return false;
    }
    let kind = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        "url"
    } else {
        "text"
    };
    items.retain(|x| x.pinned || x.content != text);
    let ts = now_ms();
    items.insert(
        0,
        ClipboardItem {
            id: format!("clip-{}", ts),
            kind: kind.into(),
            content: text,
            image_data_url: None,
            pinned: false,
            timestamp_ms: ts,
        },
    );
    prune_and_save(items);
    true
}

pub fn push_clipboard_image(items: &mut Vec<ClipboardItem>, data_url: String) -> bool {
    if data_url.is_empty() {
        return false;
    }
    if items
        .first()
        .and_then(|x| x.image_data_url.as_ref())
        .map(|d| d == &data_url)
        .unwrap_or(false)
    {
        return false;
    }
    items.retain(|x| x.pinned || x.image_data_url.as_deref() != Some(data_url.as_str()));
    let ts = now_ms();
    items.insert(
        0,
        ClipboardItem {
            id: format!("clip-img-{}", ts),
            kind: "image".into(),
            content: "Screenshot / Image".into(),
            image_data_url: Some(data_url),
            pinned: false,
            timestamp_ms: ts,
        },
    );
    prune_and_save(items);
    true
}

fn prune_and_save(items: &mut Vec<ClipboardItem>) {
    let mut unpinned_count = 0usize;
    let mut unpinned_images = 0usize;
    items.retain(|x| {
        if x.pinned {
            return true;
        }
        if x.kind == "image" {
            unpinned_images += 1;
            if unpinned_images > 4 {
                return false;
            }
        }
        unpinned_count += 1;
        unpinned_count <= 35
    });
    save_history(items);
}

#[cfg(windows)]
pub fn clipboard_sequence_number() -> u32 {
    use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;
    unsafe { GetClipboardSequenceNumber() }
}

#[cfg(not(windows))]
pub fn clipboard_sequence_number() -> u32 {
    0
}

#[cfg(windows)]
pub fn read_clipboard_unicode_text() -> Option<String> {
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};

    const CF_UNICODETEXT: u32 = 13;
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT).is_err() {
            return None;
        }
        if OpenClipboard(None).is_err() {
            return None;
        }
        let result = (|| {
            let handle = GetClipboardData(CF_UNICODETEXT).ok()?;
            let hglobal = HGLOBAL(handle.0);
            let ptr = GlobalLock(hglobal) as *const u16;
            if ptr.is_null() {
                return None;
            }
            let mut len = 0usize;
            while *ptr.add(len) != 0 && len < 65536 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(ptr, len);
            let text = String::from_utf16_lossy(slice);
            let _ = GlobalUnlock(hglobal);
            Some(text)
        })();
        let _ = CloseClipboard();
        result
    }
}

#[cfg(windows)]
pub fn write_clipboard_unicode_text(text: &str) -> Result<(), String> {
    use windows::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    const CF_UNICODETEXT: u32 = 13;
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = wide.len() * 2;
    unsafe {
        OpenClipboard(None).map_err(|e| e.to_string())?;
        let res = (|| -> Result<(), String> {
            EmptyClipboard().map_err(|e| e.to_string())?;
            let hmem: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(|e| e.to_string())?;
            let ptr = GlobalLock(hmem) as *mut u16;
            if ptr.is_null() {
                return Err("GlobalLock returned null".into());
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
            let _ = GlobalUnlock(hmem);
            SetClipboardData(CF_UNICODETEXT, HANDLE(hmem.0)).map_err(|e| e.to_string())?;
            Ok(())
        })();
        let _ = CloseClipboard();
        res
    }
}

#[cfg(not(windows))]
pub fn read_clipboard_unicode_text() -> Option<String> {
    None
}

#[cfg(not(windows))]
pub fn write_clipboard_unicode_text(_text: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
pub fn read_clipboard_image_data_url() -> Option<String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use std::mem::size_of;
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::Graphics::Gdi::BITMAPINFOHEADER;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    const CF_DIB: u32 = 8;
    unsafe {
        if IsClipboardFormatAvailable(CF_DIB).is_err() {
            return None;
        }
        if OpenClipboard(None).is_err() {
            return None;
        }
        let res = (|| -> Option<String> {
            let handle = GetClipboardData(CF_DIB).ok()?;
            let hglobal = HGLOBAL(handle.0);
            let total_size = GlobalSize(hglobal);
            if total_size < size_of::<BITMAPINFOHEADER>() || total_size > 16 * 1024 * 1024 {
                return None;
            }
            let ptr = GlobalLock(hglobal) as *const u8;
            if ptr.is_null() {
                return None;
            }
            let result = (|| -> Option<String> {
                let bih = &*(ptr as *const BITMAPINFOHEADER);
                let bpp = bih.biBitCount;
                if bpp != 32 && bpp != 24 {
                    return None;
                }
                let w = bih.biWidth;
                let h_raw = bih.biHeight;
                if w <= 0 || h_raw == 0 || w > 2400 || h_raw.abs() > 2400 {
                    return None;
                }
                let h = h_raw.abs();
                let top_down = h_raw < 0;
                let header_sz = bih.biSize as usize;
                let mask_extra = if bih.biCompression == 3 && header_sz == 40 {
                    12usize
                } else {
                    0usize
                };
                let pixel_offset = header_sz + mask_extra;
                let stride = (((w as usize) * (bpp as usize) + 31) / 32) * 4;
                if pixel_offset + stride * (h as usize) > total_size {
                    return None;
                }
                let pixels = std::slice::from_raw_parts(ptr.add(pixel_offset), stride * (h as usize));
                let scale = ((w as f64) / 480.0).max(1.0);
                let out_w = ((w as f64 / scale).round() as usize).max(1);
                let out_h = ((h as f64 / scale).round() as usize).max(1);
                let mut rgba = vec![0u8; out_w * out_h * 4];
                let bytes_per_px = (bpp / 8) as usize;
                for row in 0..out_h {
                    let sy = ((row as f64 * scale) as usize).min((h as usize) - 1);
                    let src_row = if top_down { sy } else { (h as usize) - 1 - sy };
                    let row_slice = &pixels[src_row * stride..];
                    for col in 0..out_w {
                        let sx = ((col as f64 * scale) as usize).min((w as usize) - 1);
                        let s = sx * bytes_per_px;
                        let d = (row * out_w + col) * 4;
                        rgba[d] = row_slice[s + 2];
                        rgba[d + 1] = row_slice[s + 1];
                        rgba[d + 2] = row_slice[s];
                        rgba[d + 3] = if bpp == 32 && row_slice[s + 3] != 0 {
                            row_slice[s + 3]
                        } else {
                            255
                        };
                    }
                }
                let mut png_bytes = Vec::new();
                {
                    let mut encoder = png::Encoder::new(&mut png_bytes, out_w as u32, out_h as u32);
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    let mut writer = encoder.write_header().ok()?;
                    writer.write_image_data(&rgba).ok()?;
                }
                Some(format!("data:image/png;base64,{}", STANDARD.encode(&png_bytes)))
            })();
            let _ = GlobalUnlock(hglobal);
            result
        })();
        let _ = CloseClipboard();
        res
    }
}

#[cfg(not(windows))]
pub fn read_clipboard_image_data_url() -> Option<String> {
    None
}

#[cfg(windows)]
pub fn write_clipboard_png_data_url(data_url: &str) -> Result<(), String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use std::mem::size_of;
    use windows::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows::Win32::Graphics::Gdi::{BITMAPINFOHEADER, BI_RGB};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    let b64 = data_url
        .split_once("base64,")
        .map(|(_, s)| s)
        .unwrap_or(data_url);
    let png_bytes = STANDARD.decode(b64).map_err(|e| e.to_string())?;
    let decoder = png::Decoder::new(std::io::Cursor::new(&png_bytes));
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let raw = &buf[..info.buffer_size()];
    let w = info.width as i32;
    let h = info.height as i32;
    let total_bytes = size_of::<BITMAPINFOHEADER>() + (w as usize) * (h as usize) * 4;

    const CF_DIB: u32 = 8;
    unsafe {
        OpenClipboard(None).map_err(|e| e.to_string())?;
        let res = (|| -> Result<(), String> {
            EmptyClipboard().map_err(|e| e.to_string())?;
            let hmem: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, total_bytes).map_err(|e| e.to_string())?;
            let ptr = GlobalLock(hmem) as *mut u8;
            if ptr.is_null() {
                return Err("GlobalLock returned null".into());
            }
            let bih = BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            };
            std::ptr::copy_nonoverlapping(&bih as *const BITMAPINFOHEADER as *const u8, ptr, size_of::<BITMAPINFOHEADER>());
            let out_px = std::slice::from_raw_parts_mut(
                ptr.add(size_of::<BITMAPINFOHEADER>()),
                (w as usize) * (h as usize) * 4,
            );
            let ch = match info.color_type {
                png::ColorType::Rgba => 4,
                png::ColorType::Rgb => 3,
                _ => return Err("Unsupported PNG pixel layout".into()),
            };
            for i in 0..((w as usize) * (h as usize)) {
                let s = i * ch;
                let d = i * 4;
                out_px[d] = raw[s + 2];
                out_px[d + 1] = raw[s + 1];
                out_px[d + 2] = raw[s];
                out_px[d + 3] = if ch == 4 { raw[s + 3] } else { 255 };
            }
            let _ = GlobalUnlock(hmem);
            SetClipboardData(CF_DIB, HANDLE(hmem.0)).map_err(|e| e.to_string())?;
            Ok(())
        })();
        let _ = CloseClipboard();
        res
    }
}

#[cfg(not(windows))]
pub fn write_clipboard_png_data_url(_data_url: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_dedup_and_url_detection() {
        let mut items = Vec::new();
        assert!(push_clipboard_text(&mut items, "https://example.com".into()));
        assert_eq!(items[0].kind, "url");
        assert!(!push_clipboard_text(&mut items, "https://example.com".into()));
        assert!(push_clipboard_text(&mut items, "hello world".into()));
        assert_eq!(items[0].kind, "text");
        assert_eq!(items.len(), 2);
    }
}
