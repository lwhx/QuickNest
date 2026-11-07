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
    register_combination_launch_callback(&ui, &controller);
    register_combination_delete_callback(&ui, &controller);
    register_batch_launch_callback(&ui, &controller);
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
fn combination_failures_name_items_without_counting_them_as_started() {
    use quicknest::library_model::ItemKind;
    let (ui, controller, directory) = ui_fixture();
    let ids: Vec<_> = controller
        .borrow()
        .item_rows()
        .into_iter()
        .take(2)
        .map(|row| row.id)
        .collect();
    let command_id = controller.borrow().item_rows().last().unwrap().id.clone();
    {
        let mut state = controller.borrow_mut();
        for (index, id) in ids.iter().enumerate() {
            let item = state.library_mut().find_item_mut(id).unwrap();
            item.item_type = ItemKind::Exe;
            item.name = format!("缺失程序{}", index + 1);
            item.launch.path = directory
                .0
                .join(format!("missing-{index}.exe"))
                .to_string_lossy()
                .into_owned();
            item.launch.delay_seconds = 0;
        }
        let command = state.library_mut().find_item_mut(&command_id).unwrap();
        command.item_type = ItemKind::Command;
        command.launch.confirm_on_first_run = true;
        state
            .library_mut()
            .add_combination("失败反馈回归", ids.clone());
        state
            .library_mut()
            .add_combination("跳过与失败", vec![ids[0].clone(), command_id]);
    }
    controller.borrow().persist().unwrap();
    ui.set_combination_dialog_visible(true);
    ui.invoke_combination_launch_requested(0);
    let status = ui.get_combination_dialog_status().to_string();
    assert!(status.contains("0/2 个项目已启动"), "{status}");
    assert!(status.contains("2 个失败"), "{status}");
    assert!(
        status.contains("缺失程序1") && status.contains("缺失程序2"),
        "{status}"
    );
    assert!(ui.get_combination_dialog_error());

    ui.invoke_combination_launch_requested(1);
    let status = ui.get_combination_dialog_status().to_string();
    assert!(status.contains("1 个失败"), "{status}");
    assert!(status.contains("跳过 1 个未确认命令"), "{status}");
    for id in ids {
        assert_eq!(controller.borrow().item(&id).unwrap().launch_count, 0);
    }
    assert!(!directory.0.join("missing-0.exe").exists());
    assert!(!directory.0.join("missing-1.exe").exists());
    ui.hide().unwrap();
}

#[test]
fn batch_launch_includes_hidden_categories_in_its_results() {
    use quicknest::library_model::{Item, ItemKind, new_uuid};
    let (ui, controller, directory) = ui_fixture();
    let first_id = controller.borrow().item_rows()[0].id.clone();
    {
        let mut state = controller.borrow_mut();
        let first = state.library_mut().find_item_mut(&first_id).unwrap();
        first.item_type = ItemKind::Exe;
        first.launch.path = directory
            .0
            .join("missing-first.exe")
            .to_string_lossy()
            .into_owned();
        state.toggle_batch_selection(&first_id);
    }
    let category = controller
        .borrow_mut()
        .add_category("Other selected category")
        .unwrap();
    controller.borrow_mut().select_category(&category);
    let second_id = new_uuid();
    let second = Item::new(
        &second_id,
        "Second missing target",
        ItemKind::Exe,
        directory.0.join("missing-second.exe").to_string_lossy(),
    );
    controller
        .borrow_mut()
        .library_mut()
        .active_category_mut()
        .unwrap()
        .items
        .push(second);
    controller.borrow_mut().toggle_batch_selection(&second_id);
    refresh_ui(&controller, &ui.as_weak());
    assert_eq!(controller.borrow().batch_selection_ids().len(), 2);
    ui.invoke_item_menu_batch_launch();
    let status = ui.get_validation_status().to_string();
    assert!(status.contains("0/2 个项目已启动"), "{status}");
    assert!(status.contains("2 个失败"), "{status}");
    assert!(controller.borrow().batch_selection_ids().is_empty());
    assert_eq!(controller.borrow().item(&first_id).unwrap().launch_count, 0);
    assert_eq!(
        controller.borrow().item(&second_id).unwrap().launch_count,
        0
    );
    ui.hide().unwrap();
}

