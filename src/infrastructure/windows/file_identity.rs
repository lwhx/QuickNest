//! File identity for write protection, following Windows reparse/short aliases.

use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::Path;

use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_ID_INFO, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, FileIdInfo, GetFileInformationByHandleEx, OPEN_EXISTING,
};

#[derive(Debug, PartialEq, Eq)]
struct FileIdentity {
    volume: u64,
    id: [u8; 16],
}

#[derive(Debug)]
enum Identity {
    Existing(FileIdentity),
    Missing {
        parent: FileIdentity,
        name: OsString,
    },
}

fn file_identity(path: &Path) -> io::Result<FileIdentity> {
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NUL in file path",
        ));
    }
    wide.push(0);
    // SAFETY: The path is terminated and lives through the call. Access 0
    // queries metadata only; following reparses identifies the actual file.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: Successful CreateFileW transfers the sole owning handle to File.
    let file = unsafe { File::from_raw_handle(handle) };
    let mut info = FILE_ID_INFO::default();
    // SAFETY: info is a correctly sized, writable FILE_ID_INFO.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            (&mut info as *mut FILE_ID_INFO).cast(),
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(FileIdentity {
        volume: info.VolumeSerialNumber,
        id: info.FileId.Identifier,
    })
}

fn identity(path: &Path) -> io::Result<Identity> {
    match file_identity(path) {
        Ok(id) => Ok(Identity::Existing(id)),
        Err(error) if matches!(error.raw_os_error(), Some(2 | 3)) => {
            // A dangling reparse point is not an ordinary missing destination.
            if std::fs::symlink_metadata(path).is_ok() {
                return Err(error);
            }
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let name = path
                .file_name()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "file name missing"))?;
            // Resolve the parent through the same volume+ID query: directory
            // junctions, case changes and short directory names all converge.
            Ok(Identity::Missing {
                parent: file_identity(parent)?,
                name: name.to_os_string(),
            })
        }
        Err(error) => Err(error),
    }
}

// CompareStringOrdinal lives in kernel32; declaring this one ABI avoids adding
// a Cargo feature solely for the ordinal Windows filename comparison.
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CompareStringOrdinal(
        a: *const u16,
        a_len: i32,
        b: *const u16,
        b_len: i32,
        ignore_case: i32,
    ) -> i32;
}

fn same_name(left: &std::ffi::OsStr, right: &std::ffi::OsStr) -> io::Result<bool> {
    let name = |s: &std::ffi::OsStr| {
        let mut wide: Vec<u16> = s.encode_wide().collect();
        // Non-verbatim Win32 paths normalize trailing dots and spaces.
        while matches!(wide.last(), Some(32 | 46)) {
            wide.pop();
        }
        wide
    };
    let left = name(left);
    let right = name(right);
    let left_len = i32::try_from(left.len()).map_err(|_| io::ErrorKind::InvalidInput)?;
    let right_len = i32::try_from(right.len()).map_err(|_| io::ErrorKind::InvalidInput)?;
    // SAFETY: Both buffers remain live and their explicit lengths are valid.
    match unsafe { CompareStringOrdinal(left.as_ptr(), left_len, right.as_ptr(), right_len, 1) } {
        0 => Err(io::Error::last_os_error()),
        result => Ok(result == 2), // CSTR_EQUAL
    }
}

/// Existing files compare volume + 128-bit FileId, never content. Missing
/// files compare resolved parent identity and Windows ordinal filename. Any
/// inability to establish identity is an error, never permission to write.
pub(crate) fn same_file(left: &Path, right: &Path) -> io::Result<bool> {
    match (identity(left)?, identity(right)?) {
        (Identity::Existing(left), Identity::Existing(right)) => Ok(left == right),
        (
            Identity::Missing {
                parent: left_parent,
                name: left,
            },
            Identity::Missing {
                parent: right_parent,
                name: right,
            },
        ) => Ok(left_parent == right_parent && same_name(&left, &right)?),
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_identity_checks_short_and_reparse_aliases_when_available() {
        let dir = std::env::temp_dir().join(format!(
            "quicknest-identity-alias-{}",
            crate::library_model::new_uuid()
        ));
        std::fs::create_dir(&dir).unwrap();
        let active = dir.join("long-config-file-name.json");
        std::fs::write(&active, b"active").unwrap();
        let long: Vec<u16> = active.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut short = vec![0_u16; 32768];
        // SAFETY: Input is terminated and the output capacity matches the length.
        let count = unsafe {
            windows_sys::Win32::Storage::FileSystem::GetShortPathNameW(
                long.as_ptr(),
                short.as_mut_ptr(),
                short.len() as u32,
            )
        };
        assert!(count > 0 && (count as usize) < short.len());
        use std::os::windows::ffi::OsStringExt;
        let short = std::path::PathBuf::from(OsString::from_wide(&short[..count as usize]));
        assert!(same_file(&active, &short).unwrap());
        eprintln!("short-path spelling changed: {}", active != short);
        let link = dir.join("reparse.json");
        match std::os::windows::fs::symlink_file(&active, &link) {
            Ok(()) => {
                assert!(same_file(&active, &link).unwrap());
                std::fs::remove_file(&link).unwrap();
            }
            Err(error) if error.raw_os_error() == Some(1314) => {
                eprintln!("reparse alias not exercised: symlink privilege unavailable (1314)");
            }
            Err(error) => panic!("create owned reparse alias: {error}"),
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn file_identity_distinguishes_content_and_resolves_missing_parent_aliases() {
        let dir = std::env::temp_dir().join(format!(
            "quicknest-identity-{}",
            crate::library_model::new_uuid()
        ));
        std::fs::create_dir(&dir).unwrap();
        let left = dir.join("config.json");
        let right = dir.join("export.json");
        assert!(same_file(&left, &dir.join("CONFIG.JSON")).unwrap());
        assert!(!same_file(&left, &right).unwrap());
        std::fs::write(&left, b"identical").unwrap();
        std::fs::write(&right, b"identical").unwrap();
        assert!(!same_file(&left, &right).unwrap());
        let alias = dir.join("hardlink.json");
        std::fs::hard_link(&left, &alias).unwrap();
        assert!(same_file(&left, &alias).unwrap());
        assert!(same_file(&left, &dir.join("CONFIG.JSON")).unwrap());
        assert!(same_file(&left, &dir.join("missing-parent/file.json")).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
