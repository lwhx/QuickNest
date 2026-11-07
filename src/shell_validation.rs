use std::cell::RefCell;
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use quicknest::infrastructure::windows::ole::{CO_E_NOTINITIALIZED, apartment};
use quicknest::infrastructure::windows::shell::{ShellLinkData, ShellOutcome, ShellService};

const RESOLVE_TIMEOUT: Duration = Duration::from_secs(10);
const EXECUTE_TIMEOUT: Duration = Duration::from_secs(10);

struct Report {
    file: File,
    started: std::time::Instant,
}

impl Report {
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

fn describe(outcome: &ShellOutcome) -> String {
    match outcome {
        ShellOutcome::Link(data) => format!(
            "path={:?} idlist={:?} args={:?} workdir={:?} icon={:?} index={}",
            data.path,
            data.id_list_path,
            data.arguments,
            data.working_directory,
            data.icon_location,
            data.icon_index
        ),
        ShellOutcome::Executed => "executed".into(),
        ShellOutcome::Failed { hresult } => format!("failed=0x{:08X}", hresult),
        ShellOutcome::TimedOut => "timed-out".into(),
        ShellOutcome::Unavailable => "unavailable".into(),
    }
}

fn require(ok: bool, error: &str) -> Result<(), Box<dyn Error>> {
    if ok { Ok(()) } else { Err(error.into()) }
}

fn fixture_directory(report: &mut Report) -> Result<PathBuf, Box<dyn Error>> {
    let base = std::env::temp_dir().join(format!("quicknest-p0-07-{}", std::process::id()));
    fs::create_dir_all(&base)?;
    let target = base.join("sample target.txt");
    fs::write(&target, "QuickNest Shell STA validation fixture")?;
    report.record("fixture", &format!("dir={:?} target={:?}", base, target))?;
    Ok(base)
}

fn real_link() -> Option<PathBuf> {
    let candidates = [
        r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs",
        r"C:\Users\Public\Desktop",
    ];
    for directory in candidates {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) == Some("lnk") {
                return Some(path);
            }
        }
    }
    None
}

fn expect_link(
    outcome: ShellOutcome,
    phase: &str,
    report: &mut Report,
) -> Result<ShellLinkData, Box<dyn Error>> {
    report.record(phase, &describe(&outcome))?;
    match outcome {
        ShellOutcome::Link(data) => Ok(data),
        other => Err(format!("Unexpected {} outcome: {other:?}", phase).into()),
    }
}

pub fn run(path: &Path) -> Result<(), Box<dyn Error>> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    writeln!(file, "elapsed_ms\tphase\tdetail")?;
    file.flush()?;
    let report = Rc::new(RefCell::new(Report {
        file,
        started: std::time::Instant::now(),
    }));

    let ui_before = apartment();
    report.borrow_mut().record(
        "ui_apartment_before",
        &format!(
            "hresult=0x{:08X} kind={} initialized={}",
            ui_before.hresult as u32,
            ui_before.kind,
            ui_before.hresult != CO_E_NOTINITIALIZED
        ),
    )?;

    let service = ShellService::start()?;
    report
        .borrow_mut()
        .record("service_started", &format!("tid={}", service.thread_id()))?;
    let started = report.borrow().started;
    let service_apartment = std::thread::spawn(move || {
        // The report writer is not Send; this probe only confirms a helper thread
        // can coexist with the running STA service without touching shared state.
        let elapsed = started.elapsed().as_millis();
        (
            elapsed,
            std::thread::available_parallelism()
                .map(|value| value.get())
                .unwrap_or(1),
        )
    });
    let (probe_elapsed, probe_threads) = match service_apartment.join() {
        Ok(value) => value,
        Err(_) => return Err("Service apartment probe panicked".into()),
    };
    report.borrow_mut().record(
        "service_thread_probe",
        &format!("probe_elapsed_ms={probe_elapsed} logical_cpus={probe_threads}"),
    )?;

    let base = fixture_directory(&mut report.borrow_mut())?;
    let target = base.join("sample target.txt");
    let link_path = base.join("generated.lnk");

    let created = service.create_link(
        &target,
        &base,
        &target.to_string_lossy(),
        0,
        &link_path,
        RESOLVE_TIMEOUT,
    );
    report
        .borrow_mut()
        .record("create_link", &describe(&created))?;
    require(
        matches!(created, ShellOutcome::Executed),
        "IPersistFile::Save did not create the .lnk",
    )?;
    require(link_path.exists(), "Link file missing after Save")?;

    let resolution = service.resolve(&link_path, RESOLVE_TIMEOUT);
    let data = expect_link(resolution, "generated_resolve", &mut report.borrow_mut())?;
    // The shell expands short (8.3) path forms; compare file identity, not strings.
    let expected_target = std::fs::canonicalize(&target)?;
    let expected_base = std::fs::canonicalize(&base)?;
    let same_file = |value: Option<&Path>, expected: &Path| -> bool {
        value
            .and_then(|resolved| std::fs::canonicalize(resolved).ok())
            .is_some_and(|actual| actual == expected)
    };
    require(
        same_file(data.path.as_deref(), &expected_target),
        "Generated link did not resolve to the fixture target",
    )?;
    require(
        std::fs::canonicalize(&data.working_directory)
            .map(|actual| actual == expected_base)
            .unwrap_or(false),
        "Generated link working directory mismatch",
    )?;
    require(
        !data.icon_location.is_empty(),
        "Generated link icon location empty",
    )?;
    require(
        same_file(data.id_list_path.as_deref(), &expected_target),
        "GetIDList path did not match the target",
    )?;

    if let Some(existing) = real_link() {
        let outcome = service.resolve(&existing, RESOLVE_TIMEOUT);
        let data = expect_link(outcome, "real_link_resolve", &mut report.borrow_mut())?;
        match data.path {
            Some(resolved) if resolved.exists() => {}
            _ => return Err("Real system link did not resolve to an existing target".into()),
        }
    } else {
        report
            .borrow_mut()
            .record("real_link_resolve", "no system .lnk sample found; skipped")?;
    }

    let executed = service.execute_folder(&base, EXECUTE_TIMEOUT);
    report
        .borrow_mut()
        .record("execute_folder", &describe(&executed))?;
    require(
        matches!(executed, ShellOutcome::Executed),
        "ShellExecuteExW did not report success",
    )?;

    let ui_after = apartment();
    report.borrow_mut().record(
        "ui_apartment_after",
        &format!(
            "hresult=0x{:08X} kind={}",
            ui_after.hresult as u32, ui_after.kind
        ),
    )?;
    require(
        ui_after.hresult == ui_before.hresult,
        "UI thread apartment changed across Shell STA use",
    )?;

    drop(service);
    report
        .borrow_mut()
        .record("service_stopped", "shutdown without hanging")?;

    let restarted = ShellService::start()?;
    report.borrow_mut().record(
        "service_restarted",
        &format!("tid={}", restarted.thread_id()),
    )?;
    let again = restarted.resolve(&link_path, RESOLVE_TIMEOUT);
    require(
        matches!(again, ShellOutcome::Link(_)),
        "Restarted service could not resolve again",
    )?;
    report
        .borrow_mut()
        .record("restart_resolve", &describe(&again))?;
    drop(restarted);
    report
        .borrow_mut()
        .record("completed", "all shell STA phases passed")?;
    let _ = fs::remove_dir_all(&base);
    Ok(())
}
