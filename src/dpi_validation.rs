use std::cell::RefCell;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::rc::Rc;
use std::time::Instant;

use quicknest::infrastructure::windows::coordinates::{
    CoordinateSnapshot, cursor_screen, from_screen,
};
use slint::ComponentHandle;

use crate::MainWindow;

pub fn record(
    ui: &MainWindow,
    report: &Rc<RefCell<File>>,
    started: Instant,
    phase: &str,
    screen: (i32, i32),
) -> Result<CoordinateSnapshot, Box<dyn Error>> {
    let point = from_screen(ui.window(), screen)?;
    let width = ui.get_reference_width();
    let height = ui.get_reference_height();
    let region = quadrant(point.logical, (width, height), point.inside);
    writeln!(
        report.borrow_mut(),
        "{}\tcoordinate\tphase={phase} sx={} sy={} cx={} cy={} lx={:.4} ly={:.4} dpi={} scale={:.4} pmv2={} inside={} sizes_match={} scale_match={} cw={} ch={} sw={} sh={} ox={} oy={} lw={:.4} lh={:.4} region={region}",
        started.elapsed().as_millis(),
        point.screen.0,
        point.screen.1,
        point.client.0,
        point.client.1,
        point.logical.0,
        point.logical.1,
        point.dpi,
        point.scale,
        point.per_monitor_v2,
        point.inside,
        point.sizes_match,
        point.scale_matches_dpi,
        point.client_size.0,
        point.client_size.1,
        point.slint_size.0,
        point.slint_size.1,
        point.client_origin.0,
        point.client_origin.1,
        width,
        height
    )?;
    if point.inside {
        ui.set_marker_x(point.logical.0);
        ui.set_marker_y(point.logical.1);
    }
    Ok(point)
}

fn quadrant(point: (f32, f32), size: (f32, f32), inside: bool) -> &'static str {
    if !inside {
        return "outside";
    }
    match (point.0 < size.0 / 2.0, point.1 < size.1 / 2.0) {
        (true, true) => "top_left",
        (false, true) => "top_right",
        (true, false) => "bottom_left",
        (false, false) => "bottom_right",
    }
}

pub fn attach(ui: &MainWindow, report: Rc<RefCell<File>>, started: Instant) {
    ui.set_dpi_validation(true);
    ui.set_validation_status(
        "移动指针并从 Explorer 拖入文件；红点为转换结果，分界由 Slint 布局定义".into(),
    );
    let weak = ui.as_weak();
    ui.on_pointer_reference(move |x, y, width, height| {
        let result = (|| -> Result<(), Box<dyn Error>> {
            let ui = weak.upgrade().ok_or("DPI reference window unavailable")?;
            let point = record(&ui, &report, started, "pointer", cursor_screen()?)?;
            let dx = point.logical.0 - x;
            let dy = point.logical.1 - y;
            // Independent reference comes from Slint's TouchArea event, not inverse math.
            writeln!(report.borrow_mut(), "{}\treference\tx={x:.4} y={y:.4} width={width:.4} height={height:.4} dx={dx:.4} dy={dy:.4} expected_region={}",
                started.elapsed().as_millis(), quadrant((x, y), (width, height), true))?;
            report.borrow_mut().flush()?;
            ui.set_validation_status(format!("DPI={} scale={:.2} | client=({}, {}) | logical=({:.2}, {:.2}) | Slint=({x:.2}, {y:.2}) | delta=({dx:.3}, {dy:.3})",
                point.dpi, point.scale, point.client.0, point.client.1, point.logical.0, point.logical.1).into());
            Ok(())
        })();
        if let Err(error) = result { eprintln!("DPI reference observation failed: {error}"); }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjacent_regions_use_half_open_boundaries() {
        assert_eq!(quadrant((49.9, 49.9), (100.0, 100.0), true), "top_left");
        assert_eq!(quadrant((50.0, 49.9), (100.0, 100.0), true), "top_right");
        assert_eq!(quadrant((49.9, 50.0), (100.0, 100.0), true), "bottom_left");
        assert_eq!(quadrant((50.0, 50.0), (100.0, 100.0), true), "bottom_right");
        assert_eq!(quadrant((-1.0, 0.0), (100.0, 100.0), false), "outside");
    }
}
