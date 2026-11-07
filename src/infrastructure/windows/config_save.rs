//! Atomic JSON config persistence (baseline chapter 15).
//!
//! Write chain: exclusive same-directory temp file → full write +
//! `sync_all` → close handle → `ReplaceFileW` (existing target, unique
//! backup) or `MoveFileExW` same-volume move (first creation). Every
//! failure keeps the previous config intact and the temp file on disk for
//! diagnostics, and records the Win32 error code. One write transaction at
//! a time (baseline 15.7): a save attempted while the write lock is held
//! (including recovery) is rejected with [`SaveError::Busy`].

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW, ReplaceFileW};
use windows_sys::core::GUID;

/// The config target exists and was replaced through ReplaceFileW; the
/// previous content now lives in the backup file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replaced {
    pub backup: PathBuf,
}

/// What a successful save did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveOutcome {
    /// The target did not exist; the temp file was moved into place.
    Created,
    /// The target existed and was atomically replaced.
    Replaced(Replaced),
}

#[derive(Debug)]
pub enum SaveError {
    /// Another write transaction (save or recovery) holds the write lock.
    Busy,
    /// The temp file could not be created or written; nothing touched.
    TempFile { os_error: std::io::Error },
    /// ReplaceFileW or MoveFileExW failed. The previous config is intact
    /// and the temp file is kept for diagnostics (baseline 15.4).
    ReplaceFailed {
        win32_error: u32,
        temp_path: PathBuf,
    },
    /// The parent directory does not exist.
    MissingDirectory(PathBuf),
    /// Recovery was asked to run but the live config is a supported Library.
    TargetAlreadyValid,
    /// The backup is not a readable schema-2 Library.
    InvalidBackup,
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => f.write_str("another write transaction is active"),
            Self::TempFile { os_error } => write!(f, "temp file failed: {os_error}"),
            Self::ReplaceFailed {
                win32_error,
                temp_path,
            } => write!(
                f,
                "replace failed: {win32_error} (0x{win32_error:08X}); temp kept at {}",
                temp_path.display()
            ),
            Self::MissingDirectory(path) => {
                write!(f, "target directory missing: {}", path.display())
            }
            Self::TargetAlreadyValid => {
                f.write_str("recovery refused: the live config is a supported Library")
            }
            Self::InvalidBackup => {
                f.write_str("restore refused: the backup is not a supported schema-2 Library")
            }
        }
    }
}

impl std::error::Error for SaveError {}

/// The single-writer lock (baseline 15.7). Process-local for P0; the
/// cross-instance mutex from P0-08 gates whole processes, and P1 wiring
/// will share this lock across them if both stay enabled.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// Parses the target into (directory, file name); both are required.
fn split_target(target: &Path) -> Result<(PathBuf, String), SaveError> {
    let directory = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| SaveError::MissingDirectory(directory.clone()))?;
    Ok((directory, name))
}

/// Process-unique GUID from UuidCreate (rpcrt4); used in temp file names.
fn new_uuid() -> Option<String> {
    // SAFETY: UuidCreate takes no arguments and only fills the GUID.
    let mut guid = GUID::default();
    let status = unsafe { windows_sys::Win32::System::Rpc::UuidCreate(&mut guid) };
    if status != 0 {
        return None;
    }
    Some(format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        guid.data1,
        guid.data2,
        guid.data3,
        guid.data4[0],
        guid.data4[1],
        guid.data4[2],
        guid.data4[3],
        guid.data4[4],
        guid.data4[5],
        guid.data4[6],
        guid.data4[7],
    ))
}

/// Monotonic fallback unique suffix (PID + nanos + counter) used when
/// UuidCreate is unavailable.
fn fallback_unique() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{:x}-{:x}", std::process::id(), nanos, counter)
}

/// `<name>.<PID>.<uuid>.tmp` next to the target (baseline 15.3).
fn temp_path_for(directory: &Path, name: &str) -> PathBuf {
    let unique = new_uuid().unwrap_or_else(fallback_unique);
    directory.join(format!("{name}.{}.{}.tmp", std::process::id(), unique))
}

/// `config.backup.<timestamp>.json` next to the target: unique per call in
/// practice, same volume, and lexicographic order equals time order so the
/// baseline 15.8 recovery scan can sort by name alone. Sub-millisecond
/// saves in the same millisecond append a counter via fallback_unique.
fn backup_path_for(directory: &Path, name: &str) -> PathBuf {
    let stem = name.strip_suffix(".json").unwrap_or(name);
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    directory.join(format!(
        "{stem}.backup.{}-{:016x}.json",
        timestamp_string(SystemTime::now()),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

/// UTC `YYYYmmdd-HHMMSS-mmm` timestamp (civil-calendar conversion).
fn timestamp_string(time: SystemTime) -> String {
    let duration = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0));
    let total_seconds = duration.as_secs();
    let millis = duration.subsec_millis();
    let days = (total_seconds / 86_400) as i64;
    let seconds_of_day = total_seconds % 86_400;
    let (hour, minute, second) = (
        seconds_of_day / 3600,
        (seconds_of_day % 3600) / 60,
        seconds_of_day % 60,
    );
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_shift = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_shift + 2) / 5 + 1;
    let month = if month_shift < 10 {
        month_shift + 3
    } else {
        month_shift - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}-{millis:03}")
}

