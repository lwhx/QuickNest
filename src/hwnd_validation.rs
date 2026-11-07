use std::cell::{Cell, RefCell};
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use quicknest::infrastructure::windows::window_handle::{
    WindowHandleError, WindowObservation, inspect_window, observe_value,
};
use slint::{ComponentHandle, Timer, TimerMode};

use crate::{MainWindow, create_window};

const HOLD: Duration = Duration::from_millis(500);
const TIMEOUT: Duration = Duration::from_secs(5);
const GENERATIONS: u8 = 3;

pub(crate) trait LifecycleHooks {
    fn phase(
        &mut self,
        _phase: &str,
        _generation: u8,
        _window: Option<&slint::Window>,
        _observed: WindowObservation,
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

struct NoHooks;
impl LifecycleHooks for NoHooks {}

#[derive(Clone, Copy)]
enum Phase {
    WaitingForCreation,
    Visible,
    Hidden,
    Restored,
    WaitingForDestruction,
    ConfirmingDestruction,
    Finished,
}

struct Validation {
    owner: Option<MainWindow>,
    weak: slint::Weak<MainWindow>,
    phase: Phase,
    generation: u8,
    observation: Option<WindowObservation>,
    phase_started: Instant,
    started: Instant,
    report: File,
    hooks: Box<dyn LifecycleHooks>,
    completed: bool,
    failure: Option<String>,
}

impl Validation {
    fn record(&mut self, phase: &str) -> Result<(), Box<dyn Error>> {
        let last = self.observation.ok_or("Missing window observation")?;
        let observed = observe_value(last.hwnd);
        writeln!(
            self.report,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            self.started.elapsed().as_millis(),
            self.generation,
            phase,
            last.hwnd,
            observed.is_window,
            observed.visible,
            self.weak.upgrade().is_some(),
            last.pid,
            last.tid
        )?;
        self.report.flush()?;
        Ok(())
    }

    fn advance(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_started = Instant::now();
    }

    fn inspect(&self, visible: bool) -> Result<WindowObservation, Box<dyn Error>> {
        let ui = self.owner.as_ref().ok_or("Missing component owner")?;
        let observed = inspect_window(ui.window())?;
        let previous = self.observation.ok_or("Missing original HWND")?;
        if observed.hwnd != previous.hwnd
            || observed.pid != previous.pid
            || observed.tid != previous.tid
            || observed.visible != visible
        {
            return Err("Window identity or visibility changed unexpectedly".into());
        }
        Ok(observed)
    }

    fn tick(&mut self) -> Result<bool, Box<dyn Error>> {
        let elapsed = self.phase_started.elapsed();
        if elapsed > TIMEOUT {
            return Err("HWND lifecycle phase exceeded five-second deadline".into());
        }
        match self.phase {
            Phase::WaitingForCreation => {
                let ui = self
                    .owner
                    .as_ref()
                    .ok_or("Missing component during creation")?;
                match inspect_window(ui.window()) {
                    Ok(observed) => {
                        if !observed.visible {
                            return Ok(false);
                        }
                        self.observation = Some(observed);
                        self.hooks.phase(
                            "created",
                            self.generation,
                            Some(ui.window()),
                            observed,
                        )?;
                        self.record("created")?;
                        self.advance(Phase::Visible);
                    }
                    Err(WindowHandleError::Unavailable(_)) => return Ok(false),
                    Err(error) => return Err(error.into()),
                }
            }
            Phase::Visible => {
                self.inspect(true)?;
                if elapsed >= HOLD {
                    self.owner.as_ref().ok_or("Missing component")?.hide()?;
                    let observed = self.inspect(false)?;
                    self.hooks.phase(
                        "hidden",
                        self.generation,
                        self.owner.as_ref().map(|ui| ui.window()),
                        observed,
                    )?;
                    self.record("hidden")?;
                    self.advance(Phase::Hidden);
                }
            }
            Phase::Hidden => {
                self.inspect(false)?;
                if elapsed >= HOLD {
                    self.owner.as_ref().ok_or("Missing component")?.show()?;
                    let observed = self.inspect(true)?;
                    self.hooks.phase(
                        "restored",
                        self.generation,
                        self.owner.as_ref().map(|ui| ui.window()),
                        observed,
                    )?;
                    self.record("restored")?;
                    self.advance(Phase::Restored);
                }
            }
            Phase::Restored => {
                self.inspect(true)?;
                if elapsed >= HOLD {
                    let observed = self.inspect(true)?;
                    self.hooks.phase(
                        "before_release",
                        self.generation,
                        self.owner.as_ref().map(|ui| ui.window()),
                        observed,
                    )?;
                    // No Slint WindowHandle provider or strong callback captures survive here.
                    // Hide releases Slint's visible-component reference; drop releases our owner.
                    let owner = self.owner.take().ok_or("Missing component at release")?;
                    owner.hide()?;
                    drop(owner);
                    self.record("component_released")?;
                    if self.weak.upgrade().is_some() {
                        return Err("Component remained alive after releasing its owner".into());
                    }
                    self.advance(Phase::WaitingForDestruction);
                }
            }
            Phase::WaitingForDestruction => {
                let last = self.observation.ok_or("Missing released HWND")?;
                if self.weak.upgrade().is_some() {
                    return Err("Released component unexpectedly became available".into());
                }
                if !observe_value(last.hwnd).is_window {
                    self.hooks.phase("destroyed", self.generation, None, last)?;
                    self.record("destroyed")?;
                    self.advance(Phase::ConfirmingDestruction);
                }
            }
            Phase::ConfirmingDestruction => {
                let last = self.observation.ok_or("Missing destroyed HWND")?;
                if observe_value(last.hwnd).is_window {
                    return Err("Destroyed HWND was reused during the observation interval".into());
                }
                if elapsed >= HOLD {
                    self.record("destroyed_confirmed")?;
                    if self.generation == GENERATIONS {
                        self.record("completed")?;
                        self.completed = true;
                        self.advance(Phase::Finished);
                    } else {
                        self.generation += 1;
                        let ui = validation_window(self.generation)?;
                        self.weak = ui.as_weak();
                        self.owner = Some(ui);
                        self.observation = None;
                        self.advance(Phase::WaitingForCreation);
                    }
                }
            }
            Phase::Finished => return Ok(elapsed >= HOLD),
        }
        Ok(false)
    }
}

fn validation_window(generation: u8) -> Result<MainWindow, Box<dyn Error>> {
    let ui = create_window()?;
    ui.set_validation_status(format!("P0-02 HWND 生命周期验证 · 第 {generation}/3 代").into());
    ui.window()
        .on_close_requested(|| slint::CloseRequestResponse::HideWindow);
    ui.show()?;
    Ok(ui)
}

pub fn run(path: &Path) -> Result<(), Box<dyn Error>> {
    run_with_hooks(path, Box::new(NoHooks))
}

pub(crate) fn run_with_hooks(
    path: &Path,
    hooks: Box<dyn LifecycleHooks>,
) -> Result<(), Box<dyn Error>> {
    if std::env::var_os("SLINT_DESTROY_WINDOW_ON_HIDE").is_some() {
        return Err(
            "Unset SLINT_DESTROY_WINDOW_ON_HIDE for the default Windows lifecycle test".into(),
        );
    }
    let mut report = OpenOptions::new().write(true).create_new(true).open(path)?;
    writeln!(
        report,
        "elapsed_ms\tgeneration\tphase\thwnd\tis_window\tvisible\tcomponent_alive\tpid\ttid"
    )?;
    report.flush()?;
    let ui = validation_window(1)?;
    let weak = ui.as_weak();
    let state = Rc::new(RefCell::new(Validation {
        owner: Some(ui),
        weak,
        phase: Phase::WaitingForCreation,
        generation: 1,
        observation: None,
        phase_started: Instant::now(),
        started: Instant::now(),
        report,
        hooks,
        completed: false,
        failure: None,
    }));
    let quit_requested = Rc::new(Cell::new(false));
    let quit = quit_requested.clone();
    let state_weak = Rc::downgrade(&state);
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(25), move || {
        if quit.get() {
            return;
        }
        let Some(state) = state_weak.upgrade() else {
            return;
        };
        let mut state = state.borrow_mut();
        let finished = match state.tick() {
            Ok(finished) => finished,
            Err(error) => {
                state.failure = Some(error.to_string());
                if state.observation.is_some()
                    && let Err(record_error) = state.record("failed")
                {
                    eprintln!("Failed to record lifecycle failure: {record_error}");
                }
                true
            }
        };
        if finished {
            quit.set(true);
            if let Err(error) = slint::quit_event_loop() {
                eprintln!("Failed to quit HWND validation: {error}");
                std::process::exit(1);
            }
        }
    });
    let loop_result = slint::run_event_loop_until_quit();
    drop(timer);
    loop_result?;
    let state = state.borrow();
    if let Some(error) = &state.failure {
        return Err(error.clone().into());
    }
    if !state.completed {
        return Err("HWND lifecycle validation did not complete".into());
    }
    Ok(())
}
