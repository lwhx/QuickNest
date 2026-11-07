use super::*;
use i_slint_backend_testing::ElementHandle;
use slint::platform::PointerEventButton;

struct TestDirectory(std::path::PathBuf);

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn ui_fixture() -> (
    MainWindow,
    Rc<RefCell<quicknest::library_controller::LibraryController>>,
    TestDirectory,
) {
    i_slint_backend_testing::init_no_event_loop();
    let directory = TestDirectory(std::env::temp_dir().join(format!(
        "quicknest-ui-regression-{}",
        quicknest::library_model::new_uuid()
    )));
    std::fs::create_dir(&directory.0).unwrap();
    let controller = Rc::new(RefCell::new(
        quicknest::library_controller::LibraryController::load(Some(
            directory.0.join("config.json"),
        )),
    ));
    controller.borrow().persist().unwrap();
    let ui = MainWindow::new().unwrap();
    ui.window().set_size(slint::PhysicalSize::new(760, 520));
    register_settings_panel_callbacks(&ui, &controller);
    register_backup_open_callback(&ui, &controller, &Rc::new(RefCell::new(Vec::new())));
    register_transfer_open_callback(&ui);
    appearance_ui::register(&ui, &controller);
    refresh_ui(&controller, &ui.as_weak());
    ui.show().unwrap();
    (ui, controller, directory)
}

fn click_id(ui: &MainWindow, id: &str) {
    let element = ElementHandle::find_by_element_id(ui, &format!("MainWindow::{id}"))
        .next()
        .unwrap_or_else(|| panic!("missing UI element: {id}"));
    element.mock_single_click(PointerEventButton::Left);
}

#[test]
fn settings_close_buttons_and_child_dialogs_change_real_visibility() {
    let (ui, _controller, _directory) = ui_fixture();
    click_id(&ui, "settings-hit");
    assert!(
        ui.get_settings_panel_visible(),
        "title button must open settings"
    );
    click_id(&ui, "panel-close-hit");
    assert!(
        !ui.get_settings_panel_visible(),
        "top close button must hide settings"
    );

    ui.invoke_settings_open_requested();
    let close = ElementHandle::find_by_accessible_label(&ui, "关闭")
        .find(|element| {
            element.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Button)
        })
        .expect("bottom close button");
    close.mock_single_click(PointerEventButton::Left);
    assert!(
        !ui.get_settings_panel_visible(),
        "bottom close button must hide settings"
    );

    ui.invoke_settings_open_requested();
    ui.invoke_backup_menu_opened();
    assert!(!ui.get_settings_panel_visible());
    assert!(ui.get_backup_dialog_visible());
    ui.set_backup_dialog_visible(false);

    ui.invoke_settings_open_requested();
    ui.invoke_transfer_menu_opened();
    assert!(!ui.get_settings_panel_visible());
    assert!(ui.get_transfer_dialog_visible());
    ui.set_transfer_dialog_visible(false);

    ui.invoke_settings_open_requested();
    show_update_confirmation(&ui);
    assert!(!ui.get_settings_panel_visible());
    assert!(ui.get_update_confirm_visible());
    ui.hide().unwrap();
}

#[test]
fn appearance_callbacks_immediately_sync_selection_and_report_save_failure() {
    let (ui, controller, directory) = ui_fixture();
    ui.invoke_settings_open_requested();
    ui.invoke_theme_menu_selected("dark".into());
    assert_eq!(ui.get_theme_mode(), "dark");
    assert_eq!(controller.borrow().library().settings.theme, "dark");
    ui.invoke_accent_menu_selected("#16a34a".into());
    assert_eq!(ui.get_accent_value(), "#16a34a");
    ui.invoke_opacity_menu_selected("0.70".into());
    assert_eq!(ui.get_opacity_value(), "0.70");
    ui.invoke_corners_menu_selected("off".into());
    assert!(!ui.get_corners_on());
    ui.invoke_backdrop_menu_selected("mica".into());
    assert_eq!(ui.get_backdrop_value(), "mica");
    assert!(
        ui.get_settings_panel_visible(),
        "appearance changes must keep the panel open"
    );
    let persisted: quicknest::library_model::Library =
        serde_json::from_str(&std::fs::read_to_string(directory.0.join("config.json")).unwrap())
            .unwrap();
    assert_eq!(persisted.settings, controller.borrow().library().settings);

    let missing_parent = std::env::temp_dir().join(format!(
        "quicknest-ui-missing-{}",
        quicknest::library_model::new_uuid()
    ));
    let failing = Rc::new(RefCell::new(
        quicknest::library_controller::LibraryController::load(Some(
            missing_parent.join("config.json"),
        )),
    ));
    let before = failing.borrow().library().settings.clone();
    appearance_ui::register(&ui, &failing);
    ui.invoke_theme_menu_selected("light".into());
    assert_eq!(failing.borrow().library().settings, before);
    assert!(ui.get_validation_status().starts_with("保存失败"));
    assert!(!missing_parent.exists());
    ui.hide().unwrap();
}

#[test]
fn deleting_nonempty_category_prepares_confirmation_without_nested_borrow() {
    let controller = RefCell::new(quicknest::library_controller::LibraryController::load(None));
    let source = controller.borrow().active_category_id().unwrap();
    let before = controller.borrow().item_rows().len();
    assert!(before > 0);
    let (item_count, targets) = prepare_category_delete(&controller, &source).unwrap();
    assert_eq!(item_count, before);
    assert!(targets.iter().all(|row| row.id != source));
    assert_eq!(controller.borrow().item_rows().len(), before);
    assert!(controller.borrow_mut().select_category(&source));
}

#[test]
fn deleting_empty_category_needs_no_confirmation() {
    let controller = RefCell::new(quicknest::library_controller::LibraryController::load(None));
    let id = controller
        .borrow_mut()
        .add_category("Empty regression fixture")
        .unwrap();
    assert!(prepare_category_delete(&controller, &id).is_none());
    assert!(
        controller
            .borrow()
            .category_rows()
            .iter()
            .all(|row| row.id != id)
    );
}
