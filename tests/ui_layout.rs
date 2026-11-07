use std::cell::Cell;
use std::rc::Rc;

use i_slint_backend_testing::ElementHandle;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, ModelRc, VecModel};

slint::include_modules!();

fn window() -> MainWindow {
    i_slint_backend_testing::init_no_event_loop();
    let ui = MainWindow::new().unwrap();
    ui.window().set_size(slint::PhysicalSize::new(760, 520));
    ui.show().unwrap();
    ui
}

fn element(ui: &MainWindow, id: &str) -> ElementHandle {
    ElementHandle::find_by_element_id(ui, &format!("MainWindow::{id}"))
        .next()
        .unwrap_or_else(|| panic!("missing element {id}; build with SLINT_EMIT_DEBUG_INFO=1"))
}

fn press(ui: &MainWindow, text: &str) {
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.into() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text: text.into() });
}

fn click(ui: &MainWindow, id: &str) {
    element(ui, id).mock_single_click(PointerEventButton::Left);
}

#[test]
fn modal_escape_bubbles_from_controls_and_title_buttons_keep_receiving_clicks() {
    let ui = window();
    let weak = ui.as_weak();
    ui.on_settings_open_requested(move || weak.unwrap().set_settings_panel_visible(true));
    let weak = ui.as_weak();
    ui.on_settings_panel_closed(move || weak.unwrap().set_settings_panel_visible(false));

    let escapes = Rc::new(Cell::new(0));
    let observed = escapes.clone();
    let weak = ui.as_weak();
    ui.on_escape_pressed(move || {
        observed.set(observed.get() + 1);
        let ui = weak.unwrap();
        ui.set_settings_panel_visible(false);
        ui.set_category_dialog_visible(false);
        ui.set_update_confirm_visible(false);
    });
    let toggles = Rc::new(Cell::new(0));
    let observed = toggles.clone();
    ui.on_everything_toggle_requested(move || observed.set(observed.get() + 1));

    // Exercise pointer dispatch, rather than invoking the TouchArea callback.
    click(&ui, "settings-hit");
    assert!(ui.get_settings_panel_visible());
    click(&ui, "panel-close-hit");
    assert!(!ui.get_settings_panel_visible());
    click(&ui, "settings-hit");
    press(&ui, "\u{1b}");
    assert!(!ui.get_settings_panel_visible());
    assert_eq!(escapes.get(), 1);

    click(&ui, "settings-hit");
    // Tab must still enter standard controls, and Space must still toggle
    // the checkbox. Escape must then bubble from that focused control.
    for _ in 0..8 {
        press(&ui, "\t");
        press(&ui, " ");
        if toggles.get() > 0 {
            break;
        }
    }
    assert_eq!(
        toggles.get(),
        1,
        "Tab/Space must reach the settings checkbox"
    );
    press(&ui, "\u{1b}");
    assert!(!ui.get_settings_panel_visible());
    assert_eq!(escapes.get(), 2);

    // Opening a text dialog must preserve the input's initial focus.
    ui.set_category_dialog_visible(true);
    let input = element(&ui, "category-dialog-input");
    press(&ui, "x");
    assert_eq!(input.accessible_value().as_deref(), Some("x"));
    press(&ui, "\u{1b}");
    assert!(!ui.get_category_dialog_visible());
    assert_eq!(escapes.get(), 3);

    // The update confirmation also uses the ancestor after Tab moves focus.
    ui.set_update_confirm_visible(true);
    press(&ui, "\t");
    press(&ui, "\u{1b}");
    assert!(!ui.get_update_confirm_visible());
    assert_eq!(escapes.get(), 4);
    ui.hide().unwrap();
}

