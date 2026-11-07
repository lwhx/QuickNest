use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use quicknest::infrastructure::windows::ipc::{
    CONNECT_BUDGET_MS, IpcCommand, IpcMessage, PipeServer, send_message, send_raw,
};
use quicknest::infrastructure::windows::single_instance::{InstanceIdentity, acquire};

const CHILD_TIMEOUT: Duration = Duration::from_secs(30);
const DELAYED_PIPE_START: Duration = Duration::from_millis(400);

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
        thread::sleep(Duration::from_millis(20));
    }
}

fn command_name(command: IpcCommand) -> &'static str {
    match command {
        IpcCommand::Show => "show",
        IpcCommand::Hide => "hide",
        IpcCommand::Toggle => "toggle",
        IpcCommand::ShowAndFocus => "show_and_focus",
        IpcCommand::OpenSearch => "open_search",
    }
}

fn command_by_name(name: &str) -> Result<IpcCommand, Box<dyn Error>> {
    match name {
        "show" => Ok(IpcCommand::Show),
        "hide" => Ok(IpcCommand::Hide),
        "toggle" => Ok(IpcCommand::Toggle),
        "show_and_focus" => Ok(IpcCommand::ShowAndFocus),
        "open_search" => Ok(IpcCommand::OpenSearch),
        _ => Err(format!("Unknown command: {name}").into()),
    }
}

/// Secondary instance: confirm the mutex is held, then connect within the budget.
pub fn run_client(path: &Path, command: &str) -> Result<(), Box<dyn Error>> {
    let mut report = Report::open(path)?;
    let identity = InstanceIdentity::current().map_err(|error| format!("identity: {error}"))?;
    let (outcome, _guard) = acquire().map_err(|error| format!("acquire: {error}"))?;
    report.record(
        "client",
        "mutex",
        match outcome {
            quicknest::infrastructure::windows::single_instance::AcquireOutcome::AlreadyRunning => {
                "ALREADY_RUNNING"
            }
            _ => "unexpected",
        },
    )?;
    let started = Instant::now();
    let message = IpcMessage::new(command_by_name(command)?);
    let accepted = send_message(&identity.pipe_name(), &message)
        .map_err(|error| format!("send: {error:?}"))?;
    let elapsed = started.elapsed().as_millis();
    report.record(
        "client",
        "sent",
        &format!("command={command} accepted={accepted} elapsed_ms={elapsed} budget_ms={CONNECT_BUDGET_MS}"),
    )?;
    if elapsed > CONNECT_BUDGET_MS {
        return Err("Connect exceeded the 1500 ms baseline budget".into());
    }
    Ok(())
}

/// Raw rejection probe: writes a malformed frame and records the ack verdict.
pub fn run_raw_client(path: &Path, frame_kind: &str) -> Result<(), Box<dyn Error>> {
    let mut report = Report::open(path)?;
    let identity = InstanceIdentity::current().map_err(|error| format!("identity: {error}"))?;
    let name = identity.pipe_name();
    let (frame, expect_accepted) = match frame_kind {
        "unknown-command" => (
            serde_json::to_vec(&serde_json::json!({
                "version":1,"command":"cmd.exe /c calc","source":"secondary_instance"
            }))
            .expect("json"),
            false,
        ),
        "wrong-version" => (
            serde_json::to_vec(&serde_json::json!({
                "version":2,"command":"show","source":"secondary_instance"
            }))
            .expect("json"),
            false,
        ),
        "bad-json" => (b"{\"version\":1,\"command\":".to_vec(), false),
        "unknown-field" => (
            serde_json::to_vec(&serde_json::json!({
                "version":1,"command":"show","source":"secondary_instance","args":["x"]
            }))
            .expect("json"),
            false,
        ),
        "oversize-prefix" => ((MAX_PAYLOAD_PREFIX_BYTES + 1).to_le_bytes().to_vec(), false),
        _ => return Err(format!("Unknown raw frame kind: {frame_kind}").into()),
    };
    let verdict = send_raw(&name, &frame).map_err(|error| format!("send_raw: {error:?}"))?;
    report.record(
        "raw-client",
        frame_kind,
        &format!("verdict={verdict:?} expected_accepted={expect_accepted}"),
    )?;
    if verdict != Some(expect_accepted) {
        return Err(format!(
            "Raw frame {frame_kind}: verdict {verdict:?} != expected {expect_accepted}"
        )
        .into());
    }
    Ok(())
}

