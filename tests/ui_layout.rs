use std::cell::{Cell, RefCell};
use std::rc::Rc;

use i_slint_backend_testing::{AccessibleRole, ElementHandle, ElementQuery};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, ModelRc, VecModel};

slint::include_modules!();

// Exercise the actual reusable component with dynamic enabled/selected state.
mod pill_fixture {
    slint::slint! {
        import { PillButton } from "../ui/main_window.slint";
        export component PillFixture inherits Window {
            width: 300px;
            height: 80px;
            in-out property <bool> option-enabled: true;
            in-out property <bool> option-selected: true;
            out property <bool> option-has-focus: option.has-focus;
            out property <bool> previous-has-focus: previous.has-focus;
            out property <bool> next-has-focus: next.has-focus;
            out property <int> focus-target-count: 3;
            out property <float> option-center-x: (option.absolute-position.x + option.width / 2) / 1px;
            out property <float> option-center-y: (option.absolute-position.y + option.height / 2) / 1px;
            out property <string> option-accessible-label: option.accessible-label;
            out property <bool> option-accessible-enabled: option.accessible-enabled;
            out property <bool> option-accessible-checkable: option.accessible-checkable;
            out property <bool> option-accessible-checked: option.accessible-checked;
            callback activated();
            callback escaped();
            callback accessible-activation();
            accessible-activation => { option.accessible-action-default(); }
            FocusScope {
                focus-on-click: false;
                focus-on-tab-navigation: false;
                key-pressed(event) => {
                    if (event.text == Key.Escape) { root.escaped(); accept }
                    else { reject }
                }
                HorizontalLayout {
                    padding: 10px;
                    spacing: 8px;
                    previous := PillButton { label: "上一项"; }
                    option := PillButton {
                        label: "测试选项";
                        enabled: root.option-enabled;
                        selected: root.option-selected;
                        clicked => { root.activated(); }
                    }
                    next := PillButton { label: "下一项"; }
                }
            }
        }
    }
}

fn window() -> MainWindow {
    i_slint_backend_testing::init_no_event_loop();
    let ui = MainWindow::new().unwrap();
    ui.window().set_size(slint::PhysicalSize::new(760, 520));
    ui.global::<UiContext>().set_theme(ui.get_theme());
    ui.show().unwrap();
    ui
}

fn element(ui: &MainWindow, id: &str) -> ElementHandle {
    ElementHandle::find_by_element_id(ui, &format!("MainWindow::{id}"))
        .next()
        .unwrap_or_else(|| panic!("missing element {id}; build with SLINT_EMIT_DEBUG_INFO=1"))
}

fn accessible_id(ui: &impl ComponentHandle, id: &str) -> ElementHandle {
    let id = id.to_string();
    ElementQuery::from_root(ui)
        .match_predicate(move |candidate| candidate.accessible_id().as_deref() == Some(id.as_str()))
        .find_first()
        .unwrap_or_else(|| panic!("missing accessible instance"))
}

fn labelled(ui: &impl ComponentHandle, label: &str, role: AccessibleRole) -> ElementHandle {
    ElementHandle::find_by_accessible_label(ui, label)
        .find(|candidate| candidate.accessible_role() == Some(role))
        .unwrap_or_else(|| panic!("missing {role:?} named {label}"))
}

fn press(ui: &impl ComponentHandle, text: &str) {
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.into() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text: text.into() });
}

fn key(ui: &impl ComponentHandle, key: Key) {
    let text: slint::SharedString = key.into();
    press(ui, text.as_str());
}

fn shift_tab(ui: &impl ComponentHandle) {
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Shift.into(),
    });
    press(ui, "\t");
    ui.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Shift.into(),
    });
}

fn tab_with_bound(ui: &impl ComponentHandle, bound: usize, mut reached: impl FnMut() -> bool) {
    for _ in 0..bound {
        press(ui, "\t");
        if reached() {
            return;
        }
    }
    panic!("Tab did not reach its target within {bound} elements");
}

fn tab_until(ui: &impl ComponentHandle, reached: impl FnMut() -> bool) {
    // Visible elements bound traversal even as settings gain focus targets.
    tab_with_bound(
        ui,
        ElementQuery::from_root(ui).find_all().len() + 1,
        reached,
    );
}

fn click(ui: &MainWindow, id: &str) {
    element(ui, id).mock_single_click(PointerEventButton::Left);
}