/// Writes content exclusively to a fresh same-directory temp file and closes
/// the handle (baseline 15.3). Returns the temp path on success.
fn write_temp(target: &Path, content: &str) -> Result<PathBuf, SaveError> {
    let (directory, name) = split_target(target)?;
    if !directory.is_dir() {
        return Err(SaveError::MissingDirectory(directory));
    }
    let mut temp_path = temp_path_for(&directory, &name);
    let mut file = loop {
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
        {
            Ok(file) => break file,
            // A GUID collision is practically impossible; the fallback path
            // however can collide on identical timestamps, so retry once.
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                temp_path = directory.join(format!(
                    "{name}.{}.{}.tmp",
                    std::process::id(),
                    fallback_unique()
                ));
            }
            Err(os_error) => return Err(SaveError::TempFile { os_error }),
        }
    };
    if let Err(os_error) = file
        .write_all(content.as_bytes())
        .and_then(|()| file.sync_all())
    {
        // The temp file is unusable; remove it so no garbage stays behind.
        let _ = fs::remove_file(&temp_path);
        return Err(SaveError::TempFile { os_error });
    }
    drop(file); // close the handle before the replace (baseline 15.2)
    Ok(temp_path)
}

fn to_wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt as _;
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    wide
}

/// ReplaceFileW with flags 0 (baseline 15.4: no WRITE_THROUGH — unsupported,
/// no ACL-ignoring flags). On failure the target is untouched and the temp
/// file stays for diagnostics.
fn replace_via_replacefile(
    target: &Path,
    temp_path: &Path,
    backup: &Path,
) -> Result<(), SaveError> {
    let target_wide = to_wide(target);
    let temp_wide = to_wide(temp_path);
    let backup_wide = to_wide(backup);
    // SAFETY: All three paths are null-terminated wide strings owned for the
    // call's duration; flags 0 keeps the baseline-restricted behavior.
    let replaced = unsafe {
        ReplaceFileW(
            target_wide.as_ptr(),
            temp_wide.as_ptr(),
            backup_wide.as_ptr(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if replaced == 0 {
        return Err(SaveError::ReplaceFailed {
            win32_error: unsafe { GetLastError() },
            temp_path: temp_path.to_path_buf(),
        });
    }
    Ok(())
}

/// Same-volume MoveFileExW with WRITE_THROUGH for first creation (baseline
/// 15.5). Returns false when the target exists concurrently (ERROR_ALREADY_
/// EXISTS, 183), telling the caller to fall back to ReplaceFileW.
fn move_into_place(temp_path: &Path, target: &Path) -> Result<bool, SaveError> {
    let temp_wide = to_wide(temp_path);
    let target_wide = to_wide(target);
    // SAFETY: Null-terminated wide strings owned for the call's duration.
    let moved = unsafe {
        MoveFileExW(
            temp_wide.as_ptr(),
            target_wide.as_ptr(),
            MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved != 0 {
        return Ok(true);
    }
    let error = unsafe { GetLastError() };
    if error == 183 {
        // ERROR_ALREADY_EXISTS: the target appeared concurrently; fall back.
        return Ok(false);
    }
    Err(SaveError::ReplaceFailed {
        win32_error: error,
        temp_path: temp_path.to_path_buf(),
    })
}

/// The single-writer lock (baseline 15.7). Process-local for P0; the
/// cross-instance mutex from P0-08 gates whole processes, and P1 wiring
/// will share this lock across them if both stay enabled. A poisoned lock
/// (a panic mid-transaction) does not wedge the writer: the next caller
/// takes over the guard instead of failing forever.
fn acquire_write_lock() -> Option<std::sync::MutexGuard<'static, ()>> {
    match WRITE_LOCK.try_lock() {
        Ok(guard) => Some(guard),
        Err(_) if WRITE_LOCK.is_poisoned() => {
            WRITE_LOCK.clear_poison();
            WRITE_LOCK.try_lock().ok()
        }
        Err(_) => None,
    }
}

/// Atomic save per baseline 15.2-15.6. Every failure keeps the previous
/// config; failure details carry the Win32 error and the kept temp path.
pub fn save_atomic(target: &Path, content: &str) -> Result<SaveOutcome, SaveError> {
    let Some(_guard) = acquire_write_lock() else {
        return Err(SaveError::Busy);
    };
    save_atomic_locked(target, content)
}

fn save_atomic_locked(target: &Path, content: &str) -> Result<SaveOutcome, SaveError> {
    let (directory, name) = split_target(target)?;
    let temp_path = write_temp(target, content)?;
    if !target.exists() && move_into_place(&temp_path, target)? {
        return Ok(SaveOutcome::Created);
    }
    // Either the target pre-existed or it appeared concurrently; ReplaceFileW
    // backs up whatever is there instead of losing it. On success it consumes
    // the temp file; on failure the temp stays for diagnostics (15.4).
    let backup = backup_path_for(&directory, &name);
    replace_via_replacefile(target, &temp_path, &backup)
        .map(|()| SaveOutcome::Replaced(Replaced { backup }))
}

/// What startup recovery decided (baseline 15.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryReport {
    /// The backup that was restored; None in safe mode.
    pub restored_from: Option<PathBuf>,
    /// Where the corrupt live config was renamed; None in safe mode or when
    /// the live file was missing entirely.
    pub corrupted_path: Option<PathBuf>,
}

/// Shared validation for every backup entry point, including UTF-8 BOM files.
pub(crate) fn parse_supported_library(content: &str) -> Option<crate::library_model::Library> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let library: crate::library_model::Library = serde_json::from_str(content).ok()?;
    (library.schema_version == crate::config_transfer::SUPPORTED_SCHEMA).then_some(library)
}

/// Recovers the newest usable schema-2 Library backup into `target`.
pub fn recover_latest_backup(target: &Path) -> Result<RecoveryReport, SaveError> {
    let Some(_guard) = acquire_write_lock() else {
        return Err(SaveError::Busy);
    };
    // The recovery contract (15.8) is "startup config parse failed"; refuse
    // to run against a live file that is still valid.
    if let Ok(content) = fs::read_to_string(target)
        && parse_supported_library(&content).is_some()
    {
        return Err(SaveError::TargetAlreadyValid);
    }
    let (directory, name) = split_target(target)?;
    let stem = name.strip_suffix(".json").unwrap_or(&name);
    let mut backups: Vec<PathBuf> = fs::read_dir(&directory)
        .map_err(|os_error| SaveError::TempFile { os_error })?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .map(|file_name| {
                    let text = file_name.to_string_lossy();
                    text.starts_with(&format!("{stem}.backup."))
                        && text.ends_with(".json")
                        && path.is_file()
                })
                .unwrap_or(false)
        })
        .collect();
    backups.sort_unstable_by(|left, right| right.file_name().cmp(&left.file_name()));
    let mut valid: Option<(PathBuf, String)> = None;
    for backup in backups {
        let Ok(content) = fs::read_to_string(&backup) else {
            continue;
        };
        if parse_supported_library(&content).is_some() {
            valid = Some((backup, content));
            break;
        }
    }
    let Some((backup, content)) = valid else {
        // Safe mode: no valid backup; never overwrite the original (15.8.7).
        return Ok(RecoveryReport {
            restored_from: None,
            corrupted_path: None,
        });
    };
    // Rename the corrupt live file out of the way first, then restore. If
    // the restore cannot complete, put the original back and surface the
    // error instead of leaving the target missing.
    let corrupted = directory.join(format!(
        "{stem}.corrupted.{}.json",
        timestamp_string(SystemTime::now())
    ));
    let corrupted = match fs::rename(target, &corrupted) {
        Ok(()) => Some(corrupted),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(os_error) => return Err(SaveError::TempFile { os_error }),
    };
    match save_atomic_locked(target, &content) {
        Ok(_) => Ok(RecoveryReport {
            restored_from: Some(backup),
            corrupted_path: corrupted,
        }),
        Err(error) => {
            if let Some(corrupted) = corrupted {
                // Preserve the original when the replacement fails.
                if fs::rename(&corrupted, target).is_err() {
                    eprintln!("config recovery: original copy at {}", corrupted.display());
                }
            }
            Err(error)
        }
    }
}

