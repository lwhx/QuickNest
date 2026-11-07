//! Portable mode (baseline 23 章 便携版, P4-02).
//!
//! A copy of QuickNest becomes "portable" when a `QuickNest.portable` marker
//! file sits next to the exe. In portable mode the data directory is
//! `<exe dir>\data` (config + icon cache live inside the folder), so moving
//! the folder — to another path or another drive — moves the data with it.
//! Items whose launch paths are relative are resolved against the portable
//! root at every use site (launch, missing check, icon, file location), so a
//! folder of tools referenced with relative paths keeps working after a move.
//!
//! The installed build (no marker) keeps the established per-user data
//! location (%APPDATA%\QuickNest) and untouched launch semantics.

use std::path::{Path, PathBuf};

/// The marker file that turns a copy of the app into a portable one.
pub const MARKER: &str = "QuickNest.portable";

/// The data directory name inside the portable root.
pub const DATA_DIR: &str = "data";

/// The portable root: the exe's directory when the marker file is present.
pub fn root_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.to_path_buf();
    if dir.join(MARKER).is_file() {
        Some(dir)
    } else {
        None
    }
}

/// The data directory for this copy: `<root>\data` in portable mode, `None`
/// (the caller falls back to the per-user location) otherwise.
pub fn data_dir() -> Option<PathBuf> {
    root_dir().map(|root| root.join(DATA_DIR))
}

/// `QUICKNEST_DATA_DIR` 显式指定数据目录时优先于一切（验收脚本用它隔离
/// 测试数据，绝不触碰真实用户配置）。
pub fn override_data_dir() -> Option<PathBuf> {
    std::env::var_os("QUICKNEST_DATA_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Resolves the one data directory without creating it. An explicit override
/// must be nonempty and absolute; invalid input is an error, never a fallback
/// to a real user's configuration. APPDATA must also be absolute.
pub fn data_directory() -> std::io::Result<PathBuf> {
    let override_dir = std::env::var_os("QUICKNEST_DATA_DIR");
    // A valid override must work even if the executable cannot be located.
    if let Some(value) = override_dir.as_deref() {
        return select_data_directory(Some(value), None, None);
    }
    let exe = std::env::current_exe()?;
    let root = exe
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "无法确定程序目录"))?;
    let marker = root.join(MARKER);
    let portable = match std::fs::metadata(&marker) {
        Ok(metadata) if metadata.is_file() => Some(root),
        Ok(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "便携标记必须是文件",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    select_data_directory(None, portable, std::env::var_os("APPDATA").as_deref())
}

fn select_data_directory(
    override_dir: Option<&std::ffi::OsStr>,
    portable_root: Option<&Path>,
    appdata: Option<&std::ffi::OsStr>,
) -> std::io::Result<PathBuf> {
    let absolute = |value: &std::ffi::OsStr, name: &str| {
        let path = PathBuf::from(value);
        if value.is_empty() || !path.is_absolute() {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("{name} 必须是非空绝对路径"),
            ))
        } else {
            Ok(path)
        }
    };
    if let Some(value) = override_dir {
        return absolute(value, "QUICKNEST_DATA_DIR");
    }
    if let Some(root) = portable_root {
        return Ok(root.join(DATA_DIR));
    }
    let value = appdata.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "未配置 APPDATA 数据目录")
    })?;
    Ok(absolute(value, "APPDATA")?.join("QuickNest"))
}

/// Compatibility helper. New callers must use `data_directory()` and handle
/// its error; falling back after an invalid override defeats data isolation.
pub fn effective_data_dir() -> Option<PathBuf> {
    data_directory().ok()
}

/// Resolves a stored target against a root: relative paths join the root,
/// absolute paths (and empty ones) pass through unchanged. Pure — the unit
/// tests cover the rules, the IO-free way.
pub fn resolve_with(root: Option<&Path>, path: &str) -> PathBuf {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return PathBuf::from(trimmed);
    }
    // These are Shell/remote targets, never files below a portable root.
    if crate::library_model::is_uri_target(trimmed)
        || crate::library_model::is_shell_namespace_target(trimmed)
        || crate::library_model::is_unc_path(trimmed)
        || trimmed.starts_with("//")
    {
        return PathBuf::from(trimmed);
    }
    let candidate = Path::new(trimmed);
    match root {
        Some(root) if candidate.is_relative() => root.join(candidate),
        _ => candidate.to_path_buf(),
    }
}

