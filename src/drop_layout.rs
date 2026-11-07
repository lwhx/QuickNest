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
const GROUP_PADDING_RIGHT: f32 = 8.0;
const GROUP_OVERFLOW_WIDTH: f32 = 28.0;
const GROUP_OVERFLOW_SPACING: f32 = 8.0;
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
    hit_test_scrolled(x, y, width, height, groups, categories, ScrollOffsets::ZERO)
}

/// Scroll offsets for [`hit_test_scrolled`], in logical pixels.
///
/// Positive `category_y` means scrolling down: content moves up, so the
/// offset is added to the pointer's viewport y. A negative value represents
/// overscroll above the first row; it is never interpreted as scrolling down.
/// Slint exposes the opposite sign as `ScrollView.content-y`; pass
/// `-content_y / 1px` here. `group_x` follows the same convention for the
/// top group tabs (positive = scrolled right, content moved left). UI 体验
/// 修复（2026-11-07 第三批）：分组栏支持横向滚动后，拖放命中必须补偿滚动
/// 偏移。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollOffsets {
    pub group_x: f32,
    pub category_y: f32,
}

impl ScrollOffsets {
    pub const ZERO: Self = Self {
        group_x: 0.0,
        category_y: 0.0,
    };
}

/// As `hit_test`, with both scroll offsets in logical pixels.
pub fn hit_test_scrolled(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    groups: &[GroupRow],
    categories: &[CategoryRow],
    offsets: ScrollOffsets,
) -> DropZone {
    let group_scroll_x = offsets.group_x;
    let category_scroll_y = offsets.category_y;
    if !x.is_finite()
        || !y.is_finite()
        || !width.is_finite()
        || !height.is_finite()
        || !group_scroll_x.is_finite()
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
        let viewport_right =
            width - GROUP_PADDING_RIGHT - GROUP_OVERFLOW_WIDTH - GROUP_OVERFLOW_SPACING;
        // Clip in viewport coordinates before applying scroll. Neither the
        // left padding nor the fixed overflow button/gap is part of a tab.
        if y < tab_top || y >= tab_bottom || x < GROUP_PADDING_LEFT || x >= viewport_right {
            return DropZone::None;
        }
        // The pointer is in viewport coordinates: add the scroll offset to
        // reach content coordinates. A negative offset (overscroll) shifts
        // the content right, so the gap before the first tab grows and the
        // point naturally falls into the padding whitespace.
        let effective_x = x + group_scroll_x;
        let mut left = GROUP_PADDING_LEFT;
        for group in groups {
            let tab_width = group.tab_width.max(0.0);
            if effective_x >= left && effective_x < left + tab_width {
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

    // UI 体验修复（2026-11-07 第三批）：分组栏横向滚动后，拖放命中按
    // ScrollOffsets.group_x 补偿；负向超滚（内容右移）不把空白当 tab。
    #[test]
    fn group_tab_hits_compensate_horizontal_scroll() {
        let groups = [group("g1", 60.0), group("g2", 40.0)];
        let hit_at = |x, group_x| {
            hit_test_scrolled(
                x,
                60.0,
                800.0,
                600.0,
                &groups,
                &[],
                ScrollOffsets {
                    group_x,
                    category_y: 0.0,
                },
            )
        };
        // 未滚动时 x=30 命中 g1；滚动 90px（内容左移）后同一屏幕位置
        // 命中 g2（内容坐标 120 落在 g2 的 98..138）。
        assert_eq!(hit_at(30.0, 0.0), DropZone::Group("g1".into()));
        assert_eq!(hit_at(30.0, 90.0), DropZone::Group("g2".into()));
        // 滚动后视口左侧 0..14 仍是留白，不命中任何 tab。
        assert_eq!(hit_at(5.0, 90.0), DropZone::None);
        // 负向超滚只扩大首 tab 前的空白，不产生假命中。
        assert_eq!(hit_at(30.0, -100.0), DropZone::None);
        // 非有限偏移整体拒绝。
        assert_eq!(hit_at(30.0, f32::NAN), DropZone::None);
    }

    #[test]
    fn group_viewport_excludes_fixed_padding_gap_and_overflow_before_scrolling() {
        let groups = [group("wide", 4096.0)];
        for width in [680.0, 760.0, 1200.0] {
            let right = width - 8.0 - 28.0 - 8.0;
            for scroll in [0.0, 20.0, 450.0] {
                let hit = |x| {
                    hit_test_scrolled(
                        x,
                        60.0,
                        width,
                        600.0,
                        &groups,
                        &[],
                        ScrollOffsets {
                            group_x: scroll,
                            category_y: 0.0,
                        },
                    )
                };
                for x in [
                    0.0,
                    5.0,
                    13.999,
                    right,
                    right + 7.0,
                    width - 36.0,
                    width - 8.0,
                    width - 1.0,
                ] {
                    assert_eq!(
                        hit(x),
                        DropZone::None,
                        "width={width}, x={x}, scroll={scroll}"
                    );
                }
                assert_eq!(hit(14.0), DropZone::Group("wide".into()));
                assert_eq!(hit(right - 0.001), DropZone::Group("wide".into()));
            }
        }
    }

    #[test]
    fn group_tabs_have_half_open_edges_and_preserve_overscroll_whitespace() {
        let groups = [group("g1", 60.0), group("g2", 40.0)];
        let hit = |x, y, group_x| {
            hit_test_scrolled(
                x,
                y,
                760.0,
                520.0,
                &groups,
                &[],
                ScrollOffsets {
                    group_x,
                    category_y: 0.0,
                },
            )
        };
        assert_eq!(hit(14.0, 52.0, 0.0), DropZone::Group("g1".into()));
        assert_eq!(hit(14.0, 51.999, 0.0), DropZone::None);
        assert_eq!(hit(14.0, 75.999, 0.0), DropZone::Group("g1".into()));
        assert_eq!(hit(14.0, 76.0, 0.0), DropZone::None);
        assert_eq!(hit(73.999, 60.0, 0.0), DropZone::Group("g1".into()));
        assert_eq!(hit(74.0, 60.0, 0.0), DropZone::None);
        assert_eq!(hit(98.0, 60.0, 0.0), DropZone::Group("g2".into()));
        assert_eq!(hit(138.0, 60.0, 0.0), DropZone::None);
        assert_eq!(hit(53.999, 60.0, 20.0), DropZone::Group("g1".into()));
        assert_eq!(hit(54.0, 60.0, 20.0), DropZone::None);
        assert_eq!(hit(78.0, 60.0, 20.0), DropZone::Group("g2".into()));
        assert_eq!(hit(14.0, 60.0, -20.0), DropZone::None);
        assert_eq!(hit(34.0, 60.0, -20.0), DropZone::Group("g1".into()));
        assert_eq!(hit(94.0, 60.0, -20.0), DropZone::None);
        assert_eq!(hit(14.0, 60.0, 1000.0), DropZone::None);
        for scroll in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(hit(30.0, 60.0, scroll), DropZone::None);
        }
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
        let hit = |y, scroll| {
            hit_test_scrolled(
                20.0,
                y,
                800.0,
                600.0,
                &[],
                &categories,
                ScrollOffsets {
                    group_x: 0.0,
                    category_y: scroll,
                },
            )
        };
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
        let hit = |x, y, scroll| {
            hit_test_scrolled(
                x,
                y,
                800.0,
                600.0,
                &[],
                &categories,
                ScrollOffsets {
                    group_x: 0.0,
                    category_y: scroll,
                },
            )
        };
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
