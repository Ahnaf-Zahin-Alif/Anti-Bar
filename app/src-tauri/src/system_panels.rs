use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaState {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub is_playing: bool,
    pub position_sec: f64,
    pub duration_sec: f64,
    pub has_session: bool,
    #[serde(default)]
    pub thumbnail_data_url: Option<String>,
}

impl Default for MediaState {
    fn default() -> Self {
        Self {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            is_playing: false,
            position_sec: 0.0,
            duration_sec: 0.0,
            has_session: false,
            thumbnail_data_url: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsNotificationItem {
    pub id: u32,
    pub app_name: String,
    pub title: String,
    pub body: String,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsSnapshot {
    pub status: String,
    pub items: Vec<WindowsNotificationItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub time_sec: f64,
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResult {
    pub found: bool,
    pub synced: Vec<LyricLine>,
    pub plain: String,
}

static THUMB_CACHE: std::sync::OnceLock<std::sync::Mutex<(String, Option<String>)>> =
    std::sync::OnceLock::new();

#[cfg(windows)]
fn read_thumbnail_data_url(
    thumb_ref: &windows::Storage::Streams::IRandomAccessStreamReference,
) -> Option<String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use windows::Storage::Streams::{DataReader, InputStreamOptions};

    let stream = thumb_ref.OpenReadAsync().ok()?.get().ok()?;
    let size = stream.Size().ok()?.min(512 * 1024) as u32;
    if size == 0 {
        return None;
    }
    let input = stream.GetInputStreamAt(0).ok()?;
    let reader = DataReader::CreateDataReader(&input).ok()?;
    reader.SetInputStreamOptions(InputStreamOptions::ReadAhead).ok()?;
    let loaded = reader.LoadAsync(size).ok()?.get().ok()?;
    if loaded == 0 {
        return None;
    }
    let mut buf = vec![0u8; loaded as usize];
    reader.ReadBytes(&mut buf).ok()?;
    let mime = if buf.starts_with(b"\x89PNG") {
        "image/png"
    } else {
        "image/jpeg"
    };
    Some(format!("data:{};base64,{}", mime, STANDARD.encode(&buf)))
}

#[cfg(windows)]
pub fn read_current_media() -> MediaState {
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus,
    };

    let Ok(op) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync() else {
        return MediaState::default();
    };
    let Ok(mgr) = op.get() else {
        return MediaState::default();
    };
    let Ok(session) = mgr.GetCurrentSession() else {
        return MediaState::default();
    };

    let mut state = MediaState {
        has_session: true,
        ..Default::default()
    };

    if let Ok(props_op) = session.TryGetMediaPropertiesAsync() {
        if let Ok(props) = props_op.get() {
            state.title = props.Title().map(|s| s.to_string()).unwrap_or_default();
            state.artist = props.Artist().map(|s| s.to_string()).unwrap_or_default();
            state.album = props.AlbumTitle().map(|s| s.to_string()).unwrap_or_default();
            let cache_key = format!("{}::{}::{}", state.title, state.artist, state.album);
            let cache = THUMB_CACHE.get_or_init(|| std::sync::Mutex::new((String::new(), None)));
            let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
            if guard.0 == cache_key {
                state.thumbnail_data_url = guard.1.clone();
            } else {
                let art = props
                    .Thumbnail()
                    .ok()
                    .and_then(|r| read_thumbnail_data_url(&r));
                *guard = (cache_key, art.clone());
                state.thumbnail_data_url = art;
            }
        }
    }
    if let Ok(pb) = session.GetPlaybackInfo() {
        if let Ok(status) = pb.PlaybackStatus() {
            state.is_playing =
                status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing;
        }
    }
    if let Ok(tl) = session.GetTimelineProperties() {
        if let Ok(pos) = tl.Position() {
            state.position_sec = (pos.Duration as f64) / 10_000_000.0;
        }
        if let Ok(end) = tl.EndTime() {
            state.duration_sec = (end.Duration as f64) / 10_000_000.0;
        }
    }
    state
}

#[cfg(not(windows))]
pub fn read_current_media() -> MediaState {
    MediaState::default()
}

pub fn clamp_seek_seconds(position_sec: f64, duration_sec: f64) -> f64 {
    if !position_sec.is_finite() || position_sec <= 0.0 {
        return 0.0;
    }
    if duration_sec.is_finite() && duration_sec > 0.0 {
        position_sec.min(duration_sec)
    } else {
        position_sec
    }
}

#[cfg(windows)]
pub fn seek_media_position(position_sec: f64) -> Result<(), String> {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;

    let mgr = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .map_err(|e| e.to_string())?
        .get()
        .map_err(|e| e.to_string())?;
    let session = mgr.GetCurrentSession().map_err(|e| e.to_string())?;
    let safe_pos = clamp_seek_seconds(position_sec, f64::INFINITY);
    let ticks = (safe_pos * 10_000_000.0) as i64;
    session
        .TryChangePlaybackPositionAsync(ticks)
        .map_err(|e| e.to_string())?
        .get()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(not(windows))]
pub fn seek_media_position(_position_sec: f64) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
pub fn media_control(action: &str) -> Result<(), String> {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;

    let mgr = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .map_err(|e| e.to_string())?
        .get()
        .map_err(|e| e.to_string())?;
    let session = mgr.GetCurrentSession().map_err(|e| e.to_string())?;

    match action {
        "toggle" => {
            session
                .TryTogglePlayPauseAsync()
                .map_err(|e| e.to_string())?
                .get()
                .map_err(|e| e.to_string())?;
        }
        "next" => {
            session
                .TrySkipNextAsync()
                .map_err(|e| e.to_string())?
                .get()
                .map_err(|e| e.to_string())?;
        }
        "prev" => {
            session
                .TrySkipPreviousAsync()
                .map_err(|e| e.to_string())?
                .get()
                .map_err(|e| e.to_string())?;
        }
        _ => return Err(format!("Unknown media action: {}", action)),
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn media_control(_action: &str) -> Result<(), String> {
    Ok(())
}

pub fn parse_lrc(lrc: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    for raw in lrc.lines() {
        let mut cursor = raw.trim();
        let mut timestamps = Vec::new();
        while cursor.starts_with('[') {
            let Some(end_bracket) = cursor.find(']') else {
                break;
            };
            let ts = &cursor[1..end_bracket];
            if let Some((min_s, sec_s)) = ts.split_once(':') {
                if let (Ok(mins), Ok(secs)) = (min_s.parse::<f64>(), sec_s.parse::<f64>()) {
                    if mins.is_finite() && mins >= 0.0 && secs.is_finite() && secs >= 0.0 {
                        timestamps.push(mins * 60.0 + secs);
                    }
                }
            }
            cursor = cursor[end_bracket + 1..].trim_start();
        }
        let text = cursor.trim();
        if timestamps.is_empty() || text.is_empty() {
            continue;
        }
        for time_sec in timestamps {
            lines.push(LyricLine {
                time_sec,
                text: text.to_string(),
            });
        }
    }
    lines.sort_by(|a, b| a.time_sec.total_cmp(&b.time_sec));
    lines
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

#[cfg(windows)]
pub fn winhttp_get(host: &str, path_and_query: &str, accept: &str) -> Result<String, String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Networking::WinHttp::{
        WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest,
        WinHttpQueryDataAvailable, WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse,
        WinHttpSendRequest, WinHttpSetTimeouts, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
        WINHTTP_FLAG_SECURE, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
    };

    if host.trim().is_empty() {
        return Err("Empty HTTP host".into());
    }

    unsafe {
        let ua = HSTRING::from("AntiBar/0.1.0");
        let session = WinHttpOpen(
            &ua,
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        );
        if session.is_null() {
            return Err("WinHttpOpen failed".into());
        }
        let _ = WinHttpSetTimeouts(session, 3000, 4000, 4000, 6000);

        let res = (|| -> Result<String, String> {
            let h_host = HSTRING::from(host);
            let conn = WinHttpConnect(session, &h_host, 443, 0);
            if conn.is_null() {
                return Err("WinHttpConnect failed".into());
            }
            let inner = (|| -> Result<String, String> {
                let verb = HSTRING::from("GET");
                let h_path = HSTRING::from(path_and_query);
                let req = WinHttpOpenRequest(
                    conn,
                    &verb,
                    &h_path,
                    PCWSTR::null(),
                    PCWSTR::null(),
                    std::ptr::null(),
                    WINHTTP_FLAG_SECURE,
                );
                if req.is_null() {
                    return Err("WinHttpOpenRequest failed".into());
                }
                let req_res = (|| -> Result<String, String> {
                    let headers = format!("Accept: {}\r\n", accept);
                    let h_headers = HSTRING::from(&headers);
                    WinHttpSendRequest(req, Some(h_headers.as_wide()), None, 0, 0, 0)
                        .map_err(|e| e.to_string())?;
                    WinHttpReceiveResponse(req, std::ptr::null_mut())
                        .map_err(|e| e.to_string())?;

                    let mut status_code: u32 = 0;
                    let mut status_len = std::mem::size_of::<u32>() as u32;
                    if WinHttpQueryHeaders(
                        req,
                        WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                        PCWSTR::null(),
                        Some(&mut status_code as *mut u32 as *mut _),
                        &mut status_len,
                        std::ptr::null_mut(),
                    )
                    .is_ok()
                        && status_code >= 400
                    {
                        return Err(format!("HTTP error status {}", status_code));
                    }

                    let mut body = Vec::new();
                    loop {
                        let mut avail: u32 = 0;
                        WinHttpQueryDataAvailable(req, &mut avail).map_err(|e| e.to_string())?;
                        if avail == 0 {
                            break;
                        }
                        let mut chunk = vec![0u8; avail as usize];
                        let mut read: u32 = 0;
                        WinHttpReadData(req, chunk.as_mut_ptr() as *mut _, avail, &mut read)
                            .map_err(|e| e.to_string())?;
                        if read == 0 {
                            break;
                        }
                        body.extend_from_slice(&chunk[..read as usize]);
                        if body.len() > 2_000_000 {
                            break;
                        }
                    }
                    Ok(String::from_utf8_lossy(&body).to_string())
                })();
                let _ = WinHttpCloseHandle(req);
                req_res
            })();
            let _ = WinHttpCloseHandle(conn);
            inner
        })();
        let _ = WinHttpCloseHandle(session);
        res
    }
}

#[cfg(not(windows))]
pub fn winhttp_get(_host: &str, _path_and_query: &str, _accept: &str) -> Result<String, String> {
    Err("Not on Windows".into())
}

pub fn fetch_synced_lyrics(title: &str, artist: &str, album: &str, duration_sec: f64) -> LyricsResult {
    if title.trim().is_empty() {
        return LyricsResult {
            found: false,
            synced: Vec::new(),
            plain: String::new(),
        };
    }
    let mut query = format!(
        "/api/get?track_name={}&artist_name={}&album_name={}",
        percent_encode(title),
        percent_encode(artist),
        percent_encode(album)
    );
    if duration_sec.is_finite() && duration_sec > 0.0 {
        query.push_str(&format!("&duration={}", duration_sec.round() as i64));
    }
    let Ok(body) = winhttp_get("lrclib.net", &query, "application/json") else {
        return LyricsResult {
            found: false,
            synced: Vec::new(),
            plain: String::new(),
        };
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&body) else {
        return LyricsResult {
            found: false,
            synced: Vec::new(),
            plain: String::new(),
        };
    };
    let synced_raw = val
        .get("syncedLyrics")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let plain = val
        .get("plainLyrics")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let synced = parse_lrc(synced_raw);
    LyricsResult {
        found: !synced.is_empty() || !plain.is_empty(),
        synced,
        plain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lrc_parsing_works() {
        let raw = "[00:12.50] First line\n[01:02.00] Second line\ninvalid";
        let lines = parse_lrc(raw);
        assert_eq!(lines.len(), 2);
        assert!((lines[0].time_sec - 12.5).abs() < 0.01);
        assert_eq!(lines[0].text, "First line");
        assert!((lines[1].time_sec - 62.0).abs() < 0.01);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WifiNetwork {
    pub ssid: String,
    pub signal_quality: u32,
    pub secured: bool,
    pub connected: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatus {
    pub battery_percent: u8,
    pub ac_online: bool,
    pub has_battery: bool,
}

pub fn dedup_sort_wifi(nets: Vec<WifiNetwork>) -> Vec<WifiNetwork> {
    let mut map = std::collections::HashMap::<String, WifiNetwork>::new();
    for n in nets {
        if n.ssid.trim().is_empty() {
            continue;
        }
        map.entry(n.ssid.clone())
            .and_modify(|existing| {
                if n.connected || (!existing.connected && n.signal_quality > existing.signal_quality) {
                    *existing = n.clone();
                }
            })
            .or_insert(n);
    }
    let mut list: Vec<WifiNetwork> = map.into_values().collect();
    list.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then_with(|| b.signal_quality.cmp(&a.signal_quality))
            .then_with(|| a.ssid.cmp(&b.ssid))
    });
    list
}

#[cfg(windows)]
pub fn list_wifi_networks() -> Vec<WifiNetwork> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::NetworkManagement::WiFi::{
        WlanCloseHandle, WlanEnumInterfaces, WlanFreeMemory, WlanGetAvailableNetworkList,
        WlanOpenHandle, WLAN_AVAILABLE_NETWORK_CONNECTED,
    };

    let mut out = Vec::new();
    unsafe {
        let mut neg_ver: u32 = 0;
        let mut handle = HANDLE::default();
        if WlanOpenHandle(2, None, &mut neg_ver, &mut handle) != 0 {
            return out;
        }
        let mut iface_list_ptr = std::ptr::null_mut();
        if WlanEnumInterfaces(handle, None, &mut iface_list_ptr) == 0 && !iface_list_ptr.is_null() {
            let iface_list = &*iface_list_ptr;
            let count = iface_list.dwNumberOfItems as usize;
            let ifaces = std::slice::from_raw_parts(iface_list.InterfaceInfo.as_ptr(), count);
            for iface in ifaces {
                let mut net_list_ptr = std::ptr::null_mut();
                if WlanGetAvailableNetworkList(
                    handle,
                    &iface.InterfaceGuid,
                    0,
                    None,
                    &mut net_list_ptr,
                ) == 0
                    && !net_list_ptr.is_null()
                {
                    let net_list = &*net_list_ptr;
                    let ncount = net_list.dwNumberOfItems as usize;
                    let nets = std::slice::from_raw_parts(net_list.Network.as_ptr(), ncount);
                    for n in nets {
                        let len = (n.dot11Ssid.uSSIDLength as usize).min(32);
                        let ssid = String::from_utf8_lossy(&n.dot11Ssid.ucSSID[..len]).to_string();
                        out.push(WifiNetwork {
                            ssid,
                            signal_quality: n.wlanSignalQuality,
                            secured: n.bSecurityEnabled.as_bool(),
                            connected: (n.dwFlags & WLAN_AVAILABLE_NETWORK_CONNECTED) != 0,
                        });
                    }
                    WlanFreeMemory(net_list_ptr as *mut _);
                }
            }
            WlanFreeMemory(iface_list_ptr as *mut _);
        }
        let _ = WlanCloseHandle(handle, None);
    }
    dedup_sort_wifi(out)
}

#[cfg(not(windows))]
pub fn list_wifi_networks() -> Vec<WifiNetwork> {
    Vec::new()
}

#[cfg(windows)]
pub fn get_system_status() -> SystemStatus {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    unsafe {
        let mut sps = SYSTEM_POWER_STATUS::default();
        if GetSystemPowerStatus(&mut sps).is_ok() {
            let has_battery = (sps.BatteryFlag & 128) == 0;
            let pct = if sps.BatteryLifePercent <= 100 {
                sps.BatteryLifePercent
            } else {
                100
            };
            return SystemStatus {
                battery_percent: pct,
                ac_online: sps.ACLineStatus == 1,
                has_battery,
            };
        }
    }
    SystemStatus {
        battery_percent: 100,
        ac_online: true,
        has_battery: false,
    }
}

#[cfg(not(windows))]
pub fn get_system_status() -> SystemStatus {
    SystemStatus {
        battery_percent: 100,
        ac_online: true,
        has_battery: false,
    }
}

pub fn validate_power_action(action: &str) -> bool {
    matches!(
        action,
        "lock" | "sleep" | "restart" | "shutdown" | "bluetooth" | "wifi"
    )
}

#[cfg(windows)]
pub fn execute_power_action(action: &str) -> Result<(), String> {
    if !validate_power_action(action) {
        return Err(format!("Unsupported power action: {}", action));
    }
    match action {
        "bluetooth" => crate::dock_win::launch_path("ms-settings:bluetooth", None),
        "wifi" => crate::dock_win::launch_path("ms-settings:network-wifi", None),
        "lock" => crate::dock_win::launch_path("rundll32.exe", Some("user32.dll,LockWorkStation")),
        "sleep" => crate::dock_win::launch_path("rundll32.exe", Some("powrprof.dll,SetSuspendState 0,1,0")),
        "restart" => crate::dock_win::launch_path("shutdown.exe", Some("/r /t 0")),
        "shutdown" => crate::dock_win::launch_path("shutdown.exe", Some("/s /t 0")),
        _ => Err("Unknown action".into()),
    }
}

pub fn filetime_ticks_to_unix_ms(universal_time_100ns: i64) -> i64 {
    const EPOCH_DIFF_MS: i64 = 11_644_473_600_000;
    let ms = universal_time_100ns / 10_000;
    if ms <= EPOCH_DIFF_MS {
        0
    } else {
        ms - EPOCH_DIFF_MS
    }
}

#[cfg(windows)]
pub fn list_notifications(request_if_needed: bool) -> NotificationsSnapshot {
    use windows::UI::Notifications::Management::{
        UserNotificationListener, UserNotificationListenerAccessStatus,
    };
    use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds};

    let Ok(listener) = UserNotificationListener::Current() else {
        return NotificationsSnapshot {
            status: "unsupported".into(),
            items: Vec::new(),
        };
    };

    let mut status = listener
        .GetAccessStatus()
        .unwrap_or(UserNotificationListenerAccessStatus::Unspecified);
    if status == UserNotificationListenerAccessStatus::Unspecified && request_if_needed {
        if let Ok(op) = listener.RequestAccessAsync() {
            if let Ok(next) = op.get() {
                status = next;
            }
        }
    }

    let status_str = match status {
        UserNotificationListenerAccessStatus::Allowed => "allowed",
        UserNotificationListenerAccessStatus::Denied => "denied",
        UserNotificationListenerAccessStatus::Unspecified => "unspecified",
        _ => "unsupported",
    };

    if status != UserNotificationListenerAccessStatus::Allowed {
        return NotificationsSnapshot {
            status: status_str.into(),
            items: Vec::new(),
        };
    }

    let mut items = Vec::new();
    if let Ok(op) = listener.GetNotificationsAsync(NotificationKinds::Toast) {
        if let Ok(list) = op.get() {
            let toast_binding = KnownNotificationBindings::ToastGeneric().ok();
            let count = list.Size().unwrap_or(0);
            for i in 0..count {
                let Ok(n) = list.GetAt(i) else {
                    continue;
                };
                let id = n.Id().unwrap_or(0);
                let app_name = n
                    .AppInfo()
                    .ok()
                    .and_then(|ai| ai.DisplayInfo().ok())
                    .and_then(|di| di.DisplayName().ok())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "Windows".into());

                let mut title = String::new();
                let mut body_parts = Vec::new();
                if let (Ok(notif), Some(ref tb)) = (n.Notification(), &toast_binding) {
                    if let Ok(vis) = notif.Visual() {
                        if let Ok(binding) = vis.GetBinding(tb) {
                            if let Ok(texts) = binding.GetTextElements() {
                                let tcount = texts.Size().unwrap_or(0);
                                for ti in 0..tcount {
                                    if let Ok(el) = texts.GetAt(ti) {
                                        if let Ok(txt) = el.Text() {
                                            let s = txt.to_string();
                                            if !s.trim().is_empty() {
                                                if ti == 0 {
                                                    title = s;
                                                } else {
                                                    body_parts.push(s);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Windows DateTime UniversalTime = 100ns ticks since 1601-01-01
                let timestamp_ms = n
                    .CreationTime()
                    .map(|dt| filetime_ticks_to_unix_ms(dt.UniversalTime))
                    .unwrap_or(0);

                items.push(WindowsNotificationItem {
                    id,
                    app_name,
                    title: if title.is_empty() {
                        "Notification".into()
                    } else {
                        title
                    },
                    body: body_parts.join(" · "),
                    timestamp_ms,
                });
            }
        }
    }
    items.sort_by_key(|a| std::cmp::Reverse(a.timestamp_ms));
    items.truncate(24);
    NotificationsSnapshot {
        status: "allowed".into(),
        items,
    }
}

#[cfg(not(windows))]
pub fn list_notifications(_request_if_needed: bool) -> NotificationsSnapshot {
    NotificationsSnapshot {
        status: "unsupported".into(),
        items: Vec::new(),
    }
}

#[cfg(windows)]
pub fn dismiss_notification(id: u32) -> Result<(), String> {
    use windows::UI::Notifications::Management::UserNotificationListener;
    let listener = UserNotificationListener::Current().map_err(|e| e.to_string())?;
    listener.RemoveNotification(id).map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn dismiss_notification(_id: u32) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
pub fn clear_all_notifications() -> Result<(), String> {
    use windows::UI::Notifications::Management::UserNotificationListener;
    let listener = UserNotificationListener::Current().map_err(|e| e.to_string())?;
    listener.ClearNotifications().map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn clear_all_notifications() -> Result<(), String> {
    Ok(())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BluetoothStatus {
    pub available: bool,
    pub enabled: bool,
}

#[cfg(windows)]
pub fn get_bluetooth_status() -> BluetoothStatus {
    use windows::Devices::Radios::{Radio, RadioKind, RadioState};

    let Ok(op) = Radio::GetRadiosAsync() else {
        return BluetoothStatus::default();
    };
    let Ok(radios) = op.get() else {
        return BluetoothStatus::default();
    };
    let count = radios.Size().unwrap_or(0);
    for i in 0..count {
        if let Ok(r) = radios.GetAt(i) {
            if r.Kind().ok() == Some(RadioKind::Bluetooth) {
                let enabled = r.State().ok() == Some(RadioState::On);
                return BluetoothStatus {
                    available: true,
                    enabled,
                };
            }
        }
    }
    BluetoothStatus::default()
}

#[cfg(not(windows))]
pub fn get_bluetooth_status() -> BluetoothStatus {
    BluetoothStatus::default()
}

#[cfg(windows)]
pub fn set_bluetooth_enabled(enabled: bool) -> Result<(), String> {
    use windows::Devices::Radios::{Radio, RadioKind, RadioState};

    let radios = Radio::GetRadiosAsync()
        .map_err(|e| e.to_string())?
        .get()
        .map_err(|e| e.to_string())?;
    let count = radios.Size().unwrap_or(0);
    let target = if enabled {
        RadioState::On
    } else {
        RadioState::Off
    };
    for i in 0..count {
        if let Ok(r) = radios.GetAt(i) {
            if r.Kind().ok() == Some(RadioKind::Bluetooth) {
                r.SetStateAsync(target)
                    .map_err(|e| e.to_string())?
                    .get()
                    .map_err(|e| e.to_string())?;
                return Ok(());
            }
        }
    }
    Err("No Bluetooth radio found".into())
}

#[cfg(not(windows))]
pub fn set_bluetooth_enabled(_enabled: bool) -> Result<(), String> {
    Err("Bluetooth control is only supported on Windows".into())
}

#[cfg(windows)]
pub fn connect_wifi(ssid: &str) -> Result<(), String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::NetworkManagement::WiFi::{
        dot11_BSS_type_any, wlan_connection_mode_profile, WlanCloseHandle, WlanConnect,
        WlanEnumInterfaces, WlanFreeMemory, WlanOpenHandle, WLAN_CONNECTION_PARAMETERS,
    };

    let clean = ssid.trim();
    if clean.is_empty() {
        return Err("Empty SSID".into());
    }
    let profile = HSTRING::from(clean);
    unsafe {
        let mut neg_ver = 0u32;
        let mut handle = HANDLE::default();
        let rc = WlanOpenHandle(2, None, &mut neg_ver, &mut handle);
        if rc != 0 {
            return Err(format!("WlanOpenHandle failed ({})", rc));
        }
        let mut iface_list_ptr = std::ptr::null_mut();
        let mut res = Err("No Wi-Fi interface found".to_string());
        if WlanEnumInterfaces(handle, None, &mut iface_list_ptr) == 0 && !iface_list_ptr.is_null() {
            let iface_list = &*iface_list_ptr;
            if iface_list.dwNumberOfItems > 0 {
                let ifaces = std::slice::from_raw_parts(
                    iface_list.InterfaceInfo.as_ptr(),
                    iface_list.dwNumberOfItems as usize,
                );
                let params = WLAN_CONNECTION_PARAMETERS {
                    wlanConnectionMode: wlan_connection_mode_profile,
                    strProfile: PCWSTR(profile.as_ptr()),
                    pDot11Ssid: std::ptr::null_mut(),
                    pDesiredBssidList: std::ptr::null_mut(),
                    dot11BssType: dot11_BSS_type_any,
                    dwFlags: 0,
                };
                let c_rc = WlanConnect(handle, &ifaces[0].InterfaceGuid, &params, None);
                if c_rc == 0 {
                    res = Ok(());
                } else {
                    res = Err(format!("WlanConnect failed ({})", c_rc));
                }
            }
            WlanFreeMemory(iface_list_ptr as *mut _);
        }
        let _ = WlanCloseHandle(handle, None);
        res
    }
}

#[cfg(not(windows))]
pub fn connect_wifi(_ssid: &str) -> Result<(), String> {
    Err("Wi-Fi control is only supported on Windows".into())
}

#[cfg(windows)]
pub fn disconnect_wifi() -> Result<(), String> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::NetworkManagement::WiFi::{
        WlanCloseHandle, WlanDisconnect, WlanEnumInterfaces, WlanFreeMemory, WlanOpenHandle,
    };

    unsafe {
        let mut neg_ver = 0u32;
        let mut handle = HANDLE::default();
        let rc = WlanOpenHandle(2, None, &mut neg_ver, &mut handle);
        if rc != 0 {
            return Err(format!("WlanOpenHandle failed ({})", rc));
        }
        let mut iface_list_ptr = std::ptr::null_mut();
        let mut res = Err("No Wi-Fi interface found".to_string());
        if WlanEnumInterfaces(handle, None, &mut iface_list_ptr) == 0 && !iface_list_ptr.is_null() {
            let iface_list = &*iface_list_ptr;
            if iface_list.dwNumberOfItems > 0 {
                let ifaces = std::slice::from_raw_parts(
                    iface_list.InterfaceInfo.as_ptr(),
                    iface_list.dwNumberOfItems as usize,
                );
                let d_rc = WlanDisconnect(handle, &ifaces[0].InterfaceGuid, None);
                if d_rc == 0 {
                    res = Ok(());
                } else {
                    res = Err(format!("WlanDisconnect failed ({})", d_rc));
                }
            }
            WlanFreeMemory(iface_list_ptr as *mut _);
        }
        let _ = WlanCloseHandle(handle, None);
        res
    }
}

#[cfg(not(windows))]
pub fn disconnect_wifi() -> Result<(), String> {
    Err("Wi-Fi control is only supported on Windows".into())
}

#[cfg(windows)]
pub fn get_brightness() -> Option<u32> {
    use windows::Win32::Devices::Display::{
        DestroyPhysicalMonitors, GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR,
        GetPhysicalMonitorsFromHMONITOR, PHYSICAL_MONITOR,
    };
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTOPRIMARY};

    unsafe {
        let mon = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut count = 0u32;
        if GetNumberOfPhysicalMonitorsFromHMONITOR(mon, &mut count).is_err() || count == 0 {
            return None;
        }
        let mut pms = vec![PHYSICAL_MONITOR::default(); count as usize];
        if GetPhysicalMonitorsFromHMONITOR(mon, &mut pms).is_err() {
            return None;
        }
        let mut min_b = 0u32;
        let mut cur_b = 0u32;
        let mut max_b = 0u32;
        let ok = GetMonitorBrightness(pms[0].hPhysicalMonitor, &mut min_b, &mut cur_b, &mut max_b) != 0;
        let _ = DestroyPhysicalMonitors(&pms);
        if ok && max_b > min_b {
            Some((((cur_b - min_b) * 100) / (max_b - min_b)).min(100))
        } else {
            None
        }
    }
}

#[cfg(not(windows))]
pub fn get_brightness() -> Option<u32> {
    None
}

#[cfg(windows)]
pub fn set_brightness(percent: u32) -> Result<(), String> {
    use windows::Win32::Devices::Display::{
        DestroyPhysicalMonitors, GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR,
        GetPhysicalMonitorsFromHMONITOR, SetMonitorBrightness, PHYSICAL_MONITOR,
    };
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTOPRIMARY};

    let pct = percent.min(100);
    unsafe {
        let mon = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut count = 0u32;
        GetNumberOfPhysicalMonitorsFromHMONITOR(mon, &mut count).map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("No physical monitors found".into());
        }
        let mut pms = vec![PHYSICAL_MONITOR::default(); count as usize];
        GetPhysicalMonitorsFromHMONITOR(mon, &mut pms).map_err(|e| e.to_string())?;
        let mut min_b = 0u32;
        let mut cur_b = 0u32;
        let mut max_b = 100u32;
        let _ = GetMonitorBrightness(pms[0].hPhysicalMonitor, &mut min_b, &mut cur_b, &mut max_b);
        let target = if max_b > min_b {
            min_b + ((max_b - min_b) * pct) / 100
        } else {
            pct
        };
        let ok = SetMonitorBrightness(pms[0].hPhysicalMonitor, target) != 0;
        let _ = DestroyPhysicalMonitors(&pms);
        if ok {
            Ok(())
        } else {
            Err("Monitor does not support DDC/CI brightness".into())
        }
    }
}

#[cfg(not(windows))]
pub fn set_brightness(_percent: u32) -> Result<(), String> {
    Err("Brightness control is only supported on Windows".into())
}
