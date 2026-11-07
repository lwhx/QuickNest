//! P2-15 (baseline 16.4): DPI and multi-monitor placement.
//!
//! Pure geometry without any Win32 dependency, so every rule is unit
//! testable without a display. All coordinates are physical pixels of the
//! monitor that owns them; the logical size of the window never changes when
//! the scale factor changes, only its physical size does.
//!
//! Two rules drive the module:
//!
//! * a window that is restored from the configuration must end up inside the
//!   work area of a monitor that is still connected, and
//! * a window that would be restored completely off-screen (its monitor was
//!   unplugged, the resolution changed) is moved back into the nearest
//!   monitor instead of staying invisible.

/// The scale factors the baseline requires (100%, 125%, 150%, 175%). 200% is
/// explicitly out of the V1.2 scope.
pub const REQUIRED_DPIS: [u32; 4] = [96, 120, 144, 168];

/// A rectangle in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn from_size_at(x: i32, y: i32, size: (i32, i32)) -> Self {
        Self::new(x, y, x + size.0, y + size.1)
    }

    pub fn width(&self) -> i32 {
        (self.right - self.left).max(0)
    }

    pub fn height(&self) -> i32 {
        (self.bottom - self.top).max(0)
    }

    pub fn size(&self) -> (i32, i32) {
        (self.width(), self.height())
    }

    pub fn is_empty(&self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }

    /// True when the two rectangles share at least one pixel.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.left < other.right
            && other.left < self.right
            && self.top < other.bottom
            && other.top < self.bottom
    }

    /// True when `other` lies completely inside this rectangle.
    pub fn contains(&self, other: &Rect) -> bool {
        other.left >= self.left
            && other.top >= self.top
            && other.right <= self.right
            && other.bottom <= self.bottom
    }
}

/// One connected display: its full bounds, its work area (bounds minus the
/// taskbar and other app bars), its effective DPI, whether it is the primary
/// display and the device name Windows reports for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorArea {
    pub bounds: Rect,
    pub work: Rect,
    pub dpi: u32,
    pub primary: bool,
    pub device: String,
}

/// The scale factor that belongs to a DPI value (96 dpi = 100%).
pub fn scale_for_dpi(dpi: u32) -> f32 {
    dpi as f32 / 96.0
}

/// The position that keeps a window of `size` fully inside `work`.
///
/// A window larger than the work area cannot fit; it is aligned to the top
/// left corner of the work area so its title bar stays reachable.
pub fn position_inside(size: (i32, i32), preferred: (i32, i32), work: Rect) -> (i32, i32) {
    let (width, height) = size;
    // 横纵独立钳制：一个方向刚好填满工作区时，另一方向仍保留期望位置
    // （旧实现会在任一方向满宽时把两轴同时归零）。
    let x = if width >= work.width() {
        work.left
    } else {
        preferred.0.clamp(work.left, work.right - width)
    };
    let y = if height >= work.height() {
        work.top
    } else {
        preferred.1.clamp(work.top, work.bottom - height)
    };
    (x, y)
}

/// Clamps the window rectangle fully into the work area.
pub fn clamp_into(window: Rect, work: Rect) -> Rect {
    let (x, y) = position_inside(window.size(), (window.left, window.top), work);
    Rect::from_size_at(x, y, window.size())
}

/// The monitor whose work area is closest to the window. Distance zero means
/// the window already overlaps that work area; the primary monitor wins ties.
pub fn nearest_monitor(window: Rect, monitors: &[MonitorArea]) -> Option<usize> {
    if monitors.is_empty() {
        return None;
    }
    let mut best: Option<(usize, i64, bool)> = None;
    for (index, monitor) in monitors.iter().enumerate() {
        let distance = if window.intersects(&monitor.work) {
            0
        } else {
            let dx = (monitor.work.left - window.right).max(window.left - monitor.work.right);
            let dy = (monitor.work.top - window.bottom).max(window.top - monitor.work.bottom);
            let (dx, dy) = (dx.max(0) as i64, dy.max(0) as i64);
            dx * dx + dy * dy
        };
        let better = match best {
            None => true,
            Some((_, best_distance, best_primary)) => {
                distance < best_distance
                    || (distance == best_distance && monitor.primary && !best_primary)
            }
        };
        if better {
            best = Some((index, distance, monitor.primary));
        }
    }
    best.map(|(index, _, _)| index)
}