#[test]
fn switching_modals_keeps_escape_when_the_old_focused_control_is_removed() {
    let ui = window();
    let escapes = Rc::new(Cell::new(0));
    let observed = escapes.clone();
    let weak = ui.as_weak();
    ui.on_escape_pressed(move || {
        observed.set(observed.get() + 1);
        let ui = weak.unwrap();
        ui.set_settings_panel_visible(false);
        ui.set_backup_dialog_visible(false);
        ui.set_transfer_dialog_visible(false);
        ui.set_update_confirm_visible(false);
    });
    let toggles = Rc::new(Cell::new(0));
    let observed = toggles.clone();
    ui.on_everything_toggle_requested(move || observed.set(observed.get() + 1));
    for (index, target) in ["backup", "transfer", "update"].into_iter().enumerate() {
        let before = toggles.get();
        ui.set_settings_panel_visible(true);
        for _ in 0..8 {
            press(&ui, "\t");
            press(&ui, " ");
            if toggles.get() > before {
                break;
            }
        }
        assert_eq!(toggles.get(), before + 1);
        ui.set_settings_panel_visible(false);
        match target {
            "backup" => ui.set_backup_dialog_visible(true),
            "transfer" => ui.set_transfer_dialog_visible(true),
            "update" => ui.set_update_confirm_visible(true),
            _ => unreachable!(),
        }
        assert!(ui.get_any_modal_open());
        press(&ui, "\u{1b}");
        assert!(!ui.get_backup_dialog_visible(), "{target}");
        assert!(!ui.get_transfer_dialog_visible(), "{target}");
        assert!(!ui.get_update_confirm_visible(), "{target}");
        assert_eq!(escapes.get(), index + 1, "{target}");
    }
    ui.hide().unwrap();
}

#[test]
fn group_scroll_viewport_fits_beside_the_fixed_overflow_button() {
    let ui = window();
    for count in [0usize, 2, 20] {
        let groups: Vec<_> = (0..count)
            .map(|index| GroupView {
                id: format!("g{index}").into(),
                name: format!("Group {index}").into(),
                accent: "".into(),
                active: index == 0,
                item_count: 0,
                tab_width: 96.0,
            })
            .collect();
        ui.set_groups(ModelRc::new(VecModel::from(groups)));
        ui.set_group_tabs_content_width(
            count as f32 * 96.0 + count.saturating_sub(1) as f32 * 24.0,
        );
        for width in [680, 760, 1200] {
            ui.window().set_size(slint::PhysicalSize::new(width, 520));
            let viewport = element(&ui, "group-tabs-scroll");
            let overflow = element(&ui, "overflow-column");
            let button = element(&ui, "group-overflow");
            let bar = element(&ui, "group-bar");
            let near = |actual: f32, expected: f32| {
                assert!(
                    (actual - expected).abs() < 0.01,
                    "count={count}, window={width}: {actual} != {expected}"
                );
            };
            near(bar.absolute_position().y, 44.0);
            near(bar.size().height, 40.0);
            near(viewport.absolute_position().x, 14.0);
            near(viewport.absolute_position().y, 52.0);
            near(viewport.size().width, width as f32 - 58.0);
            near(viewport.size().height, 24.0);
            near(overflow.absolute_position().x, width as f32 - 36.0);
            near(overflow.absolute_position().y, 52.0);
            near(overflow.size().width, 28.0);
            near(overflow.size().height, 24.0);
            near(button.absolute_position().y, 52.0);
            near(button.size().height, 24.0);
            near(
                viewport.absolute_position().x + viewport.size().width + 8.0,
                overflow.absolute_position().x,
            );
            near(
                overflow.absolute_position().x + overflow.size().width + 8.0,
                width as f32,
            );
        }
    }
    ui.hide().unwrap();
}

#[test]
fn search_escape_keeps_clear_then_dismiss_behavior_without_hiding_window() {
    let ui = window();
    let escapes = Rc::new(Cell::new(0));
    let observed = escapes.clone();
    ui.on_escape_pressed(move || observed.set(observed.get() + 1));
    let dismissed = Rc::new(Cell::new(0));
    let observed = dismissed.clone();
    let weak = ui.as_weak();
    ui.on_search_dismissed(move || {
        observed.set(observed.get() + 1);
        let ui = weak.unwrap();
        if ui.get_search_query().is_empty() {
            ui.set_search_visible(false);
        } else {
            ui.set_search_query("".into());
        }
    });
    ui.set_search_query("query".into());
    ui.set_search_visible(true);
    // No timer or explicit input focus: the ancestor must route this safely.
    press(&ui, "\u{1b}");
    assert!(ui.get_search_visible());
    assert!(ui.get_search_query().is_empty());
    click(&ui, "search-input");
    press(&ui, "\u{1b}");
    assert!(!ui.get_search_visible());
    assert_eq!(dismissed.get(), 2);
    assert_eq!(
        escapes.get(),
        0,
        "search Escape must not fall through to hide"
    );
    ui.hide().unwrap();
}
