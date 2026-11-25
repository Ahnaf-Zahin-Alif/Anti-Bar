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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioOutputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
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

#[cfg(windows)]
pub fn list_output_devices() -> Vec<AudioOutputDevice> {
    use windows::core::GUID;
    use windows::Win32::Media::Audio::{
        eMultimedia, eRender, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::StructuredStorage::PropVariantClear;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
        COINIT_MULTITHREADED, STGM_READ,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY;

    // PKEY_Device_FriendlyName = {a45c254e-df1c-4efd-8020-67d146a850e0}, pid 14
    const PKEY_DEVICE_FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY {
        fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
        pid: 14,
    };

    let mut out = Vec::new();
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let _ = (|| -> Result<(), windows::core::Error> {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let default_id = enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
                .ok()
                .and_then(|d| {
                    let p = d.GetId().ok()?;
                    let s = p.to_string().ok();
                    CoTaskMemFree(Some(p.0 as *const _));
                    s
                })
                .unwrap_or_default();

            let col = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
            let count = col.GetCount()?;
            for i in 0..count {
                let Ok(dev) = col.Item(i) else {
                    continue;
                };
                let Ok(id_pw) = dev.GetId() else {
                    continue;
                };
                let id = id_pw.to_string().unwrap_or_default();
                CoTaskMemFree(Some(id_pw.0 as *const _));
                if id.is_empty() {
                    continue;
                }

                let name = dev
                    .OpenPropertyStore(STGM_READ)
                    .ok()
                    .and_then(|store| {
                        let mut pv = store.GetValue(&PKEY_DEVICE_FRIENDLY_NAME).ok()?;
                        let text = pv.to_string();
                        let _ = PropVariantClear(&mut pv);
                        if text.is_empty() {
                            None
                        } else {
                            Some(text)
                        }
                    })
                    .unwrap_or_else(|| format!("Audio Output {}", i + 1));

                out.push(AudioOutputDevice {
                    is_default: id == default_id,
                    id,
                    name,
                });
            }
            Ok(())
        })();
        CoUninitialize();
    }
    out
}

#[cfg(not(windows))]
pub fn list_output_devices() -> Vec<AudioOutputDevice> {
    Vec::new()
}

#[cfg(windows)]
pub fn set_default_output_device(device_id: &str) -> Result<(), String> {
    use std::ffi::c_void;
    use windows::core::{GUID, HRESULT, HSTRING, PCWSTR};
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    const CLSID_POLICY_CONFIG_CLIENT: GUID =
        GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
    const IID_POLICY_CONFIG: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);

    #[repr(C)]
    struct PolicyConfigVtbl {
        // IUnknown
        query_interface: usize,
        add_ref: usize,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        // IPolicyConfig (indices 3..12)
        _slots: [usize; 10],
        // index 13: SetDefaultEndpoint(PCWSTR wszDeviceId, ERole role)
        set_default_endpoint:
            unsafe extern "system" fn(*mut c_void, PCWSTR, u32) -> HRESULT,
    }

    #[repr(C)]
    struct PolicyConfigObj {
        vtbl: *const PolicyConfigVtbl,
    }

    let wide = HSTRING::from(device_id);
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let res = (|| -> Result<(), String> {
            let mut ptr: *mut PolicyConfigObj = std::ptr::null_mut();
            let hr = windows::Win32::System::Com::CoCreateInstance::<_, windows::core::IUnknown>(
                &CLSID_POLICY_CONFIG_CLIENT,
                None,
                CLSCTX_ALL,
            );
            let unk = hr.map_err(|e| format!("PolicyConfig init failed: {}", e))?;
            use windows::core::Interface;
            let qhr = unk.query(&IID_POLICY_CONFIG, &mut ptr as *mut _ as *mut *mut c_void);
            if qhr.is_err() || ptr.is_null() {
                return Err("IPolicyConfig interface unavailable on this Windows build".into());
            }
            let vtbl = &*(*ptr).vtbl;
            for role in [0u32, 1u32, 2u32] {
                let s = (vtbl.set_default_endpoint)(
                    ptr as *mut c_void,
                    PCWSTR(wide.as_ptr()),
                    role,
                );
                if s.is_err() {
                    (vtbl.release)(ptr as *mut c_void);
                    return Err(format!("SetDefaultEndpoint failed: {:?}", s));
                }
            }
            (vtbl.release)(ptr as *mut c_void);
            Ok(())
        })();
        CoUninitialize();
        res
    }
}

#[cfg(not(windows))]
pub fn set_default_output_device(_device_id: &str) -> Result<(), String> {
    Ok(())
}