#[test]
fn pill_buttons_expose_state_and_reject_disabled_pointer_keyboard_and_accessible_actions() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = pill_fixture::PillFixture::new().unwrap();
    ui.show().unwrap();
    slint::platform::update_timers_and_animations();
    let activations = Rc::new(Cell::new(0));
    let observed = activations.clone();
    ui.on_activated(move || observed.set(observed.get() + 1));
    let escapes = Rc::new(Cell::new(0));
    let observed = escapes.clone();
    ui.on_escaped(move || observed.set(observed.get() + 1));
    // Inline slint! fixtures do not inherit build.rs debug-info settings.
    // Observe the real imported component through test-only property bindings;
    // MainWindow below also verifies actual ElementHandle accessibility data.
    assert_eq!(ui.get_option_accessible_label().as_str(), "测试选项");
    assert!(ui.get_option_accessible_enabled());
    assert!(ui.get_option_accessible_checkable());
    assert!(ui.get_option_accessible_checked());
    ui.set_option_selected(false);
    assert!(!ui.get_option_accessible_checked());
    let focus_bound = ui.get_focus_target_count() as usize + 1;
    let click_option = || {
        let position =
            slint::LogicalPosition::new(ui.get_option_center_x(), ui.get_option_center_y());
        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position });
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Left,
        });
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        });
    };

    tab_with_bound(&ui, focus_bound, || ui.get_option_has_focus());
    shift_tab(&ui);
    assert!(
        ui.get_previous_has_focus(),
        "Shift+Tab must leave the option"
    );
    press(&ui, "\t");
    assert!(ui.get_option_has_focus());
    for text in ["\n", " "] {
        let before = activations.get();
        // Repeated key-down and stray key-up must never duplicate activation.
        for _ in 0..3 {
            ui.window()
                .dispatch_event(WindowEvent::KeyPressed { text: text.into() });
        }
        assert_eq!(activations.get(), before);
        ui.window()
            .dispatch_event(WindowEvent::KeyReleased { text: text.into() });
        assert_eq!(activations.get(), before + 1);
        ui.window()
            .dispatch_event(WindowEvent::KeyReleased { text: text.into() });
        assert_eq!(activations.get(), before + 1);
    }
    key(&ui, Key::Escape);
    assert_eq!(escapes.get(), 1, "Escape must reach the ancestor");
    click_option();
    assert_eq!(activations.get(), 3);
    ui.invoke_accessible_activation();
    assert_eq!(activations.get(), 4);

    // Disabling a focused option clears focus and cancels a held activation.
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: " ".into() });
    ui.set_option_enabled(false);
    // No-event-loop fixtures must run Slint's deferred changed handlers before
    // observing the next input iteration, just as the production backend does.
    slint::platform::update_timers_and_animations();
    assert!(!ui.get_option_accessible_enabled());
    assert!(!ui.get_option_has_focus());
    click_option();
    ui.invoke_accessible_activation();
    press(&ui, " ");
    key(&ui, Key::Return);
    assert_eq!(activations.get(), 4);
    for _ in 0..focus_bound {
        press(&ui, "\t");
        assert!(
            !ui.get_option_has_focus(),
            "disabled option accepted Tab focus"
        );
    }
    assert!(ui.get_previous_has_focus() || ui.get_next_has_focus());
    ui.set_option_enabled(true);
    tab_with_bound(&ui, focus_bound, || ui.get_option_has_focus());
    // Releasing the key held before disabling must not activate the option.
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text: " ".into() });
    assert_eq!(activations.get(), 4);
    key(&ui, Key::Return);
    assert_eq!(activations.get(), 5);
    ui.hide().unwrap();
}

#[test]
fn settings_title_and_options_are_keyboard_reachable_without_moving_pointer_hits() {
    let ui = window();
    let settings = labelled(&ui, "设置", AccessibleRole::Button);
    let hit = element(&ui, "settings-hit");
    assert_eq!(settings.absolute_position(), hit.absolute_position());
    assert_eq!(settings.size(), hit.size());
    assert_eq!(hit.size(), slint::LogicalSize::new(36.0, 32.0));
    let opened = Rc::new(Cell::new(0));
    let observed = opened.clone();
    let weak = ui.as_weak();
    ui.on_settings_open_requested(move || {
        observed.set(observed.get() + 1);
        weak.unwrap().set_settings_panel_visible(true);
    });
    let selected = Rc::new(Cell::new(0));
    let observed = selected.clone();
    let weak = ui.as_weak();
    ui.on_theme_menu_selected(move |mode| {
        observed.set(observed.get() + 1);
        weak.unwrap().set_theme_mode(mode);
    });
    let has_title_focus = || {
        ElementHandle::find_by_element_id(&ui, "MainWindow::settings-focus-ring")
            .next()
            .is_some()
    };
    tab_until(&ui, has_title_focus);
    key(&ui, Key::Return);
    assert_eq!(opened.get(), 1);
    assert!(ui.get_settings_panel_visible());
    assert_eq!(settings.accessible_enabled(), Some(false));
    let light = labelled(&ui, "浅色", AccessibleRole::Button);
    let dark = labelled(&ui, "深色", AccessibleRole::Button);
    let has_focus = |button: &ElementHandle| {
        button
            .query_descendants()
            .match_id("PillButton::focus-ring")
            .find_first()
            .is_some()
    };
    tab_until(&ui, || has_focus(&light));
    press(&ui, "\t");
    assert!(has_focus(&dark));
    shift_tab(&ui);
    assert!(has_focus(&light));
    press(&ui, " ");
    assert_eq!(selected.get(), 1);
    assert_eq!(light.accessible_checked(), Some(true));
    assert_eq!(dark.accessible_checked(), Some(false));
    assert_eq!(
        element(&ui, "settings-hit").absolute_position(),
        hit.absolute_position()
    );
    click(&ui, "settings-hit");
    assert_eq!(
        opened.get(),
        1,
        "modal barrier must keep background settings disabled"
    );
    ui.hide().unwrap();
}

