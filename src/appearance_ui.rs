use super::*;
use quicknest::library_controller::LibraryController;
use quicknest::library_model::Settings;

fn commit(
    ui: &MainWindow,
    controller: &Rc<RefCell<LibraryController>>,
    change: impl FnOnce(&mut Settings),
    status: String,
) {
    let previous = controller.borrow().library().settings.clone();
    change(&mut controller.borrow_mut().library_mut().settings);
    let saved = controller.borrow().persist();
    if saved.is_err() {
        controller.borrow_mut().library_mut().settings = previous;
    }
    apply_theme(ui, controller);
    refresh_ui(controller, &ui.as_weak());
    match saved {
        Ok(_) => ui.set_validation_status(status.into()),
        Err(error) => ui.set_validation_status(format!("保存失败：{error}").into()),
    }
}

pub(super) fn register(ui: &MainWindow, controller: &Rc<RefCell<LibraryController>>) {
    let weak = ui.as_weak();
    let state = Rc::clone(controller);
    ui.on_theme_menu_selected(move |mode| {
        let Some(ui) = weak.upgrade() else { return };
        let parsed = quicknest::theme::ThemeMode::parse(mode.as_ref());
        let label = match parsed {
            quicknest::theme::ThemeMode::Light => "浅色",
            quicknest::theme::ThemeMode::Dark => "深色",
            quicknest::theme::ThemeMode::System => "跟随系统",
        };
        commit(
            &ui,
            &state,
            |settings| settings.theme = parsed.as_str().to_string(),
            format!("主题：{label}"),
        );
    });

    let weak = ui.as_weak();
    let state = Rc::clone(controller);
    ui.on_accent_menu_selected(move |color| {
        let Some(ui) = weak.upgrade() else { return };
        let accent = quicknest::theme::normalize_accent(color.as_ref());
        let status = format!("强调色：{accent}");
        commit(
            &ui,
            &state,
            |settings| settings.accent_color = accent,
            status,
        );
    });

    let weak = ui.as_weak();
    let state = Rc::clone(controller);
    ui.on_opacity_menu_selected(move |value| {
        let Some(ui) = weak.upgrade() else { return };
        let opacity =
            quicknest::library_model::normalize_opacity(value.parse::<f64>().unwrap_or(0.95));
        commit(
            &ui,
            &state,
            |settings| settings.opacity = opacity,
            format!("透明度：{}%", (opacity * 100.0).round() as i32),
        );
    });

    let weak = ui.as_weak();
    let state = Rc::clone(controller);
    ui.on_corners_menu_selected(move |value| {
        let Some(ui) = weak.upgrade() else { return };
        let rounded = value.as_str() == "on";
        commit(
            &ui,
            &state,
            |settings| settings.rounded_corners = rounded,
            if rounded {
                "圆角：开"
            } else {
                "圆角：关"
            }
            .into(),
        );
    });

    let weak = ui.as_weak();
    let state = Rc::clone(controller);
    ui.on_backdrop_menu_selected(move |value| {
        let Some(ui) = weak.upgrade() else { return };
        let backdrop = quicknest::theme::normalize_backdrop(value.as_ref());
        commit(
            &ui,
            &state,
            |settings| settings.backdrop = backdrop.to_string(),
            format!("背景：{backdrop}"),
        );
    });
}
