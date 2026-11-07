use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::CreateFileW;

use quicknest::infrastructure::windows::config_save::{
    RecoveryReport, SaveError, SaveOutcome, recover_latest_backup, save_atomic,
};

struct Report {
    file: std::fs::File,
    started: std::time::Instant,
}

impl Report {
    fn open(path: &Path) -> Result<Self, Box<dyn Error>> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            file,
            started: std::time::Instant::now(),
        })
    }

    fn record(&mut self, phase: &str, detail: &str) -> Result<(), Box<dyn Error>> {
        writeln!(
            self.file,
            "{}\t{}\t{}",
            self.started.elapsed().as_millis(),
            phase,
            detail
        )?;
        self.file.flush()?;
        Ok(())
    }
}

fn require(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.to_string())
    }
}

/// Real Windows failure-injection lock: FILE_SHARE_READ only, so ReplaceFileW
/// (which needs delete access) must fail while the handle is open.
struct DeleteLock(HANDLE);

impl DeleteLock {
    fn acquire(target: &Path) -> Result<Self, Box<dyn Error>> {
        const GENERIC_READ: u32 = 0x8000_0000;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const OPEN_EXISTING: u32 = 3;
        use std::os::windows::ffi::OsStrExt as _;
        let mut wide: Vec<u16> = target.as_os_str().encode_wide().collect();
        wide.push(0);
        // SAFETY: Null-terminated wide path; the handle is closed in Drop on
        // the same thread that opened it.
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        require(
            !std::ptr::eq(handle, INVALID_HANDLE_VALUE) && !handle.is_null(),
            "CreateFileW lock handle failed",
        )?;
        Ok(Self(handle))
    }
}

impl Drop for DeleteLock {
    fn drop(&mut self) {
        // SAFETY: The handle came from CreateFileW in acquire.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn read_text(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(path)?)
}

fn leftover_temps(directory: &Path) -> Vec<PathBuf> {
    fs::read_dir(directory)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| path.extension().map(|ext| ext == "tmp").unwrap_or(false))
                .collect()
        })
        .unwrap_or_default()
}

fn leftover_backups(directory: &Path) -> Vec<PathBuf> {
    fs::read_dir(directory)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().contains(".backup."))
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Covers baseline 15.2-15.8 end to end on the real Windows file system:
/// first creation, atomic replace with backup, replace-failure protection,
/// temp-file lifecycle, and backup recovery including safe mode.
pub fn run(path: &Path) -> Result<(), Box<dyn Error>> {
    let file = std::fs::File::options()
        .write(true)
        .create_new(true)
        .open(path)?;
    drop(file);

    let mut report = Report::open(path)?;
    let result = run_phases(&mut report);
    if let Err(error) = &result {
        let _ = report.record("scenario_error", &error.to_string());
    }
    result.map_err(|error: String| error.into())
}