/// The corrected window rectangle when the window is not visible in any work
/// area, or `None` when the window is already (partly) on screen.
///
/// The window keeps its size; only its position changes, so the logical
/// layout and the scale factor of the target monitor stay untouched.
pub fn ensure_visible(window: Rect, monitors: &[MonitorArea]) -> Option<Rect> {
    if monitors.is_empty() {
        return None;
    }
    if monitors
        .iter()
        .any(|monitor| window.intersects(&monitor.work))
    {
        return None;
    }
    let index = nearest_monitor(window, monitors)?;
    let work = monitors[index].work;
    let (x, y) = position_inside(window.size(), (work.left, work.top), work);
    Some(Rect::from_size_at(x, y, window.size()))
}

/// The rectangle the window should occupy: fully inside the work area of the
/// display that owns it, and on a connected display at all.
///
/// A window that is completely outside every work area (its display was
/// unplugged) is moved to the nearest connected display; a window that is
/// only partly outside is pulled back into the work area of the display that
/// owns it.
pub fn resolve_placement(window: Rect, monitors: &[MonitorArea]) -> Option<Rect> {
    if monitors.is_empty() {
        return None;
    }
    if let Some(corrected) = ensure_visible(window, monitors) {
        return Some(corrected);
    }
    let index = nearest_monitor(window, monitors)?;
    Some(clamp_into(window, monitors[index].work))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(bounds: Rect, work: Rect, dpi: u32, primary: bool) -> MonitorArea {
        MonitorArea {
            bounds,
            work,
            dpi,
            primary,
            device: String::new(),
        }
    }

    #[test]
    fn required_scale_factors_match_the_baseline() {
        assert_eq!(REQUIRED_DPIS, [96, 120, 144, 168]);
        let scales: Vec<f32> = REQUIRED_DPIS
            .iter()
            .map(|dpi| scale_for_dpi(*dpi))
            .collect();
        assert_eq!(scales, vec![1.0, 1.25, 1.5, 1.75]);
    }

    #[test]
    fn window_inside_work_area_keeps_its_position() {
        let work = Rect::new(0, 0, 1920, 1032);
        let window = Rect::from_size_at(100, 80, (760, 520));
        assert_eq!(clamp_into(window, work), window);
        assert_eq!(position_inside((760, 520), (100, 80), work), (100, 80));
    }

    #[test]
    fn window_overflowing_the_work_area_is_pulled_back() {
        let work = Rect::new(0, 0, 1920, 1032);
        let right = clamp_into(Rect::from_size_at(1800, 100, (760, 520)), work);
        assert_eq!(right.left, 1920 - 760);
        let bottom = clamp_into(Rect::from_size_at(100, 900, (760, 520)), work);
        assert_eq!(bottom.top, 1032 - 520);
        let left = clamp_into(Rect::from_size_at(-40, 100, (760, 520)), work);
        assert_eq!(left.left, 0);
        let top = clamp_into(Rect::from_size_at(100, -30, (760, 520)), work);
        assert_eq!(top.top, 0);
    }

    // 修复：宽度刚好填满工作区时，垂直位置应被钳制而非归零。
    #[test]
    fn exactly_fitting_width_keeps_the_vertical_position() {
        let work = Rect::new(0, 0, 800, 600);
        assert_eq!(
            position_inside((800, 200), (0, 300), work),
            (0, 300),
            "one full axis must not reset the other"
        );
        assert_eq!(
            position_inside((800, 200), (0, 900), work),
            (0, 400),
            "the other axis is still clamped into the work area"
        );
    }

    #[test]
    fn window_larger_than_the_work_area_is_aligned_to_its_origin() {
        let work = Rect::new(0, 0, 800, 600);
        let window = Rect::from_size_at(120, 90, (1000, 700));
        assert_eq!(
            clamp_into(window, work),
            Rect::from_size_at(0, 0, (1000, 700))
        );
    }

    #[test]
    fn visible_window_is_not_corrected() {
        let monitors = vec![
            monitor(
                Rect::new(0, 0, 1920, 1080),
                Rect::new(0, 0, 1920, 1032),
                96,
                true,
            ),
            monitor(
                Rect::new(1920, 0, 3840, 1080),
                Rect::new(1920, 0, 3840, 1032),
                144,
                false,
            ),
        ];
        let on_primary = Rect::from_size_at(300, 200, (760, 520));
        let on_second = Rect::from_size_at(2000, 100, (760, 520));
        assert_eq!(ensure_visible(on_primary, &monitors), None);
        assert_eq!(ensure_visible(on_second, &monitors), None);
    }

    #[test]
    fn window_on_a_disconnected_monitor_is_moved_back() {
        let monitors = vec![monitor(
            Rect::new(0, 0, 1920, 1080),
            Rect::new(0, 0, 1920, 1032),
            96,
            true,
        )];
        // The second display was unplugged while the window sat on it.
        let orphan = Rect::from_size_at(2400, 300, (760, 520));
        let corrected = ensure_visible(orphan, &monitors).expect("off-screen window is corrected");
        assert_eq!(corrected, Rect::from_size_at(0, 0, (760, 520)));
        assert!(monitors[0].work.contains(&corrected));
    }

    #[test]
    fn window_on_a_secondary_display_keeps_its_relative_offset() {
        let monitors = vec![
            monitor(
                Rect::new(0, 0, 1920, 1080),
                Rect::new(0, 0, 1920, 1032),
                96,
                true,
            ),
            monitor(
                Rect::new(1920, 0, 3840, 1080),
                Rect::new(1920, 0, 3840, 1032),
                144,
                false,
            ),
        ];
        // Straddling the boundary still touches the second work area.
        let straddling = Rect::from_size_at(1700, 100, (760, 520));
        assert_eq!(ensure_visible(straddling, &monitors), None);
    }

    #[test]
    fn monitor_to_the_left_of_the_origin_is_supported() {
        let monitors = vec![
            monitor(
                Rect::new(-1920, 0, 0, 1080),
                Rect::new(-1920, 0, 0, 1032),
                144,
                true,
            ),
            monitor(
                Rect::new(0, 0, 1920, 1080),
                Rect::new(0, 0, 1920, 1032),
                96,
                false,
            ),
        ];
        let orphan = Rect::from_size_at(-3000, 200, (760, 520));
        let corrected = ensure_visible(orphan, &monitors).expect("corrected");
        assert_eq!(corrected, Rect::from_size_at(-1920, 0, (760, 520)));
        assert!(monitors[0].work.contains(&corrected));
    }

    #[test]
    fn nearest_monitor_prefers_the_closest_and_the_primary_on_ties() {
        let monitors = vec![
            monitor(
                Rect::new(0, 0, 1920, 1080),
                Rect::new(0, 0, 1920, 1032),
                96,
                true,
            ),
            monitor(
                Rect::new(1920, 0, 3840, 1080),
                Rect::new(1920, 0, 3840, 1032),
                144,
                false,
            ),
        ];
        let far_right = Rect::from_size_at(3700, 900, (760, 520));
        assert_eq!(nearest_monitor(far_right, &monitors), Some(1));
        let far_left = Rect::from_size_at(-2000, 900, (760, 520));
        assert_eq!(nearest_monitor(far_left, &monitors), Some(0));
        assert_eq!(nearest_monitor(far_right, &[]), None);
    }

    #[test]
    fn resolved_placement_keeps_a_visible_window_inside_the_work_area() {
        let monitors = vec![
            monitor(
                Rect::new(0, 0, 1920, 1080),
                Rect::new(0, 0, 1920, 1032),
                96,
                true,
            ),
            monitor(
                Rect::new(1920, 0, 3840, 1080),
                Rect::new(1920, 0, 3840, 1032),
                144,
                false,
            ),
        ];
        // Fully on the second display: the position is kept.
        let on_second = Rect::from_size_at(2000, 100, (760, 520));
        assert_eq!(resolve_placement(on_second, &monitors), Some(on_second));
        // Straddling the right edge of the first display: pulled back.
        let straddling = Rect::from_size_at(1800, 100, (760, 520));
        let resolved = resolve_placement(straddling, &monitors).expect("resolved");
        assert_eq!(resolved, Rect::from_size_at(1160, 100, (760, 520)));
        // The display it belonged to is gone: moved to the nearest one.
        let orphan = Rect::from_size_at(4000, 200, (760, 520));
        let resolved = resolve_placement(orphan, &monitors).expect("resolved");
        assert_eq!(resolved, Rect::from_size_at(1920, 0, (760, 520)));
        assert_eq!(resolve_placement(orphan, &[]), None);
    }

    #[test]
    fn rectangle_predicates_cover_touching_edges() {
        let left = Rect::new(0, 0, 10, 10);
        let right = Rect::new(10, 0, 20, 10);
        assert!(!left.intersects(&right));
        let overlapping = Rect::new(9, 0, 20, 10);
        assert!(left.intersects(&overlapping));
        assert!(left.contains(&Rect::new(1, 1, 9, 9)));
        assert!(!left.contains(&overlapping));
        assert!(Rect::new(5, 5, 5, 5).is_empty());
        assert_eq!(Rect::new(0, 0, 10, 10).size(), (10, 10));
    }
}