/// Keeps at most `keep` valid backups for `target` (baseline 15.7: default 10),
/// deleting invalid and older entries. Timestamps in the names sort
/// lexicographically, so newest-first ordering is a name sort. Returns the
/// number of removed files.
pub fn prune_backups(target: &Path, keep: usize) -> usize {
    let Some(_guard) = acquire_write_lock() else {
        return 0;
    };
    prune_backups_locked(target, keep)
}

fn prune_backups_locked(target: &Path, keep: usize) -> usize {
    let Ok((directory, name)) = split_target(target) else {
        return 0;
    };
    let stem = name.strip_suffix(".json").unwrap_or(&name);
    let Ok(entries) = fs::read_dir(&directory) else {
        return 0;
    };
    let prefix = format!("{stem}.backup.");
    let mut backups: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .map(|file_name| {
                    let text = file_name.to_string_lossy();
                    text.starts_with(&prefix) && text.ends_with(".json") && path.is_file()
                })
                .unwrap_or(false)
        })
        .collect();
    backups.sort_unstable_by(|left, right| right.file_name().cmp(&left.file_name()));
    let mut kept = 0_usize;
    let mut removed = 0_usize;
    for backup in backups {
        let valid = fs::read_to_string(&backup)
            .ok()
            .is_some_and(|content| parse_supported_library(&content).is_some());
        if valid && kept < keep {
            kept += 1;
            continue;
        }
        if fs::remove_file(&backup).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Creates an explicit manual backup of an existing config (baseline 15.7)
/// and prunes old backups. The file name matches the startup recovery scan
/// pattern (`<stem>.backup.<timestamp>.json`).
pub fn backup_now(target: &Path) -> Result<PathBuf, SaveError> {
    let Some(_guard) = acquire_write_lock() else {
        return Err(SaveError::Busy);
    };
    let (directory, name) = split_target(target)?;
    let content =
        fs::read_to_string(target).map_err(|os_error| SaveError::TempFile { os_error })?;
    if parse_supported_library(&content).is_none() {
        return Err(SaveError::InvalidBackup);
    }
    let backup = backup_path_for(&directory, &name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&backup)
        .map_err(|os_error| SaveError::TempFile { os_error })?;
    if let Err(os_error) = file
        .write_all(content.as_bytes())
        .and_then(|()| file.sync_all())
    {
        drop(file);
        let _ = fs::remove_file(&backup);
        return Err(SaveError::TempFile { os_error });
    }
    drop(file);
    prune_backups_locked(target, 10);
    Ok(backup)
}

/// P2-16 (baseline 15.7/17.4): one historical backup the user can restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupEntry {
    pub path: PathBuf,
    /// The timestamp part of `<stem>.backup.<stamp>.json`.
    pub label: String,
    pub bytes: u64,
}