#[test]
fn shared_name_dialogs_preserve_edits_on_error_and_reinitialize_after_cancel() {
    let ui = window();
    let confirmed = Rc::new(RefCell::new(String::new()));
    for group in [false, true] {
        let observed = confirmed.clone();
        let weak = ui.as_weak();
        if group {
            ui.on_group_dialog_confirmed(move |name| {
                *observed.borrow_mut() = name.to_string();
                weak.unwrap()
                    .set_group_dialog_error("分组名称已存在".into());
            });
            let weak = ui.as_weak();
            ui.on_group_dialog_cancelled(move || weak.unwrap().set_group_dialog_visible(false));
            ui.set_group_dialog_title("重命名分组".into());
            ui.set_group_dialog_initial("Group".into());
            ui.set_group_dialog_error("".into());
            ui.set_group_dialog_visible(true);
        } else {
            ui.on_category_dialog_confirmed(move |name| {
                *observed.borrow_mut() = name.to_string();
                weak.unwrap()
                    .set_category_dialog_error("分类名称已存在".into());
            });
            let weak = ui.as_weak();
            ui.on_category_dialog_cancelled(move || {
                weak.unwrap().set_category_dialog_visible(false)
            });
            ui.set_category_dialog_title("重命名分类".into());
            ui.set_category_dialog_initial("Category".into());
            ui.set_category_dialog_error("".into());
            ui.set_category_dialog_visible(true);
        }
        let prefix = if group {
            "group-dialog"
        } else {
            "category-dialog"
        };
        let initial = if group { "Group" } else { "Category" };
        let input = accessible_id(&ui, &format!("{prefix}-input"));
        assert_eq!(input.accessible_value().as_deref(), Some(initial));
        assert_eq!(input.accessible_role(), Some(AccessibleRole::TextInput));
        assert_eq!(
            input.accessible_label().as_deref(),
            Some(if group {
                "分组名称"
            } else {
                "分类名称"
            })
        );
        key(&ui, Key::End);
        press(&ui, "-edited");
        let edited = format!("{initial}-edited");
        assert_eq!(input.accessible_value().as_deref(), Some(edited.as_str()));
        accessible_id(&ui, &format!("{prefix}-confirm"))
            .mock_single_click(PointerEventButton::Left);
        assert_eq!(*confirmed.borrow(), edited);
        assert_eq!(input.accessible_value().as_deref(), Some(edited.as_str()));
        if group {
            ui.set_group_dialog_initial("must not replace edits".into());
        } else {
            ui.set_category_dialog_initial("must not replace edits".into());
        }
        assert_eq!(input.accessible_value().as_deref(), Some(edited.as_str()));
        let panel = accessible_id(&ui, prefix);
        let error = accessible_id(&ui, &format!("{prefix}-error"));
        let confirm = accessible_id(&ui, &format!("{prefix}-confirm"));
        assert!(error.absolute_position().y + error.size().height <= confirm.absolute_position().y);
        assert!(
            confirm.absolute_position().y + confirm.size().height
                <= panel.absolute_position().y + panel.size().height
        );
        accessible_id(&ui, &format!("{prefix}-cancel")).mock_single_click(PointerEventButton::Left);
        assert_eq!(
            ElementHandle::find_by_accessible_label(
                &ui,
                if group {
                    "分组名称"
                } else {
                    "分类名称"
                }
            )
            .count(),
            0
        );
        if group {
            assert!(!ui.get_group_dialog_visible());
            ui.set_group_dialog_title("新建分组".into());
            ui.set_group_dialog_initial("Fresh".into());
            ui.set_group_dialog_error("".into());
            ui.set_group_dialog_visible(true);
        } else {
            assert!(!ui.get_category_dialog_visible());
            ui.set_category_dialog_title("新建分类".into());
            ui.set_category_dialog_initial("Fresh".into());
            ui.set_category_dialog_error("".into());
            ui.set_category_dialog_visible(true);
        }
        let fresh = accessible_id(&ui, &format!("{prefix}-input"));
        assert_eq!(fresh.accessible_value().as_deref(), Some("Fresh"));
        // The shield and canvas must not take the new input's focus.
        click(&ui, "settings-hit");
        key(&ui, Key::End);
        press(&ui, "-again");
        assert_eq!(fresh.accessible_value().as_deref(), Some("Fresh-again"));
        accessible_id(&ui, &format!("{prefix}-cancel")).mock_single_click(PointerEventButton::Left);
    }
    ui.hide().unwrap();
}

