use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use quicknest::infrastructure::windows::single_instance::{
    AcquireOutcome, acquire, mutex_name, open_existing, wait_and_release, wait_event_name,
};

const CHILD_TIMEOUT: Duration = Duration::from_secs(20);
const CRASH_EXIT_CODE: i32 = 70;

struct Report {
    file: File,
    started: Instant,
}

impl Report {
    fn open(path: &Path) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            file: open_append(path)?,
            started: Instant::now(),
        })
    }

    fn record(&mut self, role: &str, phase: &str, detail: &str) -> Result<(), Box<dyn Error>> {
        writeln!(
            self.file,
            "{}\t{}\t{}\t{}",
            self.started.elapsed().as_millis(),
            role,
            phase,
            detail
        )?;
        self.file.flush()?;
        Ok(())
    }
}

fn open_append(path: &Path) -> Result<File, Box<dyn Error>> {
    Ok(OpenOptions::new().create(true).append(true).open(path)?)
}

fn describe_outcome(outcome: &AcquireOutcome) -> &'static str {
    match outcome {
        AcquireOutcome::FirstInstance => "FIRST_INSTANCE",
        AcquireOutcome::TookOverAbandoned => "TOOK_OVER_ABANDONED",
        AcquireOutcome::AlreadyRunning => "ALREADY_RUNNING",
        AcquireOutcome::Failed(_) => "FAILED",
    }
}

fn exe_path() -> Result<PathBuf, Box<dyn Error>> {
    std::env::current_exe().map_err(Into::into)
}

fn spawn_exe(arguments: &[String]) -> Result<u32, Box<dyn Error>> {
    Command::new(exe_path()?)
        .args(arguments)
        .spawn()
        .map(|child| child.id())
        .map_err(Into::into)
}

/// Second-instance probe: acquires, records the outcome, exits.
pub fn run_child(path: &Path) -> Result<(), Box<dyn Error>> {
    let mut report = Report::open(path)?;
    let (outcome, guard) = acquire().map_err(|error| format!("acquire failed: {error}"))?;
    report.record("child", "acquired", describe_outcome(&outcome))?;
    let wait = match outcome {
        AcquireOutcome::AlreadyRunning => WAIT_TIMEOUT_NAME,
        AcquireOutcome::FirstInstance => WAIT_OBJECT_NAME,
        AcquireOutcome::TookOverAbandoned => WAIT_ABANDONED_NAME,
        AcquireOutcome::Failed(_) => WAIT_FAILED_NAME,
    };
    report.record("child", "probe", wait)?;
    drop(guard);
    report.record("child", "released", "guard dropped")?;
    Ok(())
}

/// Keeper: opens the mutex by name, waits for the owner to die, records evidence.
pub fn run_keeper(name: &str, path: &Path) -> Result<(), Box<dyn Error>> {
    let mut report = Report::open(path)?;
    let handle = open_existing(name).map_err(|error| format!("OpenMutexW failed: {error}"))?;
    report.record("keeper", "opened", name)?;
    let wait = unsafe { wait_and_release(handle) };
    report.record("keeper", "wait", wait_event_name(wait))?;
    if wait == WAIT_ABANDONED_CODE {
        report.record("keeper", "abandoned_takeover", "owner died without release")?;
    } else {
        return Err(format!("Keeper observed {wait}, not an abandoned mutex").into());
    }
    // SAFETY: Keeper owns the handle from OpenMutexW.
    unsafe {
        windows_sys::Win32::Foundation::CloseHandle(handle);
    }
    report.record("keeper", "completed", "object released for destruction")?;
    Ok(())
}

/// Parent scenario: FIRST_INSTANCE → child ALREADY_RUNNING → simulated crash →
/// keeper observes WAIT_ABANDONED takeover. Exits with CRASH_EXIT_CODE to skip
/// Drop and leave the mutex abandoned exactly like a crashed process.
pub fn run(path: &Path) -> Result<(), Box<dyn Error>> {
    let file = File::options().write(true).create_new(true).open(path)?;
    let mut report = Report {
        file,
        started: Instant::now(),
    };
    let name = mutex_name().map_err(|error| format!("mutex_name failed: {error}"))?;
    report.record("parent", "mutex_name", &name)?;

    let (outcome, _guard) = acquire().map_err(|error| format!("acquire failed: {error}"))?;
    if outcome != AcquireOutcome::FirstInstance {
        return Err(format!(
            "Expected FIRST_INSTANCE, got {}",
            describe_outcome(&outcome)
        )
        .into());
    }
    report.record("parent", "acquired", describe_outcome(&outcome))?;

    let child_report = path.with_extension("child.tsv");
    let child_id = spawn_exe(&[
        "--mutex-child".to_string(),
        child_report.to_string_lossy().into_owned(),
    ])?;
    wait_for_line(&child_report, "acquired", CHILD_TIMEOUT)?;
    report.record("parent", "child_probe", &format!("pid={child_id}"))?;

    // Keeper opens the object before the simulated crash so the abandoned mutex
    // has a live handle and can be observed deterministically.
    let keeper_report = path.with_extension("keeper.tsv");
    let keeper_id = spawn_exe(&[
        "--mutex-keeper".to_string(),
        name.clone(),
        keeper_report.to_string_lossy().into_owned(),
    ])?;
    wait_for_line(&keeper_report, "opened", CHILD_TIMEOUT)?;
    report.record("parent", "keeper_ready", &format!("pid={keeper_id}"))?;

    report.record(
        "parent",
        "simulated_crash",
        &format!("exit={CRASH_EXIT_CODE}"),
    )?;
    // Dropping `guard` here would release the mutex; skipping Drop via process exit
    // reproduces exactly what a crashed owner leaves behind.
    std::process::exit(CRASH_EXIT_CODE);
}

fn wait_for_line(path: &Path, marker: &str, timeout: Duration) -> Result<String, Box<dyn Error>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines().rev() {
                if line.contains(marker) {
                    return Ok(line.to_string());
                }
            }
        }
        if Instant::now() >= deadline {
            return Err(format!("Timed out waiting for '{marker}' in {}", path.display()).into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

const WAIT_OBJECT_NAME: &str = "WAIT_OBJECT_0";
const WAIT_ABANDONED_NAME: &str = "WAIT_ABANDONED";
const WAIT_TIMEOUT_NAME: &str = "WAIT_TIMEOUT";
const WAIT_FAILED_NAME: &str = "WAIT_FAILED";
const WAIT_ABANDONED_CODE: u32 = 128;
