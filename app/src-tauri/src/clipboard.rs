use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardItem {
    pub id: String,
    pub kind: String, // "text" | "url"
    pub content: String,
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
            pinned: false,
            timestamp_ms: ts,
        },
    );
    // Keep pinned items + up to 50 recent unpinned items
    let mut unpinned_count = 0usize;
    items.retain(|x| {
        if x.pinned {
            true
        } else {
            unpinned_count += 1;
            unpinned_count <= 50
        }
    });
    save_history(items);
    true
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