#[test]
fn item_editor_keeps_actions_in_window_and_scrolls_wrapped_errors_at_both_supported_sizes() {
    let ui = window();
    let saves = Rc::new(Cell::new(0));
    let observed = saves.clone();
    ui.on_item_dialog_confirmed(move || observed.set(observed.get() + 1));
    let cancels = Rc::new(Cell::new(0));
    let observed = cancels.clone();
    let weak = ui.as_weak();
    ui.on_item_dialog_cancelled(move || {
        observed.set(observed.get() + 1);
        weak.unwrap().set_item_dialog_visible(false);
    });
    let long_error = "目标路径不存在，请检查程序位置、执行参数和工作目录后再次保存。".repeat(6);
    for (width, height) in [(680, 480), (760, 520)] {
        ui.window()
            .set_size(slint::PhysicalSize::new(width, height));
        for item_type in [0, 5] {
            ui.set_item_dialog_type_index(item_type);
            ui.set_item_dialog_error("".into());
            ui.set_item_dialog_visible(true);
            let editor = element(&ui, "item-editor");
            let viewport = element(&ui, "item-editor-scroll");
            let save = element(&ui, "item-editor-save");
            let cancel = element(&ui, "item-editor-cancel");
            assert_eq!(ui.window().size(), slint::PhysicalSize::new(width, height));
            assert_eq!(editor.size().width, 470.0);
            assert_eq!(
                editor.size().height,
                if item_type == 5 { 436.0 } else { 396.0 }
            );
            assert!(editor.absolute_position().y >= 16.0);
            assert!(editor.absolute_position().y + editor.size().height <= height as f32 - 16.0);
            let footer_y = save.absolute_position().y;
            for button in [&save, &cancel] {
                assert!(button.size().height >= 24.0);
                assert!(
                    button.absolute_position().y
                        >= viewport.absolute_position().y + viewport.size().height
                );
                assert!(
                    button.absolute_position().y + button.size().height
                        <= editor.absolute_position().y + editor.size().height - 10.0
                );
                assert!(button.absolute_position().x >= editor.absolute_position().x);
                assert!(
                    button.absolute_position().x + button.size().width
                        <= editor.absolute_position().x + editor.size().width
                );
            }
            ui.set_item_dialog_error(long_error.clone().into());
            let error = element(&ui, "item-editor-error");
            assert!(
                error.size().height > 32.0,
                "long validation error must wrap: window={width}x{height}, type={item_type}, error={:?}, viewport={:?}",
                error.size(),
                viewport.size()
            );
            assert!(error.size().width <= viewport.size().width);
            assert_eq!(
                save.absolute_position().y,
                footer_y,
                "validation must not move the action row"
            );
            let error_y = error.absolute_position().y;
            viewport.scroll(0.0, -4096.0);
            assert!(
                error.absolute_position().y < error_y,
                "form must actually scroll"
            );
            assert!(
                error.absolute_position().y + error.size().height
                    <= viewport.absolute_position().y + viewport.size().height + 1.0
            );
            assert_eq!(save.absolute_position().y, footer_y);
            let before = saves.get();
            save.mock_single_click(PointerEventButton::Left);
            assert_eq!(
                saves.get(),
                before + 1,
                "Save must receive pointer input after scrolling"
            );
            let before = cancels.get();
            cancel.mock_single_click(PointerEventButton::Left);
            assert_eq!(cancels.get(), before + 1, "Cancel must remain reachable");
            assert!(!ui.get_item_dialog_visible());
        }
    }
    ui.hide().unwrap();
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
    tab_until(&ui, || {
        press(&ui, " ");
        toggles.get() > 0
    });
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
    let input = accessible_id(&ui, "category-dialog-input");
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
        tab_until(&ui, || {
            press(&ui, " ");
            toggles.get() > before
        });
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
