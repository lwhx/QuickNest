//! P1-17 external-drop hit testing (baseline 5.6).
//!
//! The OLE drop target only knows physical screen coordinates. After the UI
//! thread converts them to Slint logical coordinates, this module maps the
//! point onto the static regions of `ui/main_window.slint`: title bar, group
//! bar, category sidebar and item grid. Layout constants mirror the `.slint`
//! declarations; P1-01～P1-03 verification scripts pin the same geometry.

use crate::library_controller::{CategoryRow, GroupRow};

pub const TITLE_BAR_HEIGHT: f32 = 44.0;
pub const GROUP_BAR_HEIGHT: f32 = 40.0;
pub const CATEGORY_BAR_WIDTH: f32 = 160.0;
pub const STATUS_BAR_HEIGHT: f32 = 32.0;

const GROUP_PADDING_LEFT: f32 = 14.0;
const GROUP_PADDING_TOP: f32 = 8.0;
const GROUP_TAB_HEIGHT: f32 = 24.0;
const GROUP_TAB_SPACING: f32 = 24.0;

const CATEGORY_PADDING_TOP: f32 = 10.0;
const CATEGORY_ROW_HEIGHT: f32 = 36.0;
/// UI 优化（2026-11-06 侧边栏样式）：36px 行高 + 2px 垂直间距。
const CATEGORY_ROW_PITCH: f32 = 38.0;
const NEW_CATEGORY_ROW_HEIGHT: f32 = 36.0;
const NEW_CATEGORY_PADDING: f32 = 18.0;
const CATEGORY_PILL_INSET: f32 = 6.0;
const CATEGORY_SCROLLBAR_WIDTH: f32 = 14.0;
const CATEGORY_PILL_RADIUS: f32 = 6.0;

/// Where a dragged file would land (baseline 5.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropZone {
    /// Top group tab (spring-load target; a fast drop also targets it).
    Group(String),
    /// Left category row.
    Category(String),
    /// Right item grid (appends to the active category).
    Grid,
    /// Title bar, status bar, padding or empty sidebar space.
    None,
}

/// Maps a logical client point to a drop zone. `groups`/`categories` must be
/// the rows currently rendered (active group's categories for the sidebar).
/// Non-finite or out-of-window points never map to a zone.
pub fn hit_test(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    groups: &[GroupRow],
    categories: &[CategoryRow],
) -> DropZone {
    hit_test_scrolled(x, y, width, height, groups, categories, 0.0)
}