fn run_phases(report: &mut Report) -> Result<(), String> {
    let mut record = |phase: &str, detail: &str| -> Result<(), String> {
        report
            .record(phase, detail)
            .map_err(|error| error.to_string())
    };

    // Fresh workspace per run; the report lives elsewhere so the save
    // directory starts empty.
    let directory = std::env::temp_dir().join(format!(
        "quicknest-save-validation-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let target = directory.join("config.json");

    // a) Baseline 15.5 first creation: target missing -> Created, content
    //    written, no temp, no backup.
    let outcome = save_atomic(&target, "{\"gen\":1}").map_err(|error| error.to_string())?;
    require(outcome == SaveOutcome::Created, "expected Created")?;
    require(
        read_text(&target).map_err(|error| error.to_string())? == "{\"gen\":1}",
        "content mismatch after first save",
    )?;
    require(
        leftover_temps(&directory).is_empty() && leftover_backups(&directory).is_empty(),
        "first creation left temp/backup files",
    )?;
    record("first_create", "MoveFileExW WRITE_THROUGH; no temp/backup")?;

    // b) Baseline 15.4 atomic replace: previous content moves into a unique
    //    backup, no temp remains.
    let outcome = save_atomic(&target, "{\"gen\":2}").map_err(|error| error.to_string())?;
    let SaveOutcome::Replaced(replaced) = outcome else {
        return Err("expected Replaced".to_string());
    };
    require(
        read_text(&target).map_err(|error| error.to_string())? == "{\"gen\":2}",
        "content mismatch after replace",
    )?;
    require(
        read_text(&replaced.backup).map_err(|error| error.to_string())? == "{\"gen\":1}",
        "backup does not hold the previous content",
    )?;
    require(
        leftover_temps(&directory).is_empty(),
        "replace left a temp file",
    )?;
    record(
        "atomic_replace",
        &format!(
            "ReplaceFileW flags=0; backup={}",
            replaced
                .backup
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        ),
    )?;

    // c) Baseline 15.4 failure protection: lock the target with a real
    //    handle without delete sharing; the save must fail, keep the old
    //    config, keep the temp for diagnostics, and record the Win32 code.
    let lock = DeleteLock::acquire(&target).map_err(|error| error.to_string())?;
    let error = save_atomic(&target, "{\"gen\":3}")
        .err()
        .ok_or("save unexpectedly succeeded while the target was locked")?;
    drop(lock);
    let SaveError::ReplaceFailed {
        win32_error,
        temp_path,
    } = error
    else {
        return Err(format!("expected ReplaceFailed, got {error}"));
    };
    require(win32_error == 32, "expected ERROR_SHARING_VIOLATION(32)")?;
    require(
        read_text(&target).map_err(|error| error.to_string())? == "{\"gen\":2}",
        "previous config was modified by the failed save",
    )?;
    require(temp_path.exists(), "temp file not kept for diagnostics")?;
    require(
        read_text(&temp_path).map_err(|error| error.to_string())? == "{\"gen\":3}",
        "kept temp does not hold the failed write",
    )?;
    record(
        "failure_protection",
        &format!("win32_error={win32_error}; target intact; temp kept for diagnostics"),
    )?;
    // Diagnostics complete: clean the kept temp, restoring a pristine state.
    fs::remove_file(&temp_path).map_err(|error| error.to_string())?;
    require(
        leftover_temps(&directory).is_empty(),
        "temp cleanup after diagnostics failed",
    )?;
    record(
        "temp_lifecycle",
        "success paths leave none; failure path keeps one until cleaned",
    )?;

    // d) Baseline 15.8 recovery: newest backup broken, middle valid, oldest
    //    valid; recovery must restore the newest VALID one and rename the
    //    corrupt live file to config.corrupted.<stamp>.json. The saves from
    //    phases a-b left a real backup of their own, so the directory is
    //    rotated first to give the scan a fully controlled backup set.
    for backup in leftover_backups(&directory) {
        fs::remove_file(backup).map_err(|error| error.to_string())?;
    }
    let stamps = [
        ("20261101-000000-000", Some("{\"gen\":10}")),
        ("20261102-000000-000", Some("{\"gen\":11}")),
        ("20261103-000000-000", None), // newest but broken JSON
    ];
    for (stamp, content) in stamps {
        let backup = directory.join(format!("config.backup.{stamp}.json"));
        match content {
            Some(valid) => fs::write(&backup, valid).map_err(|error| error.to_string())?,
            None => fs::write(&backup, "not json").map_err(|error| error.to_string())?,
        }
    }
    fs::write(&target, "{corrupt").map_err(|error| error.to_string())?;
    let report_recovery = recover_latest_backup(&target).map_err(|error| error.to_string())?;
    let RecoveryReport {
        restored_from,
        corrupted_path,
    } = report_recovery;
    require(
        restored_from
            .map(|path| path.ends_with("config.backup.20261102-000000-000.json"))
            .unwrap_or(false),
        "recovery did not pick the newest valid backup",
    )?;
    require(
        read_text(&target).map_err(|error| error.to_string())? == "{\"gen\":11}",
        "restored content mismatch",
    )?;
    let corrupted_path = corrupted_path.ok_or("corrupt live file was not renamed")?;
    require(
        corrupted_path.to_string_lossy().contains(".corrupted."),
        "corrupt file rename name malformed",
    )?;
    record(
        "backup_recovery",
        &format!(
            "restored=newest-valid; corrupted={}",
            corrupted_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        ),
    )?;

    // e) Baseline 15.8 safe mode: all backups broken -> nothing touched.
    for backup in leftover_backups(&directory) {
        fs::write(backup, "not json").map_err(|error| error.to_string())?;
    }
    fs::write(&target, "{corrupt").map_err(|error| error.to_string())?;
    let safe = recover_latest_backup(&target).map_err(|error| error.to_string())?;
    require(safe.restored_from.is_none(), "safe mode restored something")?;
    require(safe.corrupted_path.is_none(), "safe mode renamed files")?;
    require(
        read_text(&target).map_err(|error| error.to_string())? == "{corrupt",
        "safe mode overwrote the original",
    )?;
    record("safe_mode", "no valid backup -> original untouched")?;

    record("completed", "all save phases passed")?;
    let _ = fs::remove_dir_all(&directory);
    Ok(())
}
