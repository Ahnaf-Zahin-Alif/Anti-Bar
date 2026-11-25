use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSessionInfo {
    pub pid: u32,
    pub name: String,
    pub volume: f32,
    pub muted: bool,
    pub is_master: bool,
}

pub fn clamp_volume(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

#[cfg(windows)]
fn resolve_process_name(pid: u32) -> String {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    if pid == 0 {
        return "System Sounds".into();
    }
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return format!("Process {}", pid);
        };
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let name = if QueryFullProcessImageNameW(
            h,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .is_ok()
        {
            let full = String::from_utf16_lossy(&buf[..len as usize]);
            Path::new(&full)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("App")
                .to_string()
        } else {
            format!("Process {}", pid)
        };
        let _ = CloseHandle(h);
        name
    }
}

#[cfg(windows)]
pub fn list_audio_sessions() -> Vec<AudioSessionInfo> {
    use windows::core::Interface;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eMultimedia, eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
        ISimpleAudioVolume, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    let mut sessions = Vec::new();
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let res = (|| -> Result<(), windows::core::Error> {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;

            if let Ok(ep_vol) = device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
                let vol = ep_vol.GetMasterVolumeLevelScalar().unwrap_or(0.5);
                let muted = ep_vol.GetMute().map(|b| b.as_bool()).unwrap_or(false);
                sessions.push(AudioSessionInfo {
                    pid: 0,
                    name: "Master Output".into(),
                    volume: clamp_volume(vol),
                    muted,
                    is_master: true,
                });
            }

            let mgr: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
            let enum_sess = mgr.GetSessionEnumerator()?;
            let count = enum_sess.GetCount()?;
            for i in 0..count {
                let Ok(ctrl) = enum_sess.GetSession(i) else {
                    continue;
                };
                let Ok(ctrl2) = ctrl.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                let pid = ctrl2.GetProcessId().unwrap_or(0);
                if pid == 0 {
                    continue;
                }
                let Ok(simple_vol) = ctrl.cast::<ISimpleAudioVolume>() else {
                    continue;
                };
                let vol = simple_vol.GetMasterVolume().unwrap_or(1.0);
                let muted = simple_vol.GetMute().map(|b| b.as_bool()).unwrap_or(false);
                let disp = ctrl
                    .GetDisplayName()
                    .ok()
                    .and_then(|p| {
                        let s = p.to_string().ok()?;
                        windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as *const _));
                        if s.is_empty() || s.starts_with('@') {
                            None
                        } else {
                            Some(s)
                        }
                    })
                    .unwrap_or_else(|| resolve_process_name(pid));

                if !sessions.iter().any(|s| !s.is_master && s.pid == pid) {
                    sessions.push(AudioSessionInfo {
                        pid,
                        name: disp,
                        volume: clamp_volume(vol),
                        muted,
                        is_master: false,
                    });
                }
            }
            Ok(())
        })();
        let _ = res;
        CoUninitialize();
    }
    sessions
}

#[cfg(not(windows))]
pub fn list_audio_sessions() -> Vec<AudioSessionInfo> {
    Vec::new()
}

#[cfg(windows)]
pub fn set_session_volume(pid: u32, is_master: bool, volume: f32, muted: bool) -> Result<(), String> {
    use windows::core::Interface;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eMultimedia, eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
        ISimpleAudioVolume, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    let vol = clamp_volume(volume);
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let res = (|| -> Result<(), String> {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
            let device = enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
                .map_err(|e| e.to_string())?;

            if is_master {
                let ep_vol: IAudioEndpointVolume =
                    device.Activate(CLSCTX_ALL, None).map_err(|e| e.to_string())?;
                ep_vol
                    .SetMasterVolumeLevelScalar(vol, std::ptr::null())
                    .map_err(|e| e.to_string())?;
                ep_vol
                    .SetMute(muted, std::ptr::null())
                    .map_err(|e| e.to_string())?;
                return Ok(());
            }

            let mgr: IAudioSessionManager2 =
                device.Activate(CLSCTX_ALL, None).map_err(|e| e.to_string())?;
            let enum_sess = mgr.GetSessionEnumerator().map_err(|e| e.to_string())?;
            let count = enum_sess.GetCount().map_err(|e| e.to_string())?;
            for i in 0..count {
                let Ok(ctrl) = enum_sess.GetSession(i) else {
                    continue;
                };
                let Ok(ctrl2) = ctrl.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                if ctrl2.GetProcessId().unwrap_or(0) == pid {
                    if let Ok(sv) = ctrl.cast::<ISimpleAudioVolume>() {
                        sv.SetMasterVolume(vol, std::ptr::null())
                            .map_err(|e| e.to_string())?;
                        sv.SetMute(muted, std::ptr::null())
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
            Ok(())
        })();
        CoUninitialize();
        res
    }
}

#[cfg(not(windows))]
pub fn set_session_volume(_pid: u32, _is_master: bool, _volume: f32, _muted: bool) -> Result<(), String> {
    Ok(())
}