/// Resolves a stored target against the portable root (no marker → unchanged).
pub fn resolve_path(path: &str) -> PathBuf {
    resolve_with(root_dir().as_deref(), path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_directory_selection_is_absolute_and_does_not_silently_fallback() {
        use std::ffi::OsStr;
        let root = Path::new(r"C:\Portable\QuickNest");
        let override_dir = OsStr::new(r"D:\isolated data");
        let appdata = OsStr::new(r"C:\Users\test\AppData\Roaming");
        assert_eq!(
            select_data_directory(Some(override_dir), Some(root), Some(appdata)).unwrap(),
            PathBuf::from(override_dir)
        );
        assert_eq!(
            select_data_directory(None, Some(root), None).unwrap(),
            root.join(DATA_DIR)
        );
        assert_eq!(
            select_data_directory(None, None, Some(appdata)).unwrap(),
            PathBuf::from(appdata).join("QuickNest")
        );
        for invalid in ["", "relative", r"C:drive-relative", r"\root-relative"] {
            assert!(
                select_data_directory(Some(OsStr::new(invalid)), Some(root), Some(appdata))
                    .is_err(),
                "{invalid}"
            );
            assert!(
                select_data_directory(None, None, Some(OsStr::new(invalid))).is_err(),
                "{invalid}"
            );
        }
        assert!(select_data_directory(None, None, None).is_err());
    }

    #[test]
    fn portable_resolution_preserves_uri_shell_and_unc_targets() {
        let root = Path::new(r"C:\Portable\QuickNest");
        for target in [
            "https://example.com/a",
            "shell:AppsFolder",
            "ms-settings:display",
            "::{20D04FE0-3AEA-1069-A2D8-08002B30309D}",
            r"\\server\share\file",
            "//server/share/file",
        ] {
            assert_eq!(
                resolve_with(Some(root), target),
                PathBuf::from(target),
                "{target}"
            );
        }
    }

    // 便携版（基线 23 章）：相对路径按便携根解析，绝对路径原样保留。
    #[test]
    fn relative_paths_join_the_root_absolutes_pass_through() {
        let root = Path::new("C:\\Portable\\QuickNest");
        assert_eq!(
            resolve_with(Some(root), "tools\\app.exe"),
            PathBuf::from("C:\\Portable\\QuickNest\\tools\\app.exe")
        );
        assert_eq!(
            resolve_with(Some(root), "C:\\Windows\\System32\\notepad.exe"),
            PathBuf::from("C:\\Windows\\System32\\notepad.exe")
        );
        // Forward slashes keep working through PathBuf on Windows.
        assert_eq!(
            resolve_with(Some(root), "data/config.json"),
            PathBuf::from("C:\\Portable\\QuickNest\\data\\config.json")
        );
    }

    // 便携版：无根（安装版）时空/相对/绝对目标都不改变既有行为。
    #[test]
    fn without_a_root_everything_stays_as_stored() {
        assert_eq!(
            resolve_with(None, "tools\\app.exe"),
            PathBuf::from("tools\\app.exe")
        );
        assert_eq!(
            resolve_with(None, "C:\\Windows\\System32\\notepad.exe"),
            PathBuf::from("C:\\Windows\\System32\\notepad.exe")
        );
    }

    // 便携版：空目标保持为空（各调用点用 trim().is_empty() 先行拒绝）。
    #[test]
    fn empty_targets_stay_empty() {
        assert_eq!(resolve_with(Some(Path::new("C:\\")), ""), PathBuf::from(""));
        assert_eq!(
            resolve_with(Some(Path::new("C:\\")), "   "),
            PathBuf::from("")
        );
    }
}