impl BackupEntry {
    /// A short human-readable label, `YYYY-mm-dd HH:MM:SS.mmm` (UTC, the
    /// format the backup names carry).
    pub fn display_label(&self) -> String {
        let mut parts = self.label.splitn(2, '-');
        let date = parts.next().unwrap_or("");
        let time = parts.next().unwrap_or("");
        // New backups append a uniqueness suffix after the timestamp.
        let time = time.get(..10).unwrap_or(time);
        // Only slice ASCII timestamp digits: arbitrary backup filenames are data.
        if date.len() == 8
            && date.bytes().all(|b| b.is_ascii_digit())
            && time.len() == 10
            && time.as_bytes()[6] == b'-'
            && time
                .bytes()
                .enumerate()
                .all(|(i, b)| i == 6 || b.is_ascii_digit())
        {
            return format!(
                "{}-{}-{} {}:{}:{}.{}",
                &date[0..4],
                &date[4..6],
                &date[6..8],
                &time[0..2],
                &time[2..4],
                &time[4..6],
                &time[7..10]
            );
        }
        self.label.clone()
    }
}

/// P2-16 (baseline 17.4): every valid historical backup, newest first.
///
/// A backup is listed only when its content parses as JSON (baseline 15.8.3
/// "validate JSON"): a truncated or hand-edited file must never be offered
/// as a restore source.
pub fn list_backups(target: &Path) -> Vec<BackupEntry> {
    let Ok((directory, name)) = split_target(target) else {
        return Vec::new();
    };
    let stem = name.strip_suffix(".json").unwrap_or(&name);
    let prefix = format!("{stem}.backup.");
    let Ok(entries) = fs::read_dir(&directory) else {
        return Vec::new();
    };
    let mut backups: Vec<BackupEntry> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter_map(|path| {
            let file_name = path.file_name()?.to_string_lossy().into_owned();
            let label = file_name
                .strip_prefix(&prefix)?
                .strip_suffix(".json")?
                .to_owned();
            let content = fs::read_to_string(&path).ok()?;
            parse_supported_library(&content)?;
            let bytes = fs::metadata(&path).ok()?.len();
            Some(BackupEntry { path, label, bytes })
        })
        .collect();
    backups.sort_unstable_by(|left, right| right.label.cmp(&left.label));
    backups
}

/// P2-16 (baseline 17.4): restores one historical backup into the live file.
///
/// The backup is validated first, then written through the same atomic
/// replace path a normal save uses, so the configuration that is being
/// replaced is itself kept as a backup and the restore stays reversible.
/// Old backups are pruned afterwards (baseline 15.7: keep the last 10).
pub fn restore_backup(target: &Path, backup: &Path) -> Result<PathBuf, SaveError> {
    restore_backup_with_library(target, backup).map(|(path, _)| path)
}

/// Return the exact validated snapshot committed to disk, avoiding a second
/// read in the controller that could observe a different backup revision.
pub(crate) fn restore_backup_with_library(
    target: &Path,
    backup: &Path,
) -> Result<(PathBuf, crate::library_model::Library), SaveError> {
    let Some(_guard) = acquire_write_lock() else {
        return Err(SaveError::Busy);
    };
    let content =
        fs::read_to_string(backup).map_err(|os_error| SaveError::TempFile { os_error })?;
    let library = parse_supported_library(&content).ok_or(SaveError::InvalidBackup)?;
    let outcome = save_atomic_locked(target, &content)?;
    prune_backups_locked(target, 10);
    let replaced = match outcome {
        SaveOutcome::Created => backup.to_path_buf(),
        SaveOutcome::Replaced(replaced) => replaced.backup,
    };
    Ok((replaced, library))
}