const MAX_PAYLOAD_PREFIX_BYTES: u32 = 4096;

/// Parent scenario: delayed pipe start exercises the 1500 ms race budget, then
/// all five whitelisted commands are delivered and must arrive in order.
pub fn run(path: &Path) -> Result<(), Box<dyn Error>> {
    let file = File::options().write(true).create_new(true).open(path)?;
    let mut report = Report {
        file,
        started: Instant::now(),
    };
    // Hold the mutex BEFORE creating the pipe, exactly like the real main instance.
    let (_mutex_outcome, _mutex_guard) = acquire().map_err(|error| format!("acquire: {error}"))?;
    report.record("parent", "mutex", "acquired before pipe")?;

    // Simulate slow startup: the pipe appears only after this delay. A secondary
    // instance must retry inside its 1500 ms budget instead of failing.
    let server = thread::spawn(move || -> Result<PipeServer, String> {
        thread::sleep(DELAYED_PIPE_START);
        quicknest::infrastructure::windows::ipc::start_server()
            .map_err(|error| format!("server start: {error:?}"))
    });
    let server = server.join().map_err(|_| "server thread panicked")??;
    report.record("parent", "server_started", server.name())?;

    // Race probe child: confirms the retry budget is honored end to end.
    let race_report = path.with_extension("race.tsv");
    let race_id = spawn_exe(&[
        "--ipc-client".to_string(),
        race_report.to_string_lossy().into_owned(),
        "show_and_focus".to_string(),
    ])?;
    wait_for_line(&race_report, "sent", CHILD_TIMEOUT)?;
    report.record("parent", "race_probe", &format!("pid={race_id}"))?;

    // All five whitelisted commands, in baseline order.
    let commands = ["show", "hide", "toggle", "show_and_focus", "open_search"];
    for command in commands {
        let child_report = path.with_extension(format!("{command}.tsv"));
        let child_id = spawn_exe(&[
            "--ipc-client".to_string(),
            child_report.to_string_lossy().into_owned(),
            command.to_string(),
        ])?;
        wait_for_line(&child_report, "sent", CHILD_TIMEOUT)?;
        report.record(
            "parent",
            "command_sent",
            &format!("{command} pid={child_id}"),
        )?;
    }
    // Wait for the server to dispatch everything, then assert the order.
    let deadline = Instant::now() + CHILD_TIMEOUT;
    let mut received: Vec<IpcCommand> = Vec::new();
    while received.len() < commands.len() + 1 {
        received.extend(server.drain());
        if Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    let mut expected: Vec<&str> = vec!["show_and_focus"];
    expected.extend_from_slice(&commands);
    let received_names: Vec<&str> = received
        .iter()
        .map(|command| command_name(*command))
        .collect();
    report.record(
        "parent",
        "dispatched",
        &format!("expected={expected:?} received={received_names:?}"),
    )?;
    if received_names != expected {
        return Err(format!("Dispatch order mismatch: {received_names:?}").into());
    }

    // Protocol rejection: a child sends malformed frames; each must be rejected.
    for kind in [
        "unknown-command",
        "wrong-version",
        "bad-json",
        "unknown-field",
    ] {
        let raw_report = path.with_extension(format!("raw-{kind}.tsv"));
        let child_id = spawn_exe(&[
            "--ipc-raw".to_string(),
            raw_report.to_string_lossy().into_owned(),
            kind.to_string(),
        ])?;
        wait_for_line(&raw_report, "verdict", CHILD_TIMEOUT)?;
        report.record(
            "parent",
            "rejected_frame",
            &format!("{kind} pid={child_id}"),
        )?;
    }

    drop(server);
    drop(_mutex_guard);
    report.record("parent", "completed", "all IPC phases passed")?;
    Ok(())
}