#[cfg(windows)]
#[test]
fn combination_delete_reports_commit_failure_and_retains_selection() {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
    let (ui, controller, directory) = ui_fixture();
    let item_id = controller.borrow().item_rows()[0].id.clone();
    controller
        .borrow_mut()
        .library_mut()
        .add_combination("Keep on failure", vec![item_id]);
    controller.borrow().persist().unwrap();
    ui.set_combination_dialog_items(ModelRc::new(VecModel::from(vec![
        "Keep on failure（1 项）".into(),
    ])));
    ui.set_combination_dialog_selected_index(0);
    ui.set_combination_dialog_selected("Keep on failure".into());
    let config = directory.0.join("config.json");
    let before = std::fs::read(&config).unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&config)
        .unwrap();
    ui.invoke_combination_delete_requested(0);
    assert!(ui.get_combination_dialog_error());
    assert!(ui.get_combination_dialog_status().starts_with("删除失败"));
    assert_eq!(controller.borrow().combination_rows().len(), 1);
    assert_eq!(ui.get_combination_dialog_selected_index(), 0);
    assert_eq!(ui.get_combination_dialog_items().row_count(), 1);
    assert_eq!(std::fs::read(&config).unwrap(), before);
    drop(lock);
    ui.invoke_combination_delete_requested(0);
    assert!(!ui.get_combination_dialog_error());
    assert!(ui.get_combination_dialog_status().starts_with("已删除组合"));
    assert!(controller.borrow().combination_rows().is_empty());
    assert_eq!(ui.get_combination_dialog_selected_index(), -1);
    assert_eq!(ui.get_combination_dialog_items().row_count(), 0);
    ui.hide().unwrap();
}

#[test]
fn restore_and_overwrite_import_reapply_pinning_appearance_and_blur_policy() {
    use quicknest::config_transfer::ImportMode;
    use quicknest::window_state::{BlurObservation, BlurRecheck, FocusStateMachine};
    let (ui, controller, directory) = ui_fixture();
    let machine = Rc::new(RefCell::new(FocusStateMachine::new()));
    machine.borrow_mut().show().unwrap();
    machine.borrow_mut().set_foreground_hint(false);
    {
        let mut state = controller.borrow_mut();
        let settings = &mut state.library_mut().settings;
        settings.pinned = true;
        settings.theme = "light".into();
        settings.auto_hide_on_blur = true;
    }
    controller.borrow().persist().unwrap();
    apply_runtime_settings(&ui, &controller, &machine);
    assert!(machine.borrow().is_pinned());
    assert!(ui.get_pin_toggled());
    let light = ui.get_theme().window;
    let mut restored = controller.borrow().library().clone();
    restored.settings.pinned = false;
    restored.settings.theme = "dark".into();
    restored.settings.opacity = 0.7;
    restored.settings.rounded_corners = false;
    restored.settings.backdrop = "mica".into();
    restored.settings.auto_hide_on_blur = false;
    restored.settings.hot_zone_enabled = true;
    restored.settings.hotkey = "Ctrl+Alt+F9".into();
    let backup = directory.0.join("settings-backup.json");
    std::fs::write(&backup, serde_json::to_string_pretty(&restored).unwrap()).unwrap();
    let message = restore_configuration_from_path(&ui, &controller, &machine, &backup).unwrap();
    assert!(message.contains("快捷键变更需重启"));
    assert!(!machine.borrow().is_pinned());
    assert!(!ui.get_pin_toggled());
    assert_eq!(ui.get_theme_mode(), "dark");
    assert_ne!(ui.get_theme().window, light);
    assert_eq!(ui.get_opacity_value(), "0.70");
    assert!(!ui.get_corners_on());
    assert_eq!(ui.get_backdrop_value(), "mica");
    assert_eq!(machine.borrow().observe_blur(), BlurObservation::Ignore);
    assert_eq!(
        machine.borrow().evaluate_blur_recheck(false),
        BlurRecheck::Stay
    );

    restored.settings.theme = "light".into();
    restored.settings.auto_hide_on_blur = true;
    let import_path = directory.0.join("settings-import.json");
    std::fs::write(
        &import_path,
        serde_json::to_string_pretty(&restored).unwrap(),
    )
    .unwrap();
    import_configuration_from_path(
        &ui,
        &controller,
        &machine,
        &import_path,
        ImportMode::Overwrite,
    )
    .unwrap();
    assert_eq!(ui.get_theme_mode(), "light");
    assert_eq!(ui.get_theme().window, light);
    assert!(!machine.borrow().is_pinned());
    assert_eq!(
        machine.borrow().evaluate_blur_recheck(false),
        BlurRecheck::Hide
    );
    let theme = ui.get_theme().window;
    let invalid = directory.0.join("invalid-import.json");
    std::fs::write(&invalid, "{broken").unwrap();
    assert!(
        import_configuration_from_path(&ui, &controller, &machine, &invalid, ImportMode::Overwrite)
            .is_err()
    );
    assert_eq!(ui.get_theme().window, theme);
    assert!(!machine.borrow().is_pinned());
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