/// cargo test runs tests in parallel threads; every test that touches the
/// process-global write lock (directly or through `save_atomic`/recovery)
/// serializes on this mutex, including controller-level tests.
#[cfg(test)]
pub(crate) static TEST_SERIALIZER: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_SERIALIZER
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::CreateFileW;

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        super::test_lock()
    }

    fn temp_dir(name: &str) -> PathBuf {
        let base =
            std::env::temp_dir().join(format!("quicknest-save-{}-{}", name, fallback_unique()));
        fs::create_dir_all(&base).expect("create temp dir");
        base
    }

    fn read_config(target: &Path) -> String {
        fs::read_to_string(target).expect("read config")
    }

    fn leftover_temps(directory: &Path) -> Vec<PathBuf> {
        fs::read_dir(directory)
            .expect("read dir")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().map(|ext| ext == "tmp").unwrap_or(false))
            .collect()
    }

    #[test]
    fn all_backup_entry_points_share_bom_structure_and_schema_validation() {
        let _serialization = test_guard();
        let directory = temp_dir("validation");
        let target = directory.join("config.json");
        let good = format!(
            "\u{feff}{}",
            serde_json::to_string(&crate::library_model::Library::default()).unwrap()
        );
        let usable = directory.join("config.backup.20260101-000000-000.json");
        fs::write(&usable, &good).unwrap();
        let invalid = [
            "{}".to_string(),
            "[]".to_string(),
            "not json".to_string(),
            "{\"schema_version\":3,\"groups\":[]}".to_string(),
            "{\"schema_version\":2,\"groups\":[{}]}".to_string(),
        ];
        fs::write(&target, "{}").unwrap(); // valid JSON, unusable Library
        for (index, content) in invalid.iter().enumerate() {
            let path = directory.join(format!(
                "config.backup.2026010{}-000000-000.json",
                index + 2
            ));
            fs::write(&path, content).unwrap();
            assert!(matches!(
                restore_backup(&target, &path),
                Err(SaveError::InvalidBackup)
            ));
            assert_eq!(read_config(&target), "{}");
        }
        assert_eq!(
            list_backups(&target)
                .iter()
                .map(|e| &e.path)
                .collect::<Vec<_>>(),
            vec![&usable]
        );
        let report = recover_latest_backup(&target).unwrap();
        assert_eq!(report.restored_from.as_ref(), Some(&usable));
        assert_eq!(read_config(&target), good);
        assert!(matches!(
            recover_latest_backup(&target),
            Err(SaveError::TargetAlreadyValid)
        ));
        assert_eq!(prune_backups(&target, 1), invalid.len());
        assert_eq!(list_backups(&target).len(), 1);
        // Manual restore uses exactly the same BOM-aware validation.
        restore_backup(&target, &usable).unwrap();
        assert_eq!(read_config(&target), good);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn repeated_manual_backups_are_unique_and_invalid_source_is_rejected() {
        let _serialization = test_guard();
        let directory = temp_dir("unique-backups");
        let target = directory.join("config.json");
        fs::write(
            &target,
            serde_json::to_string(&crate::library_model::Library::default()).unwrap(),
        )
        .unwrap();
        let first = backup_now(&target).unwrap();
        let second = backup_now(&target).unwrap();
        assert_ne!(first, second);
        assert!(first.exists() && second.exists());
        fs::write(&target, "{}").unwrap();
        assert!(matches!(backup_now(&target), Err(SaveError::InvalidBackup)));
        assert_eq!(list_backups(&target).len(), 2);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn restore_failure_preserves_live_config_and_backup() {
        let _serialization = test_guard();
        let directory = temp_dir("restore-locked");
        let target = directory.join("config.json");
        let backup = directory.join("config.backup.20260101-000000-000.json");
        let live = serde_json::to_string(&crate::library_model::Library::default()).unwrap();
        fs::write(&target, &live).unwrap();
        fs::write(&backup, &live).unwrap();
        let lock = DeleteLock::acquire(&target);
        assert!(matches!(
            restore_backup(&target, &backup),
            Err(SaveError::ReplaceFailed { .. })
        ));
        drop(lock);
        assert_eq!(read_config(&target), live);
        assert_eq!(read_config(&backup), live);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn first_creation_moves_temp_into_place_without_leftovers() {
        let _serialization = test_guard();
        let directory = temp_dir("first");
        let target = directory.join("config.json");
        let outcome = save_atomic(&target, "{\"v\":1}").expect("first save");
        assert_eq!(outcome, SaveOutcome::Created);
        assert_eq!(read_config(&target), "{\"v\":1}");
        assert!(leftover_temps(&directory).is_empty(), "no tmp leftovers");
        fs::remove_dir_all(&directory).expect("cleanup");
    }

    #[test]
    fn replacement_backs_up_previous_content_without_leftovers() {
        let _serialization = test_guard();
        let directory = temp_dir("replace");
        let target = directory.join("config.json");
        save_atomic(&target, "{\"v\":1}").expect("seed config");
        let outcome = save_atomic(&target, "{\"v\":2}").expect("replace save");
        let SaveOutcome::Replaced(replaced) = outcome else {
            panic!("expected Replaced, got {outcome:?}");
        };
        assert_eq!(read_config(&target), "{\"v\":2}");
        assert_eq!(
            fs::read_to_string(&replaced.backup).expect("read backup"),
            "{\"v\":1}",
            "backup must hold the previous content"
        );
        assert!(
            replaced
                .backup
                .file_name()
                .expect("backup name")
                .to_string_lossy()
                .contains(".backup.")
        );
        assert!(leftover_temps(&directory).is_empty(), "no tmp leftovers");
        fs::remove_dir_all(&directory).expect("cleanup");
    }

    /// A real Windows failure-injection lock: CreateFileW with FILE_SHARE_READ
    /// only, so ReplaceFileW (which needs delete access) must fail. Rust std's
    /// File::open shares everything including DELETE, which would not lock.
    struct DeleteLock(HANDLE);

    impl DeleteLock {
        fn acquire(target: &Path) -> Self {
            const GENERIC_READ: u32 = 0x8000_0000;
            const FILE_SHARE_READ: u32 = 0x0000_0001;
            const OPEN_EXISTING: u32 = 3;
            let wide = to_wide(target);
            // SAFETY: Null-terminated wide path; the returned handle is closed
            // by the Drop impl on the same thread.
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
            assert!(!std::ptr::eq(handle, INVALID_HANDLE_VALUE) && !handle.is_null());
            Self(handle)
        }
    }

    impl Drop for DeleteLock {
        fn drop(&mut self) {
            // SAFETY: The handle came from CreateFileW above and was not used
            // anywhere else.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    #[test]
    fn replace_failure_keeps_target_and_temp_for_diagnostics() {
        let _serialization = test_guard();
        let directory = temp_dir("failure");
        let target = directory.join("config.json");
        save_atomic(&target, "{\"v\":10}").expect("seed config");

        let lock = DeleteLock::acquire(&target);
        let error = save_atomic(&target, "{\"v\":11}")
            .expect_err("save must fail while the target is locked");
        drop(lock);

        let SaveError::ReplaceFailed {
            win32_error,
            temp_path,
        } = error
        else {
            panic!("expected ReplaceFailed, got {error:?}");
        };
        assert_eq!(read_config(&target), "{\"v\":10}", "previous config intact");
        assert_eq!(win32_error, 32, "ERROR_SHARING_VIOLATION");
        assert!(temp_path.exists(), "temp kept for diagnostics");
        assert_eq!(
            fs::read_to_string(&temp_path).expect("read kept temp"),
            "{\"v\":11}",
            "kept temp holds the failed write"
        );
        // Diagnostics done: the caller cleans the kept temp; verify that too.
        fs::remove_file(&temp_path).expect("remove kept temp");
        assert!(leftover_temps(&directory).is_empty(), "clean after removal");
        fs::remove_dir_all(&directory).expect("cleanup");
    }

    #[test]
    fn concurrent_creation_falls_back_to_replacing_whatever_appeared() {
        let _serialization = test_guard();
        let directory = temp_dir("concurrent");
        let target = directory.join("config.json");
        // Simulate the concurrent creator: pre-create the target, then run
        // the internal chain the way the race path does — temp written, move
        // refuses, replace wins and backs up the raced file.
        fs::write(&target, "{\"raced\":true}").expect("simulate concurrent creator");
        let temp_path = write_temp(&target, "{\"v\":99}").expect("write temp");
        let moved = move_into_place(&temp_path, &target).expect("move attempt");
        assert!(!moved, "move must refuse an existing target");
        let backup = backup_path_for(&directory, "config.json");
        replace_via_replacefile(&target, &temp_path, &backup).expect("fallback replace");
        assert_eq!(
            fs::read_to_string(&backup).expect("read backup"),
            "{\"raced\":true}",
            "the raced file must be backed up, not lost"
        );
        assert_eq!(read_config(&target), "{\"v\":99}");
        fs::remove_dir_all(&directory).expect("cleanup");
    }

    #[test]
    fn missing_directory_is_reported_without_touching_anything() {
        let _serialization = test_guard();
        let directory = temp_dir("missing");
        let target = directory.join("no-such-subdir").join("config.json");
        // Controller tests legitimately hold the process-wide write lock while
        // persisting; retry Busy so this assertion is about the missing
        // directory, not test scheduling.
        let error = loop {
            match save_atomic(&target, "{}") {
                Err(SaveError::Busy) => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                other => break other.expect_err("must fail"),
            }
        };
        assert!(matches!(error, SaveError::MissingDirectory(_)));
        fs::remove_dir_all(&directory).expect("cleanup");
    }

    #[test]
    fn backup_timestamp_names_sort_chronologically() {
        let _serialization = test_guard();
        let older = timestamp_string(UNIX_EPOCH + std::time::Duration::from_secs(1_000));
        let newer = timestamp_string(UNIX_EPOCH + std::time::Duration::from_secs(2_000));
        assert!(older < newer, "{older} must sort before {newer}");
    }

    /// Seeds backups plus a corrupt live config. `stamps` entries are
    /// (stamp, Some(valid json) | None for broken json).
    fn seed_backups(directory: &Path, target: &Path, stamps: &[(&str, Option<&str>)]) {
        for (stamp, content) in stamps {
            let path = directory.join(format!("config.backup.{stamp}.json"));
            match content {
                Some(valid) => fs::write(&path, valid).expect("write backup"),
                None => fs::write(&path, "not json at all").expect("write broken backup"),
            }
        }
        fs::write(target, "{broken").expect("write corrupt config");
    }

    #[test]
    fn recovery_restores_latest_valid_backup_and_renames_corrupt_file() {
        let _serialization = test_guard();
        let directory = temp_dir("recover");
        let target = directory.join("config.json");
        seed_backups(
            &directory,
            &target,
            &[
                (
                    "20261103-000000-000",
                    Some("{\"schema_version\":2,\"groups\":[],\"gen\":1}"),
                ),
                (
                    "20261104-000000-000",
                    Some("{\"schema_version\":2,\"groups\":[],\"gen\":2}"),
                ),
                ("20261104-120000-000", None), // newest but broken
            ],
        );
        let report = recover_latest_backup(&target).expect("recovery");
        assert_eq!(
            report.restored_from,
            Some(directory.join("config.backup.20261104-000000-000.json")),
            "newest VALID backup wins"
        );
        assert_eq!(
            read_config(&target),
            "{\"schema_version\":2,\"groups\":[],\"gen\":2}"
        );
        let corrupted = report.corrupted_path.expect("corrupt file renamed");
        assert!(
            corrupted.to_string_lossy().contains(".corrupted."),
            "corrupt file renamed: {}",
            corrupted.display()
        );
        assert_eq!(
            fs::read_to_string(&corrupted).expect("read renamed"),
            "{broken"
        );
        fs::remove_dir_all(&directory).expect("cleanup");
    }

    #[test]
    fn recovery_without_valid_backup_enters_safe_mode_untouched() {
        let _serialization = test_guard();
        let directory = temp_dir("safe");
        let target = directory.join("config.json");
        seed_backups(
            &directory,
            &target,
            &[("20261103-000000-000", None), ("20261104-000000-000", None)],
        );
        let report = recover_latest_backup(&target).expect("recovery report");
        assert_eq!(report.restored_from, None, "nothing restored");
        assert_eq!(report.corrupted_path, None, "nothing renamed");
        assert_eq!(
            fs::read_to_string(&target).expect("config untouched"),
            "{broken",
            "safe mode must not overwrite the original"
        );
        fs::remove_dir_all(&directory).expect("cleanup");
    }

    #[test]
    fn recovery_skips_backups_that_fail_content_read() {
        let _serialization = test_guard();
        let directory = temp_dir("unreadable");
        let target = directory.join("config.json");
        seed_backups(
            &directory,
            &target,
            &[(
                "20261104-000000-000",
                Some("{\"schema_version\":2,\"groups\":[],\"gen\":1}"),
            )],
        );
        // Shadow the only backup with a directory to force a read failure
        // path without needing ACL games.
        let backup = directory.join("config.backup.20261104-000000-000.json");
        fs::remove_file(&backup).expect("remove backup");
        fs::create_dir(&backup).expect("shadow backup with a directory");
        let report = recover_latest_backup(&target).expect("recovery report");
        assert_eq!(report.restored_from, None);
        assert_eq!(
            fs::read_to_string(&target).expect("config untouched"),
            "{broken"
        );
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn recovery_refuses_a_still_valid_config() {
        let _serialization = test_guard();
        let directory = temp_dir("valid-target");
        let target = directory.join("config.json");
        seed_backups(
            &directory,
            &target,
            &[(
                "20261104-000000-000",
                Some("{\"schema_version\":2,\"groups\":[],\"gen\":1}"),
            )],
        );
        fs::write(
            &target,
            "{\"schema_version\":2,\"groups\":[],\"still\": \"valid\"}",
        )
        .expect("write valid config");
        let error = recover_latest_backup(&target).expect_err("must refuse");
        assert!(matches!(error, SaveError::TargetAlreadyValid));
        fs::remove_dir_all(&directory).expect("cleanup");
    }

    #[test]
    fn recovery_holds_the_write_lock_against_concurrent_saves() {
        let _serialization = test_guard();
        let directory = temp_dir("lock");
        let target = directory.join("config.json");
        seed_backups(
            &directory,
            &target,
            &[(
                "20261104-000000-000",
                Some("{\"schema_version\":2,\"groups\":[],\"gen\":1}"),
            )],
        );
        // The single-writer rule (baseline 15.7), deterministically: while any
        // write transaction — recovery uses the same lock — holds the write
        // lock, a concurrent save must be rejected with Busy. A timing race
        // against a real recovery would be flaky (fast machines finish the
        // recovery before the racer wakes), so the held-lock side is simulated
        // by taking the same lock recovery takes.
        let guard = acquire_write_lock().expect("take the write lock");
        let raced = save_atomic(&target, "{\"raced\":1}");
        assert!(
            matches!(raced, Err(SaveError::Busy)),
            "concurrent save during a write transaction must be rejected, got {raced:?}"
        );
        drop(guard);
        // After the transaction ends the lock is free again.
        let after = save_atomic(&target, "{\"after\":1}");
        assert!(after.is_ok(), "save must succeed once the lock is free");
        // And the recovery path still works normally around it. Recovery may
        // briefly race another test's write transaction; retry Busy.
        fs::write(&target, "{broken").expect("re-corrupt config");
        let report = loop {
            match recover_latest_backup(&target) {
                Err(SaveError::Busy) => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                other => break other.expect("recovery after free"),
            }
        };
        assert!(report.restored_from.is_some());
        assert_eq!(
            read_config(&target),
            "{\"schema_version\":2,\"groups\":[],\"gen\":1}"
        );
        fs::remove_dir_all(&directory).expect("cleanup");
    }

    fn backup_names(directory: &Path) -> Vec<String> {
        fs::read_dir(directory)
            .expect("read dir")
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
            .filter(|name| name.starts_with("config.backup.") && name.ends_with(".json"))
            .collect()
    }

    #[test]
    fn pruning_keeps_the_ten_newest_valid_backups() {
        let _serialization = test_guard();
        let directory = temp_dir("prune");
        let target = directory.join("config.json");
        // Twelve recent valid backups plus an older valid one and a broken
        // one: invalid entries are always removed, and only ten valid remain.
        let mut stamps: Vec<(String, Option<&str>)> = (0..12)
            .map(|index| {
                (
                    format!("20261104-0000{index:02}-000"),
                    Some("{\"schema_version\":2,\"groups\":[],\"gen\":1}"),
                )
            })
            .collect();
        stamps.push((
            "20261103-000000-000".to_string(),
            Some("{\"schema_version\":2,\"groups\":[],\"old\":1}"),
        ));
        stamps.push(("20261102-000000-000".to_string(), Some("{broken")));
        let borrowed: Vec<(&str, Option<&str>)> = stamps
            .iter()
            .map(|(stamp, content)| (stamp.as_str(), *content))
            .collect();
        seed_backups(&directory, &target, &borrowed);
        let removed = prune_backups(&target, 10);
        assert!(
            removed >= 3,
            "invalid and overflow entries removed: {removed}"
        );
        let remaining = backup_names(&directory);
        assert_eq!(remaining.len(), 10, "ten valid backups kept");
        assert!(
            remaining.iter().any(|name| name.contains("000011-000")),
            "newest backup kept"
        );
        assert!(
            !remaining.iter().any(|name| name.contains("000000-000")),
            "oldest overflow removed"
        );
        fs::remove_dir_all(&directory).expect("cleanup");
    }

    #[test]
    fn manual_backups_match_the_recovery_pattern() {
        let _serialization = test_guard();
        let directory = temp_dir("manual");
        let target = directory.join("config.json");
        fs::write(&target, "{\"schema_version\":2,\"groups\":[],\"manual\":1}")
            .expect("write config");
        let backup = backup_now(&target).expect("manual backup");
        assert!(backup.exists());
        assert_eq!(
            read_config(&backup),
            "{\"schema_version\":2,\"groups\":[],\"manual\":1}"
        );
        let name = backup
            .file_name()
            .and_then(|name| name.to_str())
            .expect("name");
        assert!(name.starts_with("config.backup.") && name.ends_with(".json"));
        fs::remove_dir_all(&directory).expect("cleanup");
    }

    // P2-16: the restore list only offers backups whose content parses and
    // keeps them newest first (baseline 15.8.3/17.4).
    #[test]
    fn restore_list_skips_broken_backups_and_sorts_newest_first() {
        let _serialization = test_guard();
        let directory = temp_dir("list");
        let target = directory.join("config.json");
        fs::write(
            &target,
            "{\"schema_version\":2,\"groups\":[],\"current_group_id\":\"live\"}",
        )
        .expect("write config");
        fs::write(
            directory.join("config.backup.20261101-000000-000.json"),
            "{\"schema_version\":2,\"groups\":[],\"current_group_id\":\"older\"}",
        )
        .expect("older backup");
        fs::write(
            directory.join("config.backup.20261102-000000-000.json"),
            "{\"schema_version\":2,\"groups\":[],\"current_group_id\":\"newer\"}",
        )
        .expect("newer backup");
        fs::write(
            directory.join("config.backup.20261103-000000-000.json"),
            "not json",
        )
        .expect("broken backup");
        let entries = list_backups(&target);
        assert_eq!(entries.len(), 2, "the broken backup is not offered");
        assert_eq!(entries[0].label, "20261102-000000-000");
        assert_eq!(entries[1].label, "20261101-000000-000");
        assert!(entries[0].bytes > 0);
        assert_eq!(entries[0].display_label(), "2026-11-02 00:00:00.000");
        fs::remove_dir_all(&directory).expect("cleanup");
    }

    // P2-16: a restore replaces the live config, keeps the replaced content
    // as a backup (reversible) and prunes the retention window.
    #[test]
    fn restore_replaces_the_live_config_and_keeps_the_previous_one() {
        let _serialization = test_guard();
        let directory = temp_dir("restore");
        let target = directory.join("config.json");
        fs::write(
            &target,
            "{\"schema_version\":2,\"groups\":[],\"current_group_id\":\"current\"}",
        )
        .expect("write config");
        let backup = directory.join("config.backup.20261101-000000-000.json");
        fs::write(
            &backup,
            "{\"schema_version\":2,\"groups\":[],\"current_group_id\":\"restored\"}",
        )
        .expect("write backup");
        let kept = restore_backup(&target, &backup).expect("restore");
        assert_eq!(
            fs::read_to_string(&target).expect("read config"),
            "{\"schema_version\":2,\"groups\":[],\"current_group_id\":\"restored\"}",
            "the backup content is the new live config"
        );
        assert_eq!(
            fs::read_to_string(&kept).expect("read kept backup"),
            "{\"schema_version\":2,\"groups\":[],\"current_group_id\":\"current\"}",
            "the replaced config is kept as a backup"
        );
        assert!(kept.exists());
        fs::remove_dir_all(&directory).expect("cleanup");
    }

    // P2-16: a backup that does not parse must never be restored.
    #[test]
    fn restore_refuses_a_broken_backup_without_touching_the_config() {
        let _serialization = test_guard();
        let directory = temp_dir("restore-broken");
        let target = directory.join("config.json");
        fs::write(
            &target,
            "{\"schema_version\":2,\"groups\":[],\"current_group_id\":\"live\"}",
        )
        .expect("write config");
        let backup = directory.join("config.backup.20261101-000000-000.json");
        fs::write(&backup, "not json").expect("write broken backup");
        let error = restore_backup(&target, &backup).expect_err("restore must fail");
        assert!(matches!(error, SaveError::InvalidBackup));
        assert_eq!(
            fs::read_to_string(&target).expect("read config"),
            "{\"schema_version\":2,\"groups\":[],\"current_group_id\":\"live\"}",
            "the live config is untouched"
        );
        fs::remove_dir_all(&directory).expect("cleanup");
    }
}
