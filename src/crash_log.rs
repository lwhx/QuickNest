//! Crash logging (baseline 24 章 稳定性 / P4-07 崩溃日志).
//!
//! Two layers:
//! 1. A Rust panic hook writes `crash-log.txt` (panic message, location and
//!    a thread dump) into the data directory (portable mode) or the per-user
//!    data directory (installed mode).
//! 2. A `--crash-test` mode forces a real panic so the acceptance can prove
//!    the hook fires and the file lands in the right place.
//!
//! The hook is installed on startup, before any window exists, so even an
//! early panic is recorded.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Installs the panic hook once. Safe to call repeatedly (subsequent calls
/// are no-ops). The previous hook (if any) is chained.
pub fn install() {
    install_at_path(crash_log_path());
}

fn install_at_path(path: Option<PathBuf>) {
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(path) = path.as_deref() {
            let _ = write_crash_log_at(path, info);
        }
        default_hook(info);
    }));
}

/// The crash-log path: the data directory (env override → portable marker →
/// `<exe>\data`, installed → `%APPDATA%\QuickNest\data`) plus
/// `crash-log.txt`.
pub fn crash_log_path() -> Option<PathBuf> {
    let dir = crate::portable::effective_data_dir().or_else(|| {
        std::env::var_os("APPDATA").map(|appdata| {
            std::path::PathBuf::from(appdata)
                .join("QuickNest")
                .join("data")
        })
    })?;
    Some(dir.join("crash-log.txt"))
}

/// Writes one crash entry. Kept public for the panic hook and tests.
pub fn write_crash_log(info: &std::panic::PanicHookInfo<'_>) -> std::io::Result<()> {
    let Some(path) = crash_log_path() else {
        return Ok(());
    };
    write_crash_log_at(&path, info)
}

fn write_crash_log_at(
    path: &std::path::Path,
    info: &std::panic::PanicHookInfo<'_>,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("<unnamed>").to_string();
    let location = match info.location() {
        Some(location) => format!("{}:{}", location.file(), location.line()),
        None => "<unknown>".to_string(),
    };
    let message = payload_text(info);
    let entry = format!(
        "[{}] panic in thread '{}' at {}\nmessage: {}\n---\n",
        chrono_stamp(),
        thread_name,
        location,
        message
    );
    use std::io::Write as _;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(entry.as_bytes())
}

fn payload_text(info: &std::panic::PanicHookInfo<'_>) -> String {
    if let Some(text) = info.payload().downcast_ref::<&str>() {
        return (*text).to_string();
    }
    if let Some(text) = info.payload().downcast_ref::<String>() {
        return text.clone();
    }
    "<non-string panic payload>".to_string()
}

/// Local timestamp (no external crates): seconds since epoch rendered as ISO.
fn chrono_stamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = now / 86_400;
    let secs_of_day = now % 86_400;
    let (hour, minute, second) = (
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60,
    );
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {hour:02}:{minute:02}:{second:02} UTC")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_log_path_follows_the_data_dir_rules() {
        // Resolving a path is read-only, including during parallel cargo tests.
        let expected = crate::portable::effective_data_dir().or_else(|| {
            std::env::var_os("APPDATA")
                .map(|base| PathBuf::from(base).join("QuickNest").join("data"))
        });
        assert_eq!(
            crash_log_path(),
            expected.map(|dir| dir.join("crash-log.txt"))
        );
    }

    #[test]
    fn panic_hook_writes_the_crash_log() {
        // The child owns the process-global hook. No environment mutation or
        // global hook survives into another test (including a failing test).
        let dir = std::env::temp_dir().join(format!(
            "qn-p407-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir(&dir).expect("unique sandbox");
        let path = dir.join("crash-log.txt");
        let output = std::process::Command::new(std::env::current_exe().expect("test exe"))
            .args([
                "--exact",
                "crash_log::tests::panic_hook_child",
                "--nocapture",
                "--ignored",
            ])
            .env("QUICKNEST_CRASH_TEST_PATH", &path)
            .output()
            .expect("isolated hook child");
        assert!(
            output.status.success(),
            "child: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let content = std::fs::read_to_string(&path).expect("read sandbox log");
        assert_eq!(
            content.matches("message: crash-log test message").count(),
            2
        );
        assert!(content.contains("panic in thread"));
        assert!(content.contains(" UTC]"));
        assert!(dir.starts_with(std::env::temp_dir()));
        assert!(
            dir.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("qn-p407-")
        );
        assert!(
            !std::fs::symlink_metadata(&dir)
                .expect("sandbox metadata")
                .file_type()
                .is_symlink()
        );
        std::fs::remove_dir_all(&dir).expect("remove owned sandbox");
    }

    #[test]
    #[ignore = "helper runs only as a dedicated panic-hook child"]
    fn panic_hook_child() {
        let Some(path) = std::env::var_os("QUICKNEST_CRASH_TEST_PATH") else {
            return;
        };
        let path = PathBuf::from(path);
        assert!(path.is_absolute());
        assert_eq!(path.file_name().unwrap(), "crash-log.txt");
        install_at_path(Some(path));
        for _ in 0..2 {
            let result = std::panic::catch_unwind(|| panic!("crash-log test message"));
            assert!(result.is_err());
        }
    }
}