/// As `hit_test`, with the sidebar's scroll offset in logical pixels.
/// Positive means scrolling down: content moves up, so the offset is added to
/// the pointer's viewport y. A negative value represents overscroll above the
/// first row; it is never interpreted as scrolling down. Slint exposes the
/// opposite sign as `ScrollView.content-y`; pass `-content_y / 1px` here.
pub fn hit_test_scrolled(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    groups: &[GroupRow],
    categories: &[CategoryRow],
    category_scroll_y: f32,
) -> DropZone {
    if !x.is_finite()
        || !y.is_finite()
        || !width.is_finite()
        || !height.is_finite()
        || x < 0.0
        || y < 0.0
        || x >= width
        || y >= height
    {
        return DropZone::None;
    }
    let content_top = TITLE_BAR_HEIGHT;
    let group_bar_bottom = content_top + GROUP_BAR_HEIGHT;
    let status_top = height - STATUS_BAR_HEIGHT;

    if y < content_top || y >= status_top {
        return DropZone::None;
    }
    if y < group_bar_bottom {
        let tab_top = content_top + GROUP_PADDING_TOP;
        let tab_bottom = tab_top + GROUP_TAB_HEIGHT;
        if y < tab_top || y > tab_bottom {
            return DropZone::None;
        }
        let mut left = GROUP_PADDING_LEFT;
        for group in groups {
            let tab_width = group.tab_width.max(0.0);
            if x >= left && x < left + tab_width {
                return DropZone::Group(group.id.clone());
            }
            left += tab_width + GROUP_TAB_SPACING;
        }
        return DropZone::None;
    }
    if x < CATEGORY_BAR_WIDTH {
        let rows_top = group_bar_bottom + CATEGORY_PADDING_TOP;
        // ScrollView begins 10px into the sidebar; its height must stop at
        // the footer gap, rather than extending 10px below that boundary.
        let rows_bottom =
            height - STATUS_BAR_HEIGHT - NEW_CATEGORY_ROW_HEIGHT - NEW_CATEGORY_PADDING;
        let pill_left = CATEGORY_PILL_INSET;
        // Reserve the same fixed 14px scrollbar lane in the UI, independent
        // of the platform widget style or whether a scrollbar is visible.
        let pill_right = CATEGORY_BAR_WIDTH - CATEGORY_SCROLLBAR_WIDTH - CATEGORY_PILL_INSET;
        if y < rows_top
            || y >= rows_bottom
            || x < pill_left
            || x >= pill_right
            || !category_scroll_y.is_finite()
        {
            return DropZone::None;
        }
        let offset = y - rows_top + category_scroll_y;
        if offset < 0.0 || !offset.is_finite() {
            return DropZone::None;
        }
        let index = (offset / CATEGORY_ROW_PITCH).floor() as usize;
        let row_y = offset % CATEGORY_ROW_PITCH;
        if row_y >= CATEGORY_ROW_HEIGHT {
            return DropZone::None;
        }
        // Exclude the rounded pill's corners as well as its outside margins.
        let corner_dx = (pill_left + CATEGORY_PILL_RADIUS - x)
            .max(x - (pill_right - CATEGORY_PILL_RADIUS))
            .max(0.0);
        let corner_dy = (CATEGORY_PILL_RADIUS - row_y)
            .max(row_y - (CATEGORY_ROW_HEIGHT - CATEGORY_PILL_RADIUS))
            .max(0.0);
        if corner_dx * corner_dx + corner_dy * corner_dy
            > CATEGORY_PILL_RADIUS * CATEGORY_PILL_RADIUS
        {
            return DropZone::None;
        }
        return match categories.get(index) {
            Some(category) => DropZone::Category(category.id.clone()),
            None => DropZone::None,
        };
    }
    DropZone::Grid
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(id: &str, width: f32) -> GroupRow {
        GroupRow {
            id: id.to_string(),
            name: id.to_string(),
            color: String::new(),
            active: false,
            item_count: 0,
            tab_width: width,
        }
    }

    fn category(id: &str) -> CategoryRow {
        CategoryRow {
            id: id.to_string(),
            name: id.to_string(),
            item_count: 0,
        }
    }

    #[test]
    fn title_status_and_outside_are_not_drop_targets() {
        let groups = [group("g1", 60.0)];
        let categories = [category("c1")];
        assert_eq!(
            hit_test(100.0, 10.0, 800.0, 600.0, &groups, &categories),
            DropZone::None
        );
        assert_eq!(
            hit_test(100.0, 590.0, 800.0, 600.0, &groups, &categories),
            DropZone::None
        );
        assert_eq!(
            hit_test(-1.0, 200.0, 800.0, 600.0, &groups, &categories),
            DropZone::None
        );
        assert_eq!(
            hit_test(f32::NAN, 200.0, 800.0, 600.0, &groups, &categories),
            DropZone::None
        );
    }

    #[test]
    fn group_tabs_use_padding_and_spacing() {
        let groups = [group("g1", 60.0), group("g2", 40.0)];
        let categories = [category("c1")];
        assert_eq!(
            hit_test(14.0, 60.0, 800.0, 600.0, &groups, &categories),
            DropZone::Group("g1".into())
        );
        assert_eq!(
            hit_test(80.0, 60.0, 800.0, 600.0, &groups, &categories),
            DropZone::None,
            "spacing is not a tab"
        );
        assert_eq!(
            hit_test(98.0, 60.0, 800.0, 600.0, &groups, &categories),
            DropZone::Group("g2".into())
        );
        assert_eq!(
            hit_test(100.0, 46.0, 800.0, 600.0, &groups, &categories),
            DropZone::None,
            "group bar padding"
        );
    }

    #[test]
    fn category_rows_map_to_their_ids_and_clip() {
        let groups = [group("g1", 60.0)];
        let categories = [category("c1"), category("c2")];
        assert_eq!(
            hit_test(20.0, 94.0, 800.0, 600.0, &groups, &categories),
            DropZone::Category("c1".into())
        );
        assert_eq!(
            hit_test(130.0, 152.0, 800.0, 600.0, &groups, &categories),
            DropZone::Category("c2".into()),
            "row pitch is now 38px (36px pill + 2px gap)"
        );
        assert_eq!(
            hit_test(20.0, 131.0, 800.0, 600.0, &groups, &categories),
            DropZone::None,
            "the 2px gap between pills is not a row"
        );
        assert_eq!(
            hit_test(20.0, 190.0, 800.0, 600.0, &groups, &categories),
            DropZone::None,
            "third row does not exist"
        );
        assert_eq!(
            hit_test(20.0, 558.0, 800.0, 600.0, &groups, &categories),
            DropZone::None,
            "below clipped rows / new-category footer"
        );
        assert_eq!(
            hit_test(160.0, 200.0, 800.0, 600.0, &groups, &categories),
            DropZone::Grid,
            "sidebar border belongs to the grid side"
        );
        assert_eq!(
            hit_test(159.0, 152.0, 800.0, 600.0, &groups, &categories),
            DropZone::None,
            "scrollbar lane is never a category target"
        );
    }

    #[test]
    fn scrolled_rows_use_positive_downward_offset_and_preserve_gaps() {
        let categories = [category("c1"), category("c2"), category("c3")];
        let hit = |y, scroll| hit_test_scrolled(20.0, y, 800.0, 600.0, &[], &categories, scroll);
        assert_eq!(hit(94.0, 38.0), DropZone::Category("c2".into()), "one row");
        assert_eq!(hit(94.0, 19.0), DropZone::Category("c1".into()), "half row");
        assert_eq!(hit(111.0, 19.0), DropZone::None, "scrolled 2px gap");
        assert_eq!(hit(113.0, 19.0), DropZone::Category("c2".into()));
        assert_eq!(
            hit(94.0, -19.0),
            DropZone::None,
            "negative overscroll exposes blank space"
        );
        assert_eq!(hit(114.0, -19.0), DropZone::Category("c1".into()));
        assert_eq!(hit(114.0, f32::NAN), DropZone::None);
        assert_eq!(hit(114.0, f32::INFINITY), DropZone::None);
    }

    #[test]
    fn pill_margins_corners_scrollbar_and_bottom_gap_are_excluded() {
        let categories: Vec<_> = (0..30).map(|i| category(&format!("c{i}"))).collect();
        let hit = |x, y, scroll| hit_test_scrolled(x, y, 800.0, 600.0, &[], &categories, scroll);
        for x in [0.0, 5.9, 140.0, 145.0, 146.0, 159.0] {
            assert_eq!(hit(x, 110.0, 38.0), DropZone::None, "x={x}");
        }
        assert_eq!(hit(6.0, 94.0, 38.0), DropZone::None, "rounded corner");
        assert_eq!(hit(6.0, 112.0, 38.0), DropZone::Category("c1".into()));
        assert_eq!(hit(139.0, 112.0, 38.0), DropZone::Category("c1".into()));
        assert_eq!(hit(20.0, 513.0, 19.0), DropZone::Category("c11".into()));
        for y in [514.0, 515.0, 519.0, 520.0, 558.0] {
            assert_eq!(
                hit(20.0, y, 19.0),
                DropZone::None,
                "bottom gap/footer y={y}"
            );
        }
        assert_eq!(hit(20.0, 93.0, 38.0), DropZone::None, "clip above viewport");
    }

    #[test]
    fn grid_covers_the_right_content_area() {
        let groups = [group("g1", 60.0)];
        let categories = [category("c1")];
        assert_eq!(
            hit_test(180.0, 96.0, 800.0, 600.0, &groups, &categories),
            DropZone::Grid,
            "the grid starts right of the 164px sidebar"
        );
        assert_eq!(
            hit_test(799.0, 560.0, 800.0, 600.0, &groups, &categories),
            DropZone::Grid
        );
        assert_eq!(
            hit_test(799.0, 570.0, 800.0, 600.0, &groups, &categories),
            DropZone::None,
            "status bar"
        );
    }
}
