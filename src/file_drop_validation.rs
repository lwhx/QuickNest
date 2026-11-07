use std::cell::{Cell, RefCell};
use std::error::Error;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use quicknest::infrastructure::windows::drop_target::DropRegistration;
use quicknest::infrastructure::windows::file_drop::{
    FileDropEvent, FileDropStats, receiving_target,
};
use quicknest::infrastructure::windows::window_handle::{WindowHandleError, inspect_window};
use slint::{ComponentHandle, Timer, TimerMode};

pub fn run(path: &Path) -> Result<(), Box<dyn Error>> {
    run_mode(path, false)
}

pub fn run_dpi(path: &Path) -> Result<(), Box<dyn Error>> {
    if std::env::var_os("SLINT_SCALE_FACTOR").is_some() {
        return Err("Unset SLINT_SCALE_FACTOR for actual Windows DPI validation".into());
    }
    run_mode(path, true)
}

fn run_mode(path: &Path, dpi: bool) -> Result<(), Box<dyn Error>> {
    let mut report = OpenOptions::new().write(true).create_new(true).open(path)?;
    writeln!(report, "elapsed_ms\tphase\tdata")?;
    report.flush()?;
    let report = Rc::new(RefCell::new(report));
    let started = Instant::now();
    let ui = crate::create_window()?;
    ui.set_validation_status("P0-05：从 Explorer 拖入文件；只接收路径，不复制或启动".into());
    if dpi {
        crate::dpi_validation::attach(&ui, report.clone(), started);
    }
    ui.on_exit_requested(|| {
        if let Err(error) = slint::quit_event_loop() {
            eprintln!("Failed to quit file-drop test: {error}");
        }
    });
    ui.window().on_close_requested(|| {
        if let Err(error) = slint::quit_event_loop() {
            eprintln!("Failed to close file-drop test: {error}");
        }
        slint::CloseRequestResponse::HideWindow
    });
    ui.show()?;
    let (sender, receiver) = mpsc::sync_channel(256);
    let stats = Rc::new(FileDropStats::default());
    let guard = Rc::new(RefCell::new(None::<DropRegistration>));
    let failure = Rc::new(RefCell::new(None::<String>));
    let received = Rc::new(Cell::new(0_usize));
    let heartbeat = Rc::new(Cell::new(0_u32));
    let weak = ui.as_weak();
    let timer = Timer::default();
    let registration = guard.clone();
    let stats_ref = stats.clone();
    let report_ref = report.clone();
    let failure_ref = failure.clone();
    let count = received.clone();
    let pulse = heartbeat.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(25), move || {
        let result = (|| -> Result<(), Box<dyn Error>> {
            let ui = weak
                .upgrade()
                .ok_or("File-drop UI was destroyed unexpectedly")?;
            if registration.borrow().is_none() {
                match inspect_window(ui.window()) {
                    Ok(observed) => {
                        let target = receiving_target(sender.clone(), stats_ref.clone());
                        *registration.borrow_mut() =
                            Some(DropRegistration::register(ui.window(), target)?);
                        writeln!(
                            report_ref.borrow_mut(),
                            "{}\tregistered\tpid={} tid={} hwnd={}",
                            started.elapsed().as_millis(),
                            observed.pid,
                            observed.tid,
                            observed.hwnd
                        )?;
                    }
                    Err(WindowHandleError::Unavailable(_))
                        if started.elapsed() < Duration::from_secs(5) =>
                    {
                        return Ok(());
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            pulse.set(pulse.get() + 1);
            if pulse.get().is_multiple_of(40) {
                writeln!(
                    report_ref.borrow_mut(),
                    "{}\theartbeat\t{}",
                    started.elapsed().as_millis(),
                    pulse.get()
                )?;
            }
            for event in receiver.try_iter().take(128) {
                if dpi {
                    let sample = match &event {
                        FileDropEvent::Enter { x, y, .. } => Some(("enter", (*x, *y))),
                        FileDropEvent::Over { x, y } => Some(("over", (*x, *y))),
                        FileDropEvent::Dropped { x, y, .. } => Some(("drop", (*x, *y))),
                        _ => None,
                    };
                    if let Some((phase, screen)) = sample {
                        crate::dpi_validation::record(&ui, &report_ref, started, phase, screen)?;
                    }
                }
                let elapsed = started.elapsed().as_millis();
                match event {
                    FileDropEvent::Enter { supported, x, y } => {
                        writeln!(
                            report_ref.borrow_mut(),
                            "{elapsed}\tenter\tsupported={supported} x={x} y={y}"
                        )?;
                        ui.set_validation_status(
                            if supported {
                                "可接收文件引用：松开鼠标添加验证结果"
                            } else {
                                "不支持的拖放格式"
                            }
                            .into(),
                        );
                    }
                    FileDropEvent::Over { x, y } => {
                        writeln!(report_ref.borrow_mut(), "{elapsed}\tover\tx={x} y={y}")?;
                    }
                    FileDropEvent::Leave => {
                        writeln!(
                            report_ref.borrow_mut(),
                            "{elapsed}\tleave\tcount={}",
                            count.get()
                        )?;
                        ui.set_validation_status("拖放已离开/取消；接收结果未增加".into());
                    }
                    FileDropEvent::Dropped { paths, x, y } => {
                        count.set(count.get() + paths.len());
                        writeln!(
                            report_ref.borrow_mut(),
                            "{elapsed}\tdrop\tcount={} total={} x={x} y={y}",
                            paths.len(),
                            count.get()
                        )?;
                        for (index, path) in paths.iter().enumerate() {
                            writeln!(
                                report_ref.borrow_mut(),
                                "{elapsed}\tpath\t{index}\t{}",
                                path.display()
                            )?;
                        }
                        let names = paths
                            .iter()
                            .filter_map(|path| path.file_name())
                            .map(|name| name.to_string_lossy())
                            .collect::<Vec<_>>()
                            .join("、");
                        ui.set_validation_status(
                            format!(
                                "已接收 {} 项（累计 {}）：{}",
                                paths.len(),
                                count.get(),
                                names
                            )
                            .into(),
                        );
                    }
                    FileDropEvent::Failed { hresult } => {
                        writeln!(
                            report_ref.borrow_mut(),
                            "{elapsed}\terror\t0x{:08X}",
                            hresult as u32
                        )?;
                        ui.set_validation_status(
                            format!("拖放失败：0x{:08X}", hresult as u32).into(),
                        );
                    }
                }
            }
            report_ref.borrow_mut().flush()?;
            Ok(())
        })();
        if let Err(error) = result {
            *failure_ref.borrow_mut() = Some(error.to_string());
            if let Err(error) = slint::quit_event_loop() {
                eprintln!("Failed to stop file-drop validation: {error}");
            }
        }
    });
    let loop_result = slint::run_event_loop_until_quit();
    drop(timer);
    if let Some(mut registration) = guard.borrow_mut().take() {
        registration.revoke()?;
        drop(registration);
    }
    writeln!(
        report.borrow_mut(),
        "{}\tcleanup\tacquired={} released={} destroyed={} enters={} overs={} leaves={} drops={} errors={} received={} heartbeat={}",
        started.elapsed().as_millis(),
        stats.acquired.get(),
        stats.released.get(),
        stats.destroyed.get(),
        stats.enters.get(),
        stats.overs.get(),
        stats.leaves.get(),
        stats.drops.get(),
        stats.errors.get(),
        received.get(),
        heartbeat.get()
    )?;
    report.borrow_mut().flush()?;
    ui.hide()?;
    loop_result?;
    if let Some(error) = failure.borrow().as_ref() {
        return Err(error.clone().into());
    }
    if stats.acquired.get() != stats.released.get() || stats.destroyed.get() != 1 {
        return Err("File-drop medium/COM cleanup mismatch".into());
    }
    Ok(())
}
