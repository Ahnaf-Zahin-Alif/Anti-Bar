/// Formats a list of file paths as quoted command-line arguments for `ShellExecuteW`.
pub fn format_open_files_args(file_paths: &[String]) -> String {
    file_paths
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| format!("\"{}\"", p.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Launches `app_path` with the provided `file_paths` as arguments.
pub fn open_files_with_app(app_path: &str, file_paths: &[String]) -> Result<(), String> {
    if file_paths.is_empty() {
        return Err("No files provided".into());
    }
    let args = format_open_files_args(file_paths);
    if args.is_empty() {
        return Err("No valid file paths".into());
    }
    crate::dock_win::launch_path(app_path, Some(&args))
}

#[cfg(windows)]
mod win_ole {
    use windows::core::{implement, HRESULT};
    use windows::Win32::Foundation::{BOOL, DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, S_OK};
    use windows::Win32::System::Ole::{IDropSource, IDropSource_Impl, DROPEFFECT};
    use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};

    #[implement(IDropSource)]
    pub struct SimpleDropSource;

    impl IDropSource_Impl for SimpleDropSource_Impl {
        fn QueryContinueDrag(
            &self,
            fescapepressed: BOOL,
            grfkeystate: MODIFIERKEYS_FLAGS,
        ) -> HRESULT {
            if fescapepressed.as_bool() {
                return DRAGDROP_S_CANCEL;
            }
            if (grfkeystate.0 & MK_LBUTTON.0) == 0 {
                return DRAGDROP_S_DROP;
            }
            S_OK
        }

        fn GiveFeedback(&self, _dweffect: DROPEFFECT) -> HRESULT {
            DRAGDROP_S_USEDEFAULTCURSORS
        }
    }
}

/// Initiates an OS-level OLE file drag-and-drop operation (`DoDragDrop`) for `paths`.
#[cfg(windows)]
pub fn start_file_drag(paths: Vec<String>) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Win32::System::Com::IDataObject;
    use windows::Win32::System::Ole::{
        DoDragDrop, IDropSource, OleInitialize, OleUninitialize, DROPEFFECT, DROPEFFECT_COPY,
        DROPEFFECT_LINK, DROPEFFECT_MOVE,
    };
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::Shell::{ILCreateFromPathW, ILFree, SHCreateDataObject};

    let valid: Vec<String> = paths
        .into_iter()
        .filter(|p| !p.trim().is_empty() && std::path::Path::new(p).exists())
        .collect();
    if valid.is_empty() {
        return Err("No existing files to drag".into());
    }

    unsafe {
        let ole_ok = OleInitialize(None).is_ok();
        let mut pidls: Vec<*mut ITEMIDLIST> = Vec::with_capacity(valid.len());
        for p in &valid {
            let hp = HSTRING::from(p.as_str());
            let pidl = ILCreateFromPathW(&hp);
            if !pidl.is_null() {
                pidls.push(pidl);
            }
        }
        if pidls.is_empty() {
            if ole_ok {
                OleUninitialize();
            }
            return Err("Failed to resolve shell PIDLs for drag".into());
        }

        let pidl_refs: Vec<*const ITEMIDLIST> = pidls.iter().map(|&p| p as *const ITEMIDLIST).collect();
        let data_obj: Result<IDataObject, _> = SHCreateDataObject(None, Some(&pidl_refs), None);
        let result = match data_obj {
            Ok(obj) => {
                let drop_source: IDropSource = win_ole::SimpleDropSource.into();
                let mut effect = DROPEFFECT(0);
                let _hr = DoDragDrop(
                    &obj,
                    &drop_source,
                    DROPEFFECT_COPY | DROPEFFECT_MOVE | DROPEFFECT_LINK,
                    &mut effect,
                );
                Ok(())
            }
            Err(e) => Err(format!("SHCreateDataObject failed: {}", e)),
        };

        for pidl in pidls {
            ILFree(Some(pidl as *const ITEMIDLIST));
        }
        if ole_ok {
            OleUninitialize();
        }
        result
    }
}

#[cfg(not(windows))]
pub fn start_file_drag(_paths: Vec<String>) -> Result<(), String> {
    Err("File drag is only supported on Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_open_files_quotes_and_sanitizes() {
        let args = format_open_files_args(&[
            "C:\\Users\\Test\\my file.txt".into(),
            "  ".into(),
            "D:\\a\"b.png".into(),
        ]);
        assert_eq!(args, "\"C:\\Users\\Test\\my file.txt\" \"D:\\ab.png\"");
    }
}
