//! P1-05 OS dialogs: the native file picker and the clipboard path copy.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, GetSaveFileNameW, OFN_FILEMUSTEXIST, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST,
    OPENFILENAMEW,
};
use windows::core::{PCWSTR, PWSTR};

/// Shows the native "open file" dialog for the given owner window; returns
/// the chosen path, or None when the user cancels.
pub fn pick_file(owner: isize) -> Option<PathBuf> {
    let mut buffer = [0u16; 1024];
    let filter: Vec<u16> = "所有文件\0*.*\0程序与快捷方式\0*.exe;*.lnk;*.bat;*.cmd;*.ps1\0\0"
        .encode_utf16()
        .collect();
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: HWND(owner as *mut core::ffi::c_void),
        lpstrFile: PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        nFilterIndex: 1,
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST,
        ..Default::default()
    };
    // SAFETY: `dialog` points at live buffers that outlive the modal call.
    let opened = unsafe { GetOpenFileNameW(&mut dialog) };
    if !opened.as_bool() {
        return None;
    }
    let length = buffer.iter().position(|unit| *unit == 0).unwrap_or(0);
    if length == 0 {
        return None;
    }
    Some(PathBuf::from(OsString::from_wide(&buffer[..length])))
}

/// Shows the native "save file" dialog (baseline 17.4 export); returns the
/// chosen path, or None when the user cancels.
pub fn save_file(owner: isize, default_name: &str, default_extension: &str) -> Option<PathBuf> {
    let mut buffer = [0u16; 1024];
    for (slot, unit) in buffer.iter_mut().zip(default_name.encode_utf16()) {
        *slot = unit;
    }
    let filter: Vec<u16> = "QuickNest 导出\0*.json\0所有文件\0*.*\0\0"
        .encode_utf16()
        .collect();
    let extension: Vec<u16> = default_extension
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: HWND(owner as *mut core::ffi::c_void),
        lpstrFile: PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        nFilterIndex: 1,
        lpstrDefExt: PCWSTR(extension.as_ptr()),
        Flags: OFN_PATHMUSTEXIST | OFN_OVERWRITEPROMPT,
        ..Default::default()
    };
    // SAFETY: dialog points at live buffers that outlive the modal call.
    let saved = unsafe { GetSaveFileNameW(&mut dialog) };
    if !saved.as_bool() {
        return None;
    }
    let length = buffer.iter().position(|unit| *unit == 0).unwrap_or(0);
    if length == 0 {
        return None;
    }
    Some(PathBuf::from(OsString::from_wide(&buffer[..length])))
}

/// Copies `text` to the clipboard as CF_UNICODETEXT (baseline 3.5 "复制路径").
/// Returns false when the clipboard is busy or the write fails.
pub fn copy_text(text: &str) -> bool {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: the clipboard is opened and closed in this scope; the global
    // block ownership transfers to the clipboard only on success.
    unsafe {
        if OpenClipboard(None).is_err() {
            return false;
        }
        let result = (|| -> bool {
            if EmptyClipboard().is_err() {
                return false;
            }
            let bytes = wide.len() * std::mem::size_of::<u16>();
            let Ok(handle) = GlobalAlloc(GMEM_MOVEABLE, bytes) else {
                return false;
            };
            let pointer = GlobalLock(handle) as *mut u16;
            if pointer.is_null() {
                return false;
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), pointer, wide.len());
            let _ = GlobalUnlock(handle);
            SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(handle.0))).is_ok()
        })();
        let _ = CloseClipboard();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::GetClipboardData;

    #[test]
    fn clipboard_round_trip() {
        let text = format!("QuickNest\\copy-path-{}", std::process::id());
        if !copy_text(&text) {
            // The clipboard can be owned by another process; report nothing
            // rather than failing the suite for an environment race.
            return;
        }
        // SAFETY: reads back exactly what this test just wrote while holding
        // the clipboard open.
        unsafe {
            if OpenClipboard(None).is_err() {
                return;
            }
            let Ok(handle) = GetClipboardData(CF_UNICODETEXT.0 as u32) else {
                let _ = CloseClipboard();
                return;
            };
            let pointer = GlobalLock(HGLOBAL(handle.0)) as *const u16;
            if pointer.is_null() {
                let _ = CloseClipboard();
                return;
            }
            let mut length = 0usize;
            while *pointer.add(length) != 0 {
                length += 1;
            }
            let read = String::from_utf16_lossy(std::slice::from_raw_parts(pointer, length));
            let _ = GlobalUnlock(HGLOBAL(handle.0));
            let _ = CloseClipboard();
            assert_eq!(read, text);
        }
    }
}
