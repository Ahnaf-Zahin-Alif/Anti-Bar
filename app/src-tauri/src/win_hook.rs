use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};

static HOOK_ENABLED: AtomicBool = AtomicBool::new(false);
static HOOK_STARTED: AtomicBool = AtomicBool::new(false);

pub fn set_win_key_hook_enabled(app: &AppHandle, enabled: bool) {
    HOOK_ENABLED.store(enabled, Ordering::SeqCst);
    if enabled && !HOOK_STARTED.swap(true, Ordering::SeqCst) {
        #[cfg(windows)]
        spawn_hook_thread(app.clone());
        #[cfg(not(windows))]
        let _ = app;
    }
}

#[cfg(windows)]
fn spawn_hook_thread(app: AppHandle) {
    use std::sync::OnceLock;
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
        UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
        WM_SYSKEYDOWN,
    };

    static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
    static WIN_DOWN: AtomicBool = AtomicBool::new(false);
    static COMBO_USED: AtomicBool = AtomicBool::new(false);
    let _ = APP_HANDLE.set(app);

    unsafe extern "system" fn ll_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 && HOOK_ENABLED.load(Ordering::Relaxed) {
            let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let vk = kb.vkCode;
            const VK_LWIN: u32 = 0x5B;
            const VK_RWIN: u32 = 0x5C;
            let msg = wparam.0 as u32;

            if vk == VK_LWIN || vk == VK_RWIN {
                if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                    WIN_DOWN.store(true, Ordering::SeqCst);
                    COMBO_USED.store(false, Ordering::SeqCst);
                } else if msg == WM_KEYUP {
                    let was_down = WIN_DOWN.swap(false, Ordering::SeqCst);
                    let used_combo = COMBO_USED.swap(false, Ordering::SeqCst);
                    if was_down && !used_combo {
                        if let Some(h) = APP_HANDLE.get() {
                            let _ = h.emit("antibar://win-key-tapped", ());
                            crate::toggle_spotlight(h.clone());
                        }
                        return LRESULT(1);
                    }
                }
            } else if WIN_DOWN.load(Ordering::Relaxed) {
                COMBO_USED.store(true, Ordering::SeqCst);
            }
        }
        CallNextHookEx(None, code, wparam, lparam)
    }

    std::thread::spawn(move || unsafe {
        let Ok(hook) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_proc), None, 0) else {
            return;
        };
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = UnhookWindowsHookEx(hook);
    });
}
