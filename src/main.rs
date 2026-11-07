#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::{Cell, RefCell};
use std::error::Error;
use std::rc::Rc;
use std::time::{Duration, Instant};

use quicknest::{DEMO_ITEMS, RunMode, USAGE, parse_args, updater_validation};
use slint::{ComponentHandle, Model, ModelRc, Timer, TimerMode, VecModel};

slint::include_modules!();

/// P1-18: shared focus state machine (UI-thread only).
type SharedFocusMachine = Rc<RefCell<quicknest::window_state::FocusStateMachine>>;

/// P2-12 (baseline 16.1): resolves `settings.theme` (light/dark/system) and
/// the accent color, then pushes the semantic palette into the UI. Called at
/// startup, on every theme/accent change and when the system theme changes.
fn apply_theme(
    ui: &MainWindow,
    controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
) {
    let (mode, accent, opacity, rounded, backdrop) = {
        let library = controller.borrow();
        let settings = &library.library().settings;
        (
            quicknest::theme::ThemeMode::parse(&settings.theme),
            quicknest::theme::normalize_accent(&settings.accent_color),
            quicknest::library_model::normalize_opacity(settings.opacity),
            settings.rounded_corners,
            quicknest::theme::Backdrop::parse(&settings.backdrop),
        )
    };
    let dark = mode.is_dark(quicknest::theme::system_theme_is_dark());
    let palette = quicknest::theme::palette(dark, &accent);
    // Keep Slint's std-widgets (dialogs, buttons, editors) on the same scheme.
    ui.set_dark_mode(dark);
    let color = |value: quicknest::theme::Rgb| slint::Color::from_rgb_u8(value.0, value.1, value.2);
    // P2-14 (baseline 16.3): apply the persisted DWM system backdrop
    // (Mica/Acrylic/none). The DWM call is the capability probe: unsupported
    // systems fail it and keep the plain solid color. The client keeps the
    // palette colors; with `renderer-software` the panel pixels are opaque, so
    // the material composes behind them (DWM-managed regions such as rounded
    // corners still show it).
    #[cfg(windows)]
    if let Ok(observation) =
        quicknest::infrastructure::windows::window_handle::inspect_window(ui.window())
    {
        let appearance_backdrop = match backdrop {
            quicknest::theme::Backdrop::None => {
                quicknest::infrastructure::windows::appearance::Backdrop::None
            }
            quicknest::theme::Backdrop::Mica => {
                quicknest::infrastructure::windows::appearance::Backdrop::Mica
            }
            quicknest::theme::Backdrop::Acrylic => {
                quicknest::infrastructure::windows::appearance::Backdrop::Acrylic
            }
        };
        let _ = quicknest::infrastructure::windows::appearance::apply_backdrop(
            observation.hwnd,
            appearance_backdrop,
        );
    }
    let theme_colors = ThemeColors {
        window: color(palette.window),
        panel: color(palette.panel),
        sidebar: color(palette.sidebar),
        border: color(palette.border),
        text: color(palette.text),
        secondary: color(palette.secondary),
        muted: color(palette.muted),
        subtle: color(palette.subtle),
        accent: color(palette.accent),
        accent_soft: color(palette.accent_soft),
        card_hover: color(palette.card_hover),
        card_hover_border: color(palette.card_hover_border),
        card_pressed: color(palette.card_pressed),
        sidebar_hover: color(palette.sidebar_hover),
        sidebar_active: color(palette.sidebar_active),
        title_hover: color(palette.title_hover),
        alt_row: color(palette.alt_row),
        danger: color(palette.danger),
        warning: color(palette.warning),
    };
    ui.set_theme(theme_colors.clone());
    // 第三批：顶层组件（PillButton）经全局上下文读取同一主题。
    ui.global::<UiContext>().set_theme(theme_colors);
    // P2-13 (baseline 16.2): window opacity + DWM rounded corners are
    // refreshed last so both values land on the shown window.
    #[cfg(windows)]
    if let Ok(observation) =
        quicknest::infrastructure::windows::window_handle::inspect_window(ui.window())
    {
        quicknest::infrastructure::windows::appearance::apply_window_effects(
            observation.hwnd,
            opacity,
            rounded,
        );
    }
}

#[cfg(windows)]
mod dpi_scale_validation;
mod dpi_validation;
#[cfg(windows)]
mod drop_target_validation;
#[cfg(windows)]
mod file_drop_validation;
#[cfg(windows)]
mod focus_validation;
#[cfg(windows)]
mod hotkey_validation;
#[cfg(windows)]
mod hwnd_validation;
#[cfg(windows)]
mod ipc_validation;
#[cfg(windows)]
mod mutex_validation;
#[cfg(windows)]
mod ole_validation;
#[cfg(windows)]
mod save_validation;
#[cfg(windows)]
mod shell_validation;
mod transfer_validation;
#[cfg(windows)]
mod tray_validation;

fn select_backend() -> Result<(), slint::PlatformError> {
    let selector = slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into());
    #[cfg(windows)]
    let selector = {
        use slint::winit_030::winit::platform::windows::WindowAttributesExtWindows;
        // QuickNest owns OLE registration. Never revoke winit's target after creation.
        // The frameless custom title bar (baseline 2.5) is declared here at creation
        // time — the most reliable path on Windows; the .slint `no-frame: true` keeps
        // the property in sync for runtime updates.
        selector.with_winit_window_attributes_hook(|attributes| {
            attributes.with_drag_and_drop(false).with_decorations(false)
            // PROBE: resizable state is untouched by update_window_properties;
            // if its bit disappears from GWL_STYLE the hook path is live.
        })
    };
    selector.select()
}

fn create_window() -> Result<MainWindow, slint::PlatformError> {
    let ui = MainWindow::new()?;
    // P0 validation windows keep the six demo tiles; the interactive mode
    // replaces them with the active category's projects in `refresh_ui`.
    let items: Vec<GridItem> = DEMO_ITEMS
        .iter()
        .map(|(name, _detail)| GridItem {
            id: "".into(),
            name: (*name).into(),
            path: "".into(),
            note: "".into(),
            item_type: "演示".into(),
            path_missing: false,
            missing_reason: String::new().into(),
            artwork: "app".into(),
            batch_selected: false,
            favorite: false,
            launch_count: 0,
            last_used: "".into(),
            icon: slint::Image::default(),
        })
        .collect();
    ui.set_items(ModelRc::new(VecModel::from(items)));
    Ok(ui)
}

/// Exposes the validation window's native handle for close-request wiring in
/// validation modules that run their own event loop on the main thread.
pub fn ui_window(ui: &MainWindow) -> &slint::Window {
    ui.window()
}

/// P1-10: UI-side icon state. A worker thread extracts icons; this struct
/// caches the resulting `slint::Image`s and deduplicates pending requests.
struct IconContext {
    cache: RefCell<std::collections::HashMap<String, slint::Image>>,
    pending: RefCell<std::collections::HashSet<String>>,
    #[cfg(windows)]
    service: Option<quicknest::infrastructure::windows::icons::IconService>,
}

impl IconContext {
    fn new(cache_dir: std::path::PathBuf) -> Self {
        #[cfg(not(windows))]
        let _ = cache_dir;
        Self {
            cache: RefCell::new(std::collections::HashMap::new()),
            pending: RefCell::new(std::collections::HashSet::new()),
            #[cfg(windows)]
            service: Some(quicknest::infrastructure::windows::icons::IconService::start(cache_dir)),
        }
    }

    fn request(&self, key: &str) {
        #[cfg(windows)]
        if let Some(service) = &self.service {
            service.request(key, key);
        }
        #[cfg(not(windows))]
        let _ = key;
    }

    /// Joins finished extractions into the cache; true when a new icon arrived.
    fn drain(&self) -> bool {
        #[cfg(windows)]
        {
            let Some(service) = &self.service else {
                return false;
            };
            let mut updated = false;
            while let Some(result) = service.try_recv() {
                let image = match result.pixels {
                    Some(pixels) => {
                        let mut buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(
                            pixels.width,
                            pixels.height,
                        );
                        let slice = buffer.make_mut_slice();
                        for (pixel, source) in slice.iter_mut().zip(pixels.rgba.chunks_exact(4)) {
                            pixel.r = source[0];
                            pixel.g = source[1];
                            pixel.b = source[2];
                            pixel.a = source[3];
                        }
                        slint::Image::from_rgba8(buffer)
                    }
                    None => slint::Image::default(),
                };
                self.cache.borrow_mut().insert(result.key.clone(), image);
                self.pending.borrow_mut().remove(&result.key);
                updated = true;
            }
            updated
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
}

thread_local! {
    /// The UI-thread icon context; `refresh_ui` and the drain timer read it.
    static ICON_CONTEXT: RefCell<Option<Rc<IconContext>>> = const { RefCell::new(None) };
}

fn install_icon_context(context: Rc<IconContext>) {
    ICON_CONTEXT.with(|slot| *slot.borrow_mut() = Some(context));
}

fn icon_context() -> Option<Rc<IconContext>> {
    ICON_CONTEXT.with(|slot| slot.borrow().clone())
}

fn drain_icon_results() -> bool {
    ICON_CONTEXT.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|context| context.drain())
    })
}

/// P1-11: drops cache entries no item references anymore. Shared icons stay
/// while any item still references them (baseline 12.3).
#[cfg(windows)]
fn prune_icon_cache(
    directory: &std::path::Path,
    controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
) {
    use quicknest::infrastructure::windows::icons::{ICON_SIZE, cache_key, prune_cache};
    let mut keep = std::collections::HashSet::new();
    for group in &controller.borrow().library().groups {
        for category in &group.categories {
            for item in &category.items {
                if let Some(source) = item.icon_source() {
                    keep.insert(cache_key(&source, ICON_SIZE));
                }
            }
        }
    }
    let _ = prune_cache(directory, &keep);
}

#[cfg(not(windows))]
fn prune_icon_cache(
    _directory: &std::path::Path,
    _controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
) {
}

/// P2-15 (baseline 16.4): the device name of the display the panel is on.
#[cfg(windows)]
fn current_monitor_device(ui: &MainWindow) -> Option<String> {
    use quicknest::infrastructure::windows::placement;
    let observation =
        quicknest::infrastructure::windows::window_handle::inspect_window(ui.window()).ok()?;
    let monitors = placement::monitor_areas();
    let rect = placement::window_rect(observation.hwnd)?;
    let index = quicknest::placement::nearest_monitor(rect, &monitors)?;
    let device = monitors[index].device.clone();
    if device.is_empty() {
        None
    } else {
        Some(device)
    }
}

/// P2-15 (baseline 16.4): keeps the panel on a connected display.
///
/// The window may end up outside every work area when the display it was on
/// is unplugged while the app runs. The correction moves it back into the
/// nearest connected display, so the panel can never stay invisible.
#[cfg(windows)]
fn correct_window_placement(ui: &MainWindow) {
    use quicknest::infrastructure::windows::placement;
    if let Ok(observation) =
        quicknest::infrastructure::windows::window_handle::inspect_window(ui.window())
    {
        let _ = placement::correct_window_position(observation.hwnd);
    }
}

/// P2-15 (baseline 16.4): restores the position the user left the panel at.
///
/// The persisted bounds are physical pixels of the display that owned them.
/// When that display is gone the position is corrected into the nearest
/// connected display instead of restoring the window off-screen.
#[cfg(windows)]
fn restore_window_placement(
    ui: &MainWindow,
    controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
) {
    use quicknest::infrastructure::windows::placement;
    use quicknest::placement::Rect;
    let Some(bounds) = controller.borrow().library().settings.window_bounds.clone() else {
        return;
    };
    let Ok(observation) =
        quicknest::infrastructure::windows::window_handle::inspect_window(ui.window())
    else {
        return;
    };
    let Some(current) = placement::window_rect(observation.hwnd) else {
        return;
    };
    let monitors = placement::monitor_areas();
    if monitors.is_empty() {
        return;
    }
    let target = Rect::from_size_at(bounds.x, bounds.y, current.size());
    let Some(resolved) = quicknest::placement::resolve_placement(target, &monitors) else {
        return;
    };
    let _ = placement::set_window_position(observation.hwnd, resolved.left, resolved.top);
}

/// P3-01: whether the Ctrl key is physically down. The batch selection
/// uses Ctrl+click as a shortcut; the context menu offers the same action for
/// users (and for the acceptance scripts, where injected key presses never
/// reach the system).
enum DelayedLaunchOutcome {
    Started { item_id: String },
    Failed { item_id: String, reason: String },
}

static DELAYED_TASKS: std::sync::OnceLock<
    quicknest::launch_tasks::DelayedTasks<DelayedLaunchOutcome>,
> = std::sync::OnceLock::new();

fn delayed_tasks() -> &'static quicknest::launch_tasks::DelayedTasks<DelayedLaunchOutcome> {
    DELAYED_TASKS.get_or_init(quicknest::launch_tasks::DelayedTasks::new)
}

#[cfg(windows)]
#[derive(Default)]
struct UpdateUiState {
    busy: bool,
    pending: Option<quicknest::updater::VerifiedUpdate>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(windows)]
impl UpdateUiState {
    fn finish_check(&mut self, cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>) -> bool {
        if cancel.load(std::sync::atomic::Ordering::SeqCst)
            || !std::sync::Arc::ptr_eq(&self.cancel, cancel)
        {
            return false;
        }
        self.busy = false;
        true
    }
}

#[cfg(windows)]
thread_local! {
    static UPDATE_UI: RefCell<UpdateUiState> = RefCell::new(UpdateUiState::default());
}

#[cfg(windows)]
fn start_update_check(
    ui: &MainWindow,
    controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
    manual: bool,
) {
    let pending = UPDATE_UI.with(|state| {
        let state = state.borrow();
        state
            .pending
            .as_ref()
            .map(|update| (update.version().to_string(), update.notes().to_string()))
    });
    if let Some((version, notes)) = pending {
        if manual {
            ui.set_update_confirm_version(version.into());
            ui.set_update_confirm_notes(notes.into());
            ui.set_update_confirm_status(
                "签名和文件校验已通过。确认后将退出并安装，失败保留恢复报告。".into(),
            );
            ui.set_update_confirm_visible(true);
        }
        return;
    }
    let url = controller.borrow().update_url();
    if url.trim().is_empty() {
        if manual {
            ui.set_validation_status("尚未配置 HTTPS 更新清单地址".into());
        }
        return;
    }
    let cancel = UPDATE_UI.with(|state| {
        let mut state = state.borrow_mut();
        if state.busy {
            return None;
        }
        state.busy = true;
        state.cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        Some(std::sync::Arc::clone(&state.cancel))
    });
    let Some(cancel) = cancel else {
        if manual {
            ui.set_validation_status("正在检查更新，请稍候".into());
        }
        return;
    };
    ui.set_update_check_busy(true);
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let result = quicknest::updater::check_download(&url, env!("CARGO_PKG_VERSION"), &cancel);
        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if !UPDATE_UI.with(|state| state.borrow_mut().finish_check(&cancel)) {
                return;
            }
            ui.set_update_check_busy(false);
            match result {
                Ok(Some(update)) => {
                    let version = update.version().to_string();
                    let notes = update.notes().to_string();
                    UPDATE_UI.with(|state| state.borrow_mut().pending = Some(update));
                    ui.set_update_confirm_version(version.clone().into());
                    ui.set_update_confirm_notes(notes.into());
                    ui.set_update_confirm_status(
                        "签名和 SHA-256 校验已通过。请确认是否退出并安装。".into(),
                    );
                    if manual {
                        ui.set_update_confirm_visible(true);
                    }
                    ui.set_validation_status(
                        format!("发现新版本 {version}，请从检查更新入口确认安装").into(),
                    );
                }
                Ok(None) => {
                    if manual {
                        ui.set_validation_status("当前已是最新版本".into());
                    }
                }
                Err(error) => {
                    if manual {
                        ui.set_validation_status(format!("检查更新失败：{error}").into());
                    }
                }
            }
        });
    });
}

/// MTA COM guard for worker threads that call COM-backed launch paths.
struct ComMtaGuard {
    owns_initialization: bool,
}

impl ComMtaGuard {
    fn new() -> Self {
        #[cfg(windows)]
        {
            // SAFETY: Balanced CoInitializeEx/CoUninitialize pair per thread.
            let hr = unsafe {
                windows_sys::Win32::System::Com::CoInitializeEx(
                    std::ptr::null_mut(),
                    windows_sys::Win32::System::Com::COINIT_MULTITHREADED as u32,
                )
            };
            // Both S_OK and S_FALSE acquire a COM reference; failed calls do not.
            Self {
                owns_initialization: hr >= 0,
            }
        }
        #[cfg(not(windows))]
        Self {
            owns_initialization: false,
        }
    }
}

impl Drop for ComMtaGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        if self.owns_initialization {
            unsafe {
                windows_sys::Win32::System::Com::CoUninitialize();
            }
        }
    }
}

/// P3-03 (baseline 22 延时启动): starts one item, honoring its per-item
/// delay. A delayed launch runs on a detached thread so the UI and the
/// remaining batch items are not blocked; the returned flag is `true` for
/// the immediate part and the caller counts delayed items separately.
#[cfg(windows)]
fn launch_item_maybe_delayed(
    controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
    item_id: &str,
    as_admin: bool,
) -> (bool, u32) {
    use quicknest::infrastructure::windows::launch::{LaunchOutcome, launch_item};
    let item = controller.borrow().item(item_id);
    let Some(item) = item else {
        return (false, 0);
    };
    let delay = item.launch.delay_seconds;
    if delay == 0 {
        let started = matches!(launch_item(&item, as_admin), LaunchOutcome::Started { .. });
        return (started, 0);
    }
    // Detached thread: the timer API requires the main thread, and the batch
    // loop must not sleep the UI thread.
    // P4 审核修复（2026-11-07）：排队不再等于成功——线程真实启动后才经
    // UI 排空定时器记录 usage；失败回传状态栏（旧实现吞掉结果并提前
    // 计数）。线程内补 COM 初始化（MTA），否则 AUMID 直接激活失败。
    let admin = as_admin;
    let item_id_owned = item_id.to_string();
    let queued = delayed_tasks().schedule(Duration::from_secs(u64::from(delay)), move || {
        let _com = ComMtaGuard::new();
        match launch_item(&item, admin) {
            LaunchOutcome::Started { .. } => DelayedLaunchOutcome::Started {
                item_id: item_id_owned,
            },
            LaunchOutcome::Failed(error) => DelayedLaunchOutcome::Failed {
                item_id: item_id_owned,
                reason: error.message(),
            },
        }
    });
    (queued, delay)
}

#[cfg(not(windows))]
fn launch_item_maybe_delayed(
    _controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
    _item_id: &str,
    _as_admin: bool,
) -> (bool, u32) {
    (false, 0)
}

#[cfg(windows)]
fn control_key_held() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyState;
    // SAFETY: GetKeyState only reads the calling thread's key state.
    unsafe { (GetKeyState(0x11) as u16 & 0x8000) != 0 }
}

#[cfg(not(windows))]
fn control_key_held() -> bool {
    false
}

/// P1-14: brings the window back and asks Windows to focus it after the
/// global hotkey fires (baseline 11.1).
#[cfg(windows)]
fn show_and_focus(ui: &MainWindow) {
    if ui.show().is_err() {
        return;
    }
    // P2-15: a display may have been unplugged while the panel was hidden.
    correct_window_placement(ui);
    if let Ok(observation) =
        quicknest::infrastructure::windows::window_handle::inspect_window(ui.window())
    {
        // SAFETY: inspect_window validated that the HWND belongs to this
        // process and UI thread immediately above.
        unsafe {
            let _ = windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(
                observation.hwnd as *mut core::ffi::c_void,
            );
        }
    }
}

#[cfg(not(windows))]
fn show_and_focus(ui: &MainWindow) {
    let _ = ui.show();
}

/// P1-18: every user-visible show path also advances the focus state machine
/// (baseline 10.1) so blur checks only run while the panel is Visible.
fn show_panel(ui: &MainWindow, machine: &SharedFocusMachine) {
    let _ = machine.borrow_mut().show();
    show_and_focus(ui);
}

/// P1-18: every hide path updates the machine before hiding the window.
fn hide_panel(ui: &MainWindow, machine: &SharedFocusMachine) {
    let _ = machine.borrow_mut().hide();
    let _ = ui.hide();
}

/// P1-16: user-facing prompt for single-instance failure paths that leave no
/// main window (baseline 9.5). The release build has no console, so a message
/// box is the only visible report.
#[cfg(windows)]
fn notify_single_instance_failure(message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
    let caption: Vec<u16> = "QuickNest".encode_utf16().chain(Some(0)).collect();
    // SAFETY: Both buffers are NUL-terminated and outlive the call; a null
    // owner window is valid for a modal message box.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

/// P1-15: routes tray events into the UI (baseline 11.3).
#[cfg(windows)]
fn handle_tray_event(
    weak: &slint::Weak<MainWindow>,
    controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
    machine: &SharedFocusMachine,
    menu_suppression: &Rc<RefCell<Option<quicknest::window_state::SuppressionToken>>>,
    event: quicknest::infrastructure::windows::tray::TrayEvent,
) {
    use quicknest::infrastructure::windows::tray::{TrayCommand, TrayEvent};
    use quicknest::window_state::SuppressionReason;
    match event {
        TrayEvent::MenuOpened => {
            // Baseline 10.5: a native menu holds a context-menu suppression.
            let mut token = menu_suppression.borrow_mut();
            if token.is_none() {
                *token = Some(machine.borrow().acquire(SuppressionReason::ContextMenuOpen));
            }
        }
        TrayEvent::MenuDismissed => {
            menu_suppression.borrow_mut().take();
        }
        TrayEvent::LeftClick | TrayEvent::DoubleClick => {
            if let Some(ui) = weak.upgrade() {
                if ui.window().is_visible() {
                    hide_panel(&ui, machine);
                    ui.set_validation_status("托盘：已隐藏".into());
                } else {
                    show_panel(&ui, machine);
                    ui.set_validation_status("托盘：已显示".into());
                }
            }
        }
        TrayEvent::MenuSelected { command } => {
            menu_suppression.borrow_mut().take();
            match command {
                TrayCommand::Show => {
                    if let Some(ui) = weak.upgrade() {
                        show_panel(&ui, machine);
                    }
                }
                TrayCommand::Hide => {
                    if let Some(ui) = weak.upgrade() {
                        hide_panel(&ui, machine);
                    }
                }
                TrayCommand::TogglePin => {
                    if let Some(ui) = weak.upgrade() {
                        let next = !controller.borrow().library().settings.pinned;
                        controller.borrow_mut().library_mut().settings.pinned = next;
                        let _ = controller.borrow().persist();
                        machine.borrow_mut().set_pinned(next);
                        ui.set_pin_toggled(next);
                        ui.set_validation_status(if next {
                            "已固定（托盘，不自动隐藏）".into()
                        } else {
                            "已取消固定（托盘）".into()
                        });
                    }
                }
                TrayCommand::Settings => {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_validation_status("请使用标题栏「设置」调整主题与外观".into());
                    }
                }
                TrayCommand::CheckUpdate => {
                    // P4-04（基线 23 章）：真实检查更新——后台线程取清单、
                    // 验签、比较版本；发现新版本时下载并验证，由用户确认安装
                    //（默认不静默）。
                    if let Some(ui) = weak.upgrade() {
                        let enabled = controller.borrow().update_check_enabled();
                        if !enabled {
                            ui.set_validation_status("更新检查已在设置中关闭".into());
                        } else {
                            ui.set_validation_status("正在检查更新…".into());
                            start_update_check(&ui, controller, true);
                        }
                    }
                }
                TrayCommand::Exit => {
                    // Baseline 11.3 exit order starts by saving the config; the
                    // remaining steps happen through Drop (hotkey unregister,
                    // tray icon removal, background threads) as main returns.
                    let _ = controller.borrow().persist();
                    machine.borrow_mut().begin_shutdown();
                    let _ = slint::quit_event_loop();
                }
            }
        }
        _ => {}
    }
}

/// P1-16: applies one IPC command to the window (baseline 9.4).
#[cfg(windows)]
fn handle_ipc_command(
    ui: &MainWindow,
    machine: &SharedFocusMachine,
    command: quicknest::infrastructure::windows::ipc::IpcCommand,
) {
    use quicknest::infrastructure::windows::ipc::IpcCommand;
    match command {
        IpcCommand::Show | IpcCommand::ShowAndFocus => show_panel(ui, machine),
        IpcCommand::Hide => hide_panel(ui, machine),
        IpcCommand::Toggle => {
            if ui.window().is_visible() {
                hide_panel(ui, machine);
            } else {
                show_panel(ui, machine);
            }
        }
        IpcCommand::OpenSearch => {
            ui.set_validation_status("搜索快捷键已触发（搜索中心属 P2-01）".into());
        }
    }
}

/// Opens the native file picker owned by the QuickNest window (P1-05).
#[cfg(windows)]
fn browse_for_item(ui: &MainWindow) -> Option<std::path::PathBuf> {
    use quicknest::infrastructure::windows::{dialogs, window_handle};
    let owner = window_handle::inspect_window(ui.window())
        .map(|observation| observation.hwnd)
        .unwrap_or(0);
    dialogs::pick_file(owner)
}

#[cfg(not(windows))]
fn browse_for_item(_ui: &MainWindow) -> Option<std::path::PathBuf> {
    None
}

/// Launches a project through the P1-06 launch module and reports the outcome
/// in the status bar (baseline 4: failures are never silent).
/// P3-09 (baseline 22 Everything 可选集成): hands the search query to
/// Everything (`Everything.exe -search <query>`). A missing installation is
/// reported without affecting the main program.
#[cfg(windows)]
fn launch_everything_handoff(
    ui: &MainWindow,
    controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
    query: &str,
) {
    use quicknest::infrastructure::windows::launch::{
        LaunchOutcome, everything_fallback_paths, everything_handoff_item, launch_item,
        resolve_everything_path,
    };
    let configured = controller
        .borrow()
        .library()
        .settings
        .everything_path
        .clone();
    let Some(path) = resolve_everything_path(&configured, &everything_fallback_paths()) else {
        ui.set_validation_status("未找到 Everything，请检查安装路径".into());
        return;
    };
    match launch_item(&everything_handoff_item(&path, query), false) {
        LaunchOutcome::Started { .. } => {
            ui.set_validation_status(format!("已移交 Everything 搜索：{query}").into());
        }
        LaunchOutcome::Failed(error) => {
            ui.set_validation_status(error.message().into());
        }
    }
}

#[cfg(not(windows))]
fn launch_everything_handoff(
    _ui: &MainWindow,
    _controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
    _query: &str,
) {
}

/// P3-08 (baseline 8.2): the user-facing label of a command execution mode.
fn execution_mode_label(mode: &str) -> &'static str {
    match quicknest::library_model::normalize_execution_mode(mode) {
        "cmd" => "CMD（cmd.exe /d /s /c）",
        "powershell" => "PowerShell（powershell.exe -NoProfile -File）",
        _ => "直接执行（CreateProcessW，不经过 cmd.exe）",
    }
}

/// P3-08 (baseline 8.3): single-item launches gate unverified command items
/// behind the first-run confirmation dialog; everything else starts
/// directly. The dialog shows program, arguments, working directory and the
/// execution mode, and the user may cancel.
#[cfg(windows)]
fn launch_item_with_confirm(
    ui: &MainWindow,
    controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
    machine: &SharedFocusMachine,
    pending: &Rc<RefCell<Option<(String, bool)>>>,
    item_id: &str,
    as_admin: bool,
) {
    use quicknest::library_model::ItemKind;
    let unverified = controller.borrow().item(item_id).is_some_and(|item| {
        item.item_type == ItemKind::Command && item.launch.confirm_on_first_run
    });
    if !unverified {
        launch_item_by_id(ui, controller, machine, item_id, as_admin);
        return;
    }
    let Some(item) = controller.borrow().item(item_id) else {
        return;
    };
    pending
        .borrow_mut()
        .replace((item_id.to_string(), as_admin));
    ui.set_command_confirm_program(item.launch.path.trim().to_string().into());
    ui.set_command_confirm_args(item.launch.args.join(" ").into());
    ui.set_command_confirm_workdir(
        if item.launch.workdir.trim().is_empty() {
            "（空）".to_string()
        } else {
            item.launch.workdir.clone()
        }
        .into(),
    );
    ui.set_command_confirm_mode(execution_mode_label(&item.launch.execution_mode).into());
    ui.set_command_confirm_visible(true);
}

#[cfg(not(windows))]
fn launch_item_with_confirm(
    _ui: &MainWindow,
    _controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
    _machine: &SharedFocusMachine,
    _pending: &Rc<RefCell<Option<(String, bool)>>>,
    _item_id: &str,
    _as_admin: bool,
) {
}

fn refresh_ui(
    controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
    weak: &slint::Weak<MainWindow>,
) {
    let Some(ui) = weak.upgrade() else {
        return;
    };
    // P3-09: the Everything toggle state drives the settings menu.
    ui.set_everything_enabled(controller.borrow().library().settings.everything_enabled);
    ui.set_update_check_enabled(controller.borrow().library().settings.update_check_enabled);
    let views: Vec<GroupView> = controller
        .borrow()
        .group_rows()
        .into_iter()
        .map(|row| GroupView {
            id: row.id.clone().into(),
            name: row.name.into(),
            accent: row.color.into(),
            active: row.active,
            item_count: row.item_count as i32,
            tab_width: row.tab_width,
        })
        .collect();
    let categories: Vec<CategoryView> = controller
        .borrow()
        .category_rows()
        .into_iter()
        .map(|row| CategoryView {
            id: row.id.clone().into(),
            name: row.name.into(),
            item_count: row.item_count as i32,
        })
        .collect();
    ui.set_groups(ModelRc::new(VecModel::from(views)));
    ui.set_categories(ModelRc::new(VecModel::from(categories)));
    let names: Vec<slint::SharedString> = controller
        .borrow()
        .category_rows()
        .iter()
        .map(|row| row.name.clone().into())
        .collect();
    ui.set_category_names(ModelRc::new(VecModel::from(names)));
    ui.set_active_category_id(
        controller
            .borrow()
            .library()
            .active_category()
            .map(|category| category.id.clone())
            .unwrap_or_default()
            .into(),
    );
    // P1-04 grid rows with P1-10 icons from the UI-side cache; missing
    // icons are queued once and use the default artwork.
    let icon_context = icon_context();
    let rows = controller.borrow().item_rows();
    let items: Vec<GridItem> = rows
        .into_iter()
        .map(|row| {
            let icon = match (&row.icon_source, &icon_context) {
                (Some(source), Some(context)) => {
                    match context.cache.borrow().get(source).cloned() {
                        Some(image) => image,
                        None => {
                            if context.pending.borrow_mut().insert(source.clone()) {
                                context.request(source);
                            }
                            slint::Image::default()
                        }
                    }
                }
                _ => slint::Image::default(),
            };
            // P3-01: the batch selection is shown on the card.
            let batch_selected = controller.borrow().is_batch_selected(&row.id);
            GridItem {
                id: row.id.into(),
                name: row.name.into(),
                path: row.path.into(),
                note: row.note.into(),
                item_type: row.item_type.into(),
                path_missing: row.path_missing,
                missing_reason: row.missing_reason.into(),
                artwork: row.artwork.into(),
                batch_selected,
                favorite: row.favorite,
                launch_count: row.launch_count as i32,
                last_used: (if row.last_used > 0 {
                    // Local-time display needs Win32; the tooltip only
                    // needs a stable marker, so show unix minutes.
                    format!("{}", row.last_used / 60_000)
                } else {
                    String::new()
                })
                .into(),
                icon,
            }
        })
        .collect();
    ui.set_items(ModelRc::new(VecModel::from(items)));
    let icon_size = controller.borrow().icon_size();
    let (cell_width, cell_height) = icon_size.cell();
    ui.set_cell_width(cell_width);
    ui.set_cell_height(cell_height);
    ui.set_icon_px(icon_size.icon_px());
    // UI 体验修复（2026-11-07）：设置菜单 checkable 与底栏“视图”的状态快照。
    // 各设置回调最终都会调用 refresh_ui，因此在这里统一同步。
    let settings_snapshot = controller.borrow().library().settings.clone();
    ui.set_theme_mode(
        quicknest::theme::ThemeMode::parse(&settings_snapshot.theme)
            .as_str()
            .into(),
    );
    ui.set_accent_value(quicknest::theme::normalize_accent(&settings_snapshot.accent_color).into());
    ui.set_opacity_value(format!("{:.2}", settings_snapshot.opacity).into());
    ui.set_corners_on(settings_snapshot.rounded_corners);
    ui.set_backdrop_value(
        quicknest::theme::Backdrop::parse(&settings_snapshot.backdrop)
            .as_str()
            .into(),
    );
    ui.set_sort_mode_value(settings_snapshot.sort_mode.clone().into());
    ui.set_icon_size_preset(icon_size.key().into());
    // 第三批：分组栏横向滚动的 tabs 总内容宽度（drop_layout 命中补偿用）。
    let group_tabs_width: f32 = controller
        .borrow()
        .group_rows()
        .iter()
        .map(|row| row.tab_width)
        .sum::<f32>();
    let group_tabs_spacing = controller.borrow().group_rows().len().saturating_sub(1) as f32 * 24.0;
    ui.set_group_tabs_content_width(group_tabs_width + group_tabs_spacing);
    ui.set_active_item_id("".into());
    // P1-12: save failures are surfaced instead of being ignored;
    // a clear error restores the default status text.
    match controller.borrow().save_error() {
        Some(error) => ui.set_validation_status(format!("保存失败：{error}").into()),
        None if ui.get_validation_status().starts_with("保存失败") => {
            ui.set_validation_status("基础工程 · 仅演示图标，不启动项目".into());
        }
        None => {}
    }
}

#[cfg(windows)]
fn launch_item_by_id(
    ui: &MainWindow,
    controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
    machine: &SharedFocusMachine,
    item_id: &str,
    as_admin: bool,
) {
    use quicknest::infrastructure::windows::launch::{LaunchOutcome, launch_item};
    use quicknest::window_state::SuppressionReason;
    let Some(item) = controller.borrow().item(item_id) else {
        ui.set_validation_status("项目不存在".into());
        return;
    };
    let name = item.name.clone();
    // Baseline 10.6: the UAC secure desktop takes the foreground; hold a
    // suppression lock so a cancelled elevation cannot blur-hide the panel.
    let elevated = as_admin.then(|| {
        machine
            .borrow()
            .acquire(SuppressionReason::LaunchingElevated)
    });
    let outcome = launch_item(&item, as_admin);
    drop(elevated);
    match outcome {
        LaunchOutcome::Started { .. } => {
            // P3-05/P3-06: record the successful launch for recent/frequent
            // sorting.
            // P4 审核修复：记录后立即刷新模型——最近/高频模式下网格顺序
            // 即时更新（旧实现要等下一次全量刷新）。
            controller.borrow_mut().record_launch(&item.id);
            if controller.borrow().sort_mode() != "manual" {
                refresh_ui(controller, &ui.as_weak());
            }
            let (auto_hide, pinned) = {
                let library = controller.borrow();
                (
                    library.library().settings.auto_hide_on_launch,
                    machine.borrow().is_pinned(),
                )
            };
            // P3-08 (baseline 8.2): CMD/PowerShell runs display the actual
            // execution content in the status bar.
            let status = if item.item_type == quicknest::library_model::ItemKind::Command {
                format!(
                    "已启动 {name}：{}",
                    quicknest::infrastructure::windows::launch::command_display(&item)
                )
            } else {
                format!("已启动 {name}")
            };
            ui.set_validation_status(status.into());
            if auto_hide && !pinned {
                hide_panel(ui, machine);
            } else {
                // Pinned or auto-hide-on-launch disabled: keep the panel and
                // re-assert focus after the launched process took it.
                show_and_focus(ui);
            }
        }
        LaunchOutcome::Failed(error) => {
            ui.set_validation_status(error.message().into());
        }
    }
}

#[cfg(not(windows))]
fn launch_item_by_id(
    ui: &MainWindow,
    _controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
    _machine: &SharedFocusMachine,
    _item_id: &str,
    _as_admin: bool,
) {
    ui.set_validation_status("启动功能需要 Windows".into());
}

/// Fills the editor's metadata fields from a picked shortcut (baseline 4.1:
/// IShellLinkW/IPersistFile GetArguments/GetWorkingDirectory/GetIconLocation).
#[cfg(windows)]
fn fill_lnk_metadata(ui: &MainWindow, path: &std::path::Path) {
    use quicknest::infrastructure::windows::shell::{detect_advertised_shortcut, resolve_link_now};
    // P2-09 (baseline 4.3): an MSI advertised shortcut is only a Darwin
    // reference into a Windows Installer product. Its metadata must not be
    // resolved/replaced; the item keeps the original LNK and the Shell (MSI)
    // owns installation and repair.
    if detect_advertised_shortcut(path) {
        ui.set_validation_status("已识别 MSI 广告快捷方式（由 Windows Installer 启动）".into());
        return;
    }
    // P2-07 (baseline 4.2): a UWP shortcut keeps its original LNK and reports
    // the AUMID read from its target AppsFolder item (never guessed).
    if let Some(uwp) = quicknest::infrastructure::windows::shell::detect_uwp_link(path) {
        if uwp.aumid.is_empty() {
            ui.set_validation_status("已识别 UWP 应用（未取得 AUMID，保留原始快捷方式）".into());
        } else {
            ui.set_validation_status(format!("已识别 UWP 应用（AUMID：{}）", uwp.aumid).into());
        }
        return;
    }
    let Some(link) = resolve_link_now(path) else {
        return;
    };
    if ui.get_item_dialog_args().is_empty() {
        ui.set_item_dialog_args(link.arguments.into());
    }
    if ui.get_item_dialog_workdir().is_empty() {
        ui.set_item_dialog_workdir(link.working_directory.into());
    }
    if ui.get_item_dialog_icon().is_empty() {
        ui.set_item_dialog_icon(link.icon_location.into());
    }
}

#[cfg(not(windows))]
fn fill_lnk_metadata(_ui: &MainWindow, _path: &std::path::Path) {}

/// Fills empty metadata fields of an LNK draft when it is saved (baseline 4.1:
/// importing a shortcut reads its arguments/working directory/icon).
#[cfg(windows)]
fn fill_lnk_draft(draft: &mut quicknest::library_model::ItemDraft) {
    use quicknest::infrastructure::windows::shell::{detect_advertised_shortcut, resolve_link_now};
    draft.msi_advertised = false;
    if draft.item_type != quicknest::library_model::ItemKind::Lnk {
        return;
    }
    let lnk_path = std::path::Path::new(draft.path.trim());
    // P2-09 (baseline 4.3): remember that the LNK is an MSI advertised
    // shortcut. The original LNK stays the launch target; no target EXE is
    // resolved or stored, and the shortcut is never copied or rewritten.
    if detect_advertised_shortcut(lnk_path) {
        draft.msi_advertised = true;
        return;
    }
    // P2-07 (baseline 4.2.3): save the AUMID when the shortcut is a UWP app
    // and the target item carries one; the item keeps the original LNK path.
    if let Some(uwp) = quicknest::infrastructure::windows::shell::detect_uwp_link(lnk_path) {
        if draft.aumid.trim().is_empty() {
            draft.aumid = uwp.aumid;
        }
        return;
    }
    let Some(link) = resolve_link_now(lnk_path) else {
        return;
    };
    if draft.args.trim().is_empty() {
        draft.args = link.arguments;
    }
    if draft.workdir.trim().is_empty() {
        draft.workdir = link.working_directory;
    }
    if draft.icon.trim().is_empty() {
        draft.icon = link.icon_location;
    }
}

#[cfg(not(windows))]
fn fill_lnk_draft(_draft: &mut quicknest::library_model::ItemDraft) {}

/// P2-10 (baseline 4.4): recognizes typed Shell namespace, URI and UNC
/// targets, maps them onto the existing seven item kinds, and fills an empty
/// name from the Shell namespace display name. The original target string is
/// never rewritten.
#[cfg(windows)]
fn fill_shell_draft(draft: &mut quicknest::library_model::ItemDraft) {
    use quicknest::library_model::{
        ItemKind, is_shell_namespace_target, is_unc_path, is_uri_target,
    };
    let path = draft.path.trim();
    if path.is_empty()
        || (!is_shell_namespace_target(path) && !is_uri_target(path) && !is_unc_path(path))
    {
        return;
    }
    // The editor starts at Exe; a typed Shell/URI/UNC target keeps the
    // existing seven-kind model but gets its real kind (Uri or Folder).
    if matches!(draft.item_type, ItemKind::Exe | ItemKind::File) {
        draft.item_type = ItemKind::from_path(path);
    }
    if draft.name.trim().is_empty()
        && is_shell_namespace_target(path)
        && let Some(name) = quicknest::infrastructure::windows::shell::shell_display_name(
            std::path::Path::new(path),
        )
    {
        draft.name = name;
    }
}

#[cfg(not(windows))]
fn fill_shell_draft(_draft: &mut quicknest::library_model::ItemDraft) {}

/// Opens the containing folder with the item selected (baseline 3.5).
#[cfg(windows)]
fn open_item_location_by_id(
    ui: &MainWindow,
    controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
    item_id: &str,
) {
    use quicknest::infrastructure::windows::launch::{LaunchOutcome, open_item_location};
    let Some(item) = controller.borrow().item(item_id) else {
        ui.set_validation_status("项目不存在".into());
        return;
    };
    match open_item_location(&item) {
        LaunchOutcome::Started { .. } => ui.set_validation_status("已打开文件所在位置".into()),
        LaunchOutcome::Failed(error) => ui.set_validation_status(error.message().into()),
    }
}

#[cfg(not(windows))]
fn open_item_location_by_id(
    ui: &MainWindow,
    _controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
    _item_id: &str,
) {
    ui.set_validation_status("定位功能需要 Windows".into());
}

/// Copies a project target to the clipboard (baseline 3.5 "复制路径").
#[cfg(windows)]
fn copy_path_to_clipboard(text: &str) -> bool {
    quicknest::infrastructure::windows::dialogs::copy_text(text)
}

#[cfg(not(windows))]
fn copy_path_to_clipboard(_text: &str) -> bool {
    false
}

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<String> = std::env::args().collect();
    #[cfg(windows)]
    if let Some(result) = quicknest::updater::worker_entry(&arguments) {
        return result.map_err(Into::into);
    }
    let mode = parse_args(arguments.into_iter().skip(1))?;
    if mode == RunMode::Help {
        println!("{USAGE}");
        return Ok(());
    }

    if let RunMode::TransferTest(report) = &mode {
        #[cfg(windows)]
        return transfer_validation::run(report);
        #[cfg(not(windows))]
        return Err(format!("transfer validation requires Windows: {}", report.display()).into());
    }

    if let RunMode::OleTest(report) = &mode {
        #[cfg(windows)]
        return ole_validation::run(report);
        #[cfg(not(windows))]
        return Err(format!("OLE validation requires Windows: {}", report.display()).into());
    }

    #[cfg(windows)]
    {
        if let RunMode::MutexChild(report) = &mode {
            return mutex_validation::run_child(report);
        }
        if let RunMode::MutexKeeper { name, report } = &mode {
            return mutex_validation::run_keeper(name, report);
        }
        if let RunMode::IpcClient { report, command } = &mode {
            return ipc_validation::run_client(report, command);
        }
        if let RunMode::IpcRaw { report, kind } = &mode {
            return ipc_validation::run_raw_client(report, kind);
        }
        if let RunMode::HotkeyConflict(report) = &mode {
            return hotkey_validation::run_conflict(report);
        }
    }

    // P1-16: single instance + IPC wake-up (baseline 9.1-9.5). This runs
    // before OLE/window creation so a second launch exits immediately; the
    // guard lives until main returns (Mutex released through Drop).
    #[cfg(windows)]
    let mut _instance_guard: Option<
        quicknest::infrastructure::windows::single_instance::MutexGuard,
    > = None;
    #[cfg(windows)]
    let mut _ipc_server_keepalive: Option<
        Rc<quicknest::infrastructure::windows::ipc::PipeServer>,
    > = None;
    let mut _ipc_timer_keepalive: Option<Timer> = None;
    let mut _delayed_timer_keepalive: Option<Timer> = None;
    #[cfg(windows)]
    if mode == RunMode::Interactive {
        use quicknest::infrastructure::windows::ipc::{
            IpcCommand, IpcMessage, send_message, start_server,
        };
        use quicknest::infrastructure::windows::single_instance::{
            AcquireOutcome, InstanceIdentity, acquire,
        };
        let wake_main_instance = || {
            InstanceIdentity::current()
                .map(|identity| {
                    send_message(
                        &identity.pipe_name(),
                        &IpcMessage::new(IpcCommand::ShowAndFocus),
                    )
                })
                .is_ok_and(|result| matches!(result, Ok(true)))
        };
        match acquire() {
            Ok((AcquireOutcome::AlreadyRunning, _probe)) => {
                // Wake the live instance; a confirmed ack means this launch has
                // nothing left to do.
                if wake_main_instance() {
                    return Ok(());
                }
                // Baseline 9.5: the pipe may lag behind the mutex, or the owner
                // may have died. Recheck ownership before giving up; only a
                // successful re-acquire turns this process into the main one.
                eprintln!(
                    "QuickNest is running but did not acknowledge the wake-up; rechecking ownership"
                );
                match acquire() {
                    Ok((
                        AcquireOutcome::FirstInstance | AcquireOutcome::TookOverAbandoned,
                        guard,
                    )) => {
                        _instance_guard = Some(guard);
                    }
                    Ok((AcquireOutcome::AlreadyRunning, _probe)) => {
                        notify_single_instance_failure(
                            "QuickNest 已在运行，但无法唤醒主窗口。请稍后重试或结束现有 QuickNest 进程。",
                        );
                        return Ok(());
                    }
                    Ok((AcquireOutcome::Failed(error), _guard)) => {
                        eprintln!("single-instance recheck failed: {error}");
                        notify_single_instance_failure(
                            "QuickNest 无法确认单实例状态，已安全退出。",
                        );
                        return Ok(());
                    }
                    Err(error) => {
                        eprintln!("single-instance recheck mutex failed: {error}");
                        notify_single_instance_failure(
                            "QuickNest 无法确认单实例状态，已安全退出。",
                        );
                        return Ok(());
                    }
                }
            }
            Ok((AcquireOutcome::FirstInstance | AcquireOutcome::TookOverAbandoned, guard)) => {
                _instance_guard = Some(guard);
            }
            Ok((AcquireOutcome::Failed(error), _guard)) => {
                // Baseline 9.2: WAIT_FAILED is recorded and the process exits
                // safely instead of risking a second main instance.
                eprintln!("single-instance check failed: {error}");
                return Ok(());
            }
            Err(error) => {
                eprintln!("single-instance mutex failed: {error}");
                return Ok(());
            }
        }
        if _instance_guard.is_some() {
            match start_server() {
                Ok(server) => _ipc_server_keepalive = Some(Rc::new(server)),
                Err(error) => eprintln!("IPC server failed: {error:?}"),
            }
        }
    }

    #[cfg(windows)]
    let _ole = quicknest::infrastructure::windows::ole::OleGuard::initialize()?;
    select_backend()?;

    if let RunMode::HwndTest(report) = &mode {
        #[cfg(windows)]
        return hwnd_validation::run(report);
        #[cfg(not(windows))]
        return Err(format!("HWND validation requires Windows: {}", report.display()).into());
    }

    if let RunMode::DropTargetTest(report) = &mode {
        #[cfg(windows)]
        return drop_target_validation::run(report);
        #[cfg(not(windows))]
        return Err(format!(
            "Drop target validation requires Windows: {}",
            report.display()
        )
        .into());
    }

    if let RunMode::FileDropTest(report) = &mode {
        #[cfg(windows)]
        return file_drop_validation::run(report);
        #[cfg(not(windows))]
        return Err(format!(
            "File-drop validation requires Windows: {}",
            report.display()
        )
        .into());
    }

    if let RunMode::DpiScaleTest(report) = &mode {
        #[cfg(windows)]
        return dpi_scale_validation::run(report);
        #[cfg(not(windows))]
        return Err(format!(
            "DPI scale validation requires Windows: {}",
            report.display()
        )
        .into());
    }

    if let RunMode::DpiTest(report) = &mode {
        #[cfg(windows)]
        return file_drop_validation::run_dpi(report);
        #[cfg(not(windows))]
        return Err(format!("DPI validation requires Windows: {}", report.display()).into());
    }

    if let RunMode::ShellTest(report) = &mode {
        #[cfg(windows)]
        return shell_validation::run(report);
        #[cfg(not(windows))]
        return Err(format!("Shell validation requires Windows: {}", report.display()).into());
    }

    if let RunMode::MutexTest(report) = &mode {
        #[cfg(windows)]
        return mutex_validation::run(report);
        #[cfg(not(windows))]
        return Err(format!("Mutex validation requires Windows: {}", report.display()).into());
    }

    if let RunMode::IpcTest(report) = &mode {
        #[cfg(windows)]
        return ipc_validation::run(report);
        #[cfg(not(windows))]
        return Err(format!("IPC validation requires Windows: {}", report.display()).into());
    }

    if let RunMode::HotkeyTest(report) = &mode {
        #[cfg(windows)]
        return hotkey_validation::run(report);
        #[cfg(not(windows))]
        return Err(format!("Hotkey validation requires Windows: {}", report.display()).into());
    }

    if let RunMode::TrayTest(report) = &mode {
        #[cfg(windows)]
        return tray_validation::run(report);
        #[cfg(not(windows))]
        return Err(format!("Tray validation requires Windows: {}", report.display()).into());
    }

    if let RunMode::FocusTest(report) = &mode {
        #[cfg(windows)]
        return focus_validation::run(report);
        #[cfg(not(windows))]
        return Err(format!("Focus validation requires Windows: {}", report.display()).into());
    }

    if mode == RunMode::CrashTest {
        // P4-07: force a real panic so the hook records it; the non-zero exit
        // proves the process aborted after the log was written.
        quicknest::crash_log::install();
        panic!("QuickNest crash-test panic (P4-07)");
    }

    if let RunMode::UpdateTest { report, url } = &mode {
        // P4-04/P4-05/P4-06: the update chain against a local test server.
        return updater_validation::run(report, url, env!("CARGO_PKG_VERSION")).map_err(Into::into);
    }

    if let RunMode::SaveTest(report) = &mode {
        // No window or OLE required: the save chain is pure filesystem work.
        #[cfg(windows)]
        return save_validation::run(report);
        #[cfg(not(windows))]
        return Err(format!("Save validation requires Windows: {}", report.display()).into());
    }

    // P4-07: the panic hook is installed before any window exists so even an
    // early panic lands in the crash log.
    quicknest::crash_log::install();

    let ui = create_window()?;

    let data_directory = quicknest::portable::data_directory()?;
    std::fs::create_dir_all(&data_directory)?;
    let config_path = Some(data_directory.join("config.json"));
    let controller = Rc::new(RefCell::new(
        quicknest::library_controller::LibraryController::load(config_path),
    ));
    // P1-18: focus/auto-hide state machine (baseline chapter 10). Pinned is a
    // persisted setting mirrored into the machine's suppression set.
    let focus_machine: SharedFocusMachine = Rc::new(RefCell::new(
        quicknest::window_state::FocusStateMachine::new(),
    ));
    let dialog_suppression: Rc<RefCell<Option<quicknest::window_state::SuppressionToken>>> =
        Rc::new(RefCell::new(None));
    let tray_menu_suppression: Rc<RefCell<Option<quicknest::window_state::SuppressionToken>>> =
        Rc::new(RefCell::new(None));
    {
        let pinned = controller.borrow().library().settings.pinned;
        focus_machine.borrow_mut().set_pinned(pinned);
        ui.set_pin_toggled(pinned);
    }
    // P2-12: apply the persisted theme before the first paint.
    apply_theme(&ui, &controller);
    // Follow the Windows app theme while `settings.theme == "system"`. The
    // registry read is cheap; 2 s keeps the switch responsive without a
    // WM_SETTINGCHANGE listener on the winit message loop.
    let _theme_timer_keepalive = {
        let weak = ui.as_weak();
        let controller = Rc::clone(&controller);
        let system_dark = Cell::new(quicknest::theme::system_theme_is_dark());
        let timer = Timer::default();
        timer.start(
            TimerMode::Repeated,
            Duration::from_millis(2000),
            move || {
                let mode = quicknest::theme::ThemeMode::parse(
                    &controller.borrow().library().settings.theme,
                );
                if mode == quicknest::theme::ThemeMode::System {
                    let now = quicknest::theme::system_theme_is_dark();
                    if now != system_dark.get() {
                        system_dark.set(now);
                        if let Some(ui) = weak.upgrade() {
                            apply_theme(&ui, &controller);
                        }
                    }
                }
            },
        );
        timer
    };
    // P1-10: background icon extraction (worker thread + UI-side cache).
    // P1-11: the cache directory sits next to the config; entries not
    // referenced by the current library are pruned on startup.
    let icons_dir = Some(data_directory.join("icons"));
    install_icon_context(Rc::new(IconContext::new(
        icons_dir.clone().unwrap_or_default(),
    )));
    if let Some(directory) = &icons_dir {
        prune_icon_cache(directory, &controller);
    }
    // P1-12: surface recovery / safe-mode results (baseline 15.8).
    match controller.borrow().load_state() {
        quicknest::library_controller::LoadState::Recovered { backup, corrupted } => {
            let mut message = format!("已从备份恢复配置：{}", backup.display());
            if let Some(corrupted) = corrupted {
                message.push_str(&format!("；损坏文件保留为 {}", corrupted.display()));
            }
            ui.set_validation_status(message.into());
        }
        quicknest::library_controller::LoadState::SafeMode { reason } => {
            ui.set_validation_status(
                format!("配置不可用，已进入安全模式（不会覆盖原文件）：{reason}").into(),
            );
        }
        quicknest::library_controller::LoadState::Loaded => {}
    }
    // P1-14: the global hotkey guard and its event-polling timer must live for
    // the whole session (baseline 11.1).
    #[cfg(windows)]
    let mut _hotkey_guard_keepalive: Option<
        quicknest::infrastructure::windows::hotkey::HotkeyGuard,
    > = None;
    let mut _hotkey_timer_keepalive: Option<Timer> = None;
    let hotkey_spec = controller.borrow().library().settings.hotkey.clone();
    let mut hotkey_message: Option<String> = None;
    #[cfg(windows)]
    if let Some((modifiers, virtual_key)) =
        quicknest::infrastructure::windows::hotkey::parse_hotkey(&hotkey_spec)
    {
        match quicknest::infrastructure::windows::hotkey::register("toggle", modifiers, virtual_key)
        {
            Ok(mut guard) => {
                if let Some(events) = guard.take_events() {
                    let weak = ui.as_weak();
                    let machine = Rc::clone(&focus_machine);
                    let timer = Timer::default();
                    timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
                        while events.try_recv().is_ok() {
                            if let Some(ui) = weak.upgrade() {
                                show_panel(&ui, &machine);
                            }
                        }
                    });
                    _hotkey_timer_keepalive = Some(timer);
                }
                _hotkey_guard_keepalive = Some(guard);
            }
            Err(error) => {
                hotkey_message = Some(format!(
                    "快捷键 {hotkey_spec} 注册失败：{}",
                    quicknest::infrastructure::windows::hotkey::error_name(error)
                ));
            }
        }
    } else {
        hotkey_message = Some(format!("快捷键配置无法解析：{hotkey_spec}"));
    }
    #[cfg(not(windows))]
    let _ = &hotkey_spec;
    if let Some(message) = hotkey_message {
        ui.set_validation_status(message.into());
    }
    // P1-15: tray icon (baseline 11.3). The service and its polling timer live
    // for the session; Drop removes the icon and joins the tray thread.
    #[cfg(windows)]
    let mut _tray_service_keepalive: Option<
        quicknest::infrastructure::windows::tray::TrayService,
    > = None;
    let mut _tray_timer_keepalive: Option<Timer> = None;
    #[cfg(windows)]
    match quicknest::infrastructure::windows::tray::register("main") {
        Ok(mut service) => {
            if let Some(events) = service.take_events() {
                let weak = ui.as_weak();
                let controller = Rc::clone(&controller);
                let machine = Rc::clone(&focus_machine);
                let menu_suppression = Rc::clone(&tray_menu_suppression);
                let timer = Timer::default();
                timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
                    while let Ok(event) = events.try_recv() {
                        handle_tray_event(&weak, &controller, &machine, &menu_suppression, event);
                    }
                });
                _tray_timer_keepalive = Some(timer);
            }
            _tray_service_keepalive = Some(service);
        }
        Err(error) => {
            ui.set_validation_status(format!("托盘注册失败：{error}").into());
        }
    }
    // P1-16: drain IPC commands from secondary launches (baseline 9.4).
    if let Some(server) = &_ipc_server_keepalive {
        let server = Rc::clone(server);
        let weak = ui.as_weak();
        let machine = Rc::clone(&focus_machine);
        let timer = Timer::default();
        timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
            for command in server.drain() {
                if let Some(ui) = weak.upgrade() {
                    handle_ipc_command(&ui, &machine, command);
                }
            }
        });
        _ipc_timer_keepalive = Some(timer);
    }
    let mut _icon_timer_keepalive: Option<Timer> = None;
    // P1-17: the OLE drop registration and its polling timer live for the
    // whole session; Drop revokes the target before the window goes away.
    let mut _drop_timer_keepalive: Option<Timer> = None;
    // P2-06: spring-load timer that switches the target group after 400 ms of
    // hovering during an internal drag.
    let mut _spring_timer_keepalive: Option<Timer> = None;
    #[cfg(windows)]
    let mut _drop_registration_keepalive: Option<
        Rc<RefCell<Option<quicknest::infrastructure::windows::drop_target::DropRegistration>>>,
    > = None;
    {
        // Refresh pushes the model into the UI; every callback owns its own
        // Rc clone and calls this after mutating the model.
        // refresh_ui 已提升为模块级函数（P4 审核：launch_item_by_id 也需要调用）。

        /// P2-06: updates only the sidebar (groups/categories/active
        /// category) without rebuilding the grid, so a card mid-drag keeps
        /// its pointer capture while the spring-load switches groups.
        fn refresh_sidebar(
            controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
            weak: &slint::Weak<MainWindow>,
        ) {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let views: Vec<GroupView> = controller
                .borrow()
                .group_rows()
                .into_iter()
                .map(|row| GroupView {
                    id: row.id.clone().into(),
                    name: row.name.into(),
                    accent: row.color.into(),
                    active: row.active,
                    item_count: row.item_count as i32,
                    tab_width: row.tab_width,
                })
                .collect();
            let categories: Vec<CategoryView> = controller
                .borrow()
                .category_rows()
                .into_iter()
                .map(|row| CategoryView {
                    id: row.id.clone().into(),
                    name: row.name.into(),
                    item_count: row.item_count as i32,
                })
                .collect();
            ui.set_groups(ModelRc::new(VecModel::from(views)));
            ui.set_categories(ModelRc::new(VecModel::from(categories)));
            ui.set_active_category_id(
                controller
                    .borrow()
                    .library()
                    .active_category()
                    .map(|category| category.id.clone())
                    .unwrap_or_default()
                    .into(),
            );
        }

        // P2-01: search center helpers (baseline 2.6/13). Linear scan over the
        // registered library; pinyin matching is P2-02.
        let search_all_groups = Rc::new(Cell::new(false));
        let search_query_state = Rc::new(RefCell::new(String::new()));
        // P2-06: the group tab an internal drag hovers over, with the instant
        // the hover started (spring-load switches after 400 ms).
        let drag_group_hover: Rc<RefCell<Option<(String, Instant)>>> = Rc::new(RefCell::new(None));
        {
            let hover = Rc::clone(&drag_group_hover);
            ui.on_item_drag_cancelled(move || {
                hover.borrow_mut().take();
            });
        }
        {
            let controller = Rc::clone(&controller);
            let weak = ui.as_weak();
            let hover = Rc::clone(&drag_group_hover);
            ui.on_item_drag_finished(move || {
                hover.borrow_mut().take();
                refresh_ui(&controller, &weak);
            });
        }

        fn refresh_search(
            ui: &MainWindow,
            controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
            all_groups: bool,
            query: &str,
        ) {
            let mut rows: Vec<SearchRow> = controller
                .borrow()
                .search_items(query, all_groups)
                .into_iter()
                .map(|row| SearchRow {
                    id: row.id.into(),
                    name: row.name.into(),
                    path: row.path.into(),
                    scope: row.scope.into(),
                    type_label: row.type_label.into(),
                })
                .collect();
            // P3-09 (baseline 22 Everything 可选集成): when enabled and a
            // query is present, offer to hand the search to Everything.
            let everything_enabled = controller.borrow().library().settings.everything_enabled;
            #[cfg(windows)]
            if everything_enabled && !query.trim().is_empty() {
                use quicknest::infrastructure::windows::launch::{
                    everything_fallback_paths, resolve_everything_path,
                };
                let configured = controller
                    .borrow()
                    .library()
                    .settings
                    .everything_path
                    .clone();
                let path = resolve_everything_path(&configured, &everything_fallback_paths());
                rows.push(SearchRow {
                    id: format!("everything:{query}").into(),
                    name: format!("在 Everything 中搜索“{query}”").into(),
                    path: match path {
                        Some(path) => path.into(),
                        None => "未找到 Everything".into(),
                    },
                    scope: "集成".into(),
                    type_label: "Everything".into(),
                });
            }
            #[cfg(not(windows))]
            let _ = everything_enabled;
            let count = rows.len();
            ui.set_search_results(ModelRc::new(VecModel::from(rows)));
            // P2-03: default selection is the first match (baseline 13.4).
            ui.set_search_selected_index(if count > 0 { 0 } else { -1 });
            ui.set_search_scope_label(if all_groups {
                "范围：全部分组".into()
            } else {
                "范围：当前分组".into()
            });
        }

        fn open_search_center(
            ui: &MainWindow,
            controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
            all_groups: &Cell<bool>,
            query_state: &RefCell<String>,
        ) {
            all_groups.set(false);
            query_state.borrow_mut().clear();
            ui.set_search_query("".into());
            ui.set_search_visible(true);
            refresh_search(ui, controller, false, "");
        }

        /// P2-03: the item id under the keyboard selection, if any.
        fn selected_search_id(ui: &MainWindow) -> Option<String> {
            let index = ui.get_search_selected_index();
            if index < 0 {
                return None;
            }
            ui.get_search_results()
                .row_data(index as usize)
                .map(|row| row.id.to_string())
        }

        // P1-10: join background icon results on the UI thread.
        // P4 审核修复：延时启动结果的 UI 排空定时器——成功才记 usage 并刷新
        // 排序，失败提示用户。
        let delayed_drain_timer = Timer::default();
        {
            let controller = Rc::clone(&controller);
            let weak = ui.as_weak();
            delayed_drain_timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
                let drained: Vec<DelayedLaunchOutcome> = delayed_tasks().drain();
                for outcome in drained {
                    match outcome {
                        DelayedLaunchOutcome::Started { item_id } => {
                            controller.borrow_mut().record_launch(&item_id);
                            refresh_ui(&controller, &weak);
                        }
                        DelayedLaunchOutcome::Failed { item_id, reason } => {
                            let name = controller
                                .borrow()
                                .item(&item_id)
                                .map(|item| item.name)
                                .unwrap_or_default();
                            if let Some(ui) = weak.upgrade() {
                                ui.set_validation_status(
                                    format!("延时启动失败：{name}（{reason}）").into(),
                                );
                            }
                        }
                    }
                }
            });
            _delayed_timer_keepalive = Some(delayed_drain_timer);
        }
        let icon_drain_timer = Timer::default();
        {
            let controller = Rc::clone(&controller);
            let weak = ui.as_weak();
            icon_drain_timer.start(TimerMode::Repeated, Duration::from_millis(150), move || {
                if drain_icon_results() {
                    refresh_ui(&controller, &weak);
                }
            });
        }
        _icon_timer_keepalive = Some(icon_drain_timer);

        let weak = ui.as_weak();
        refresh_ui(&controller, &weak);
        let weak = ui.as_weak();
        refresh_ui(&controller, &weak);

        // P1-05 helpers shared by the menu and dialog callbacks.
        fn draft_from_ui(ui: &MainWindow) -> quicknest::library_model::ItemDraft {
            use quicknest::library_model::{ItemDraft, ItemKind, WindowState};
            ItemDraft {
                name: ui.get_item_dialog_name().to_string(),
                item_type: ItemKind::from_index(ui.get_item_dialog_type_index().max(0) as usize),
                path: ui.get_item_dialog_target().to_string(),
                args: ui.get_item_dialog_args().to_string(),
                workdir: ui.get_item_dialog_workdir().to_string(),
                icon: ui.get_item_dialog_icon().to_string(),
                window_state: match ui.get_item_dialog_window_index() {
                    1 => WindowState::Minimized,
                    2 => WindowState::Maximized,
                    _ => WindowState::Normal,
                },
                run_as_admin: ui.get_item_dialog_run_as_admin(),
                note: ui.get_item_dialog_note().to_string(),
                aumid: String::new(),
                msi_advertised: false,
                // Editing preserves the stored mark; new items start unverified.
                confirm_on_first_run: true,
                delay_seconds: 0,
                // P3-08 (baseline 8.2): the editor's execution mode picker.
                execution_mode: match ui.get_item_dialog_exec_index().max(0) {
                    1 => "cmd".to_string(),
                    2 => "powershell".to_string(),
                    _ => "direct".to_string(),
                },
                last_used: 0,
                launch_count: 0,
                favorite: false,
            }
        }

        fn open_item_editor(
            ui: &MainWindow,
            controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
            editing: &Rc<RefCell<Option<String>>>,
            item_id: Option<String>,
        ) {
            use quicknest::library_model::{ItemDraft, WindowState};
            let draft = item_id
                .as_deref()
                .and_then(|id| controller.borrow().item_draft(id))
                .unwrap_or_else(ItemDraft::empty);
            ui.set_item_dialog_title(
                if item_id.is_some() {
                    "编辑项目"
                } else {
                    "添加项目"
                }
                .into(),
            );
            ui.set_item_dialog_name(draft.name.into());
            ui.set_item_dialog_target(draft.path.into());
            ui.set_item_dialog_args(draft.args.into());
            ui.set_item_dialog_workdir(draft.workdir.into());
            ui.set_item_dialog_icon(draft.icon.into());
            ui.set_item_dialog_note(draft.note.into());
            ui.set_item_dialog_type_index(draft.item_type.index() as i32);
            // P3-08 (baseline 8.2): the execution mode picker for commands.
            ui.set_item_dialog_exec_index(
                match quicknest::library_model::normalize_execution_mode(&draft.execution_mode) {
                    "cmd" => 1,
                    "powershell" => 2,
                    _ => 0,
                },
            );
            ui.set_item_dialog_window_index(match draft.window_state {
                WindowState::Minimized => 1,
                WindowState::Maximized => 2,
                _ => 0,
            });
            ui.set_item_dialog_run_as_admin(draft.run_as_admin);
            ui.set_item_dialog_error("".into());
            *editing.borrow_mut() = item_id;
            ui.set_item_dialog_visible(true);
        }

        let editing_item: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let pending_item_delete: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        // P3-08 (baseline 8.3): the command item awaiting its first-run
        // confirmation, plus whether the run was requested as administrator.
        let pending_command_confirm: Rc<RefCell<Option<(String, bool)>>> =
            Rc::new(RefCell::new(None));
        let pending_item_move: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            ui.on_update_check_requested(move || {
                if let Some(ui) = weak.upgrade() {
                    start_update_check(&ui, &controller, true);
                }
            });
        }
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            ui.on_update_check_toggle_requested(move || {
                let enabled = !controller.borrow().update_check_enabled();
                controller.borrow_mut().set_update_check_enabled(enabled);
                let enabled = controller.borrow().update_check_enabled();
                if !enabled {
                    UPDATE_UI.with(|state| {
                        state
                            .borrow()
                            .cancel
                            .store(true, std::sync::atomic::Ordering::SeqCst)
                    });
                    UPDATE_UI.with(|state| state.borrow_mut().busy = false);
                }
                if let Some(ui) = weak.upgrade() {
                    ui.set_update_check_busy(UPDATE_UI.with(|state| state.borrow().busy));
                    refresh_ui(&controller, &weak);
                }
            });
        }
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            ui.on_update_install_confirmed(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let update = UPDATE_UI.with(|state| state.borrow_mut().pending.take());
                let Some(update) = update else {
                    ui.set_update_confirm_status("没有已验证的更新，请重新检查".into());
                    return;
                };
                if let Err(error) = controller.borrow().persist() {
                    ui.set_update_confirm_status(
                        format!("配置保存失败，未启动更新：{error}").into(),
                    );
                    UPDATE_UI.with(|state| state.borrow_mut().pending = Some(update));
                    return;
                }
                let target = match std::env::current_exe() {
                    Ok(target) => target,
                    Err(error) => {
                        ui.set_update_confirm_status(error.to_string().into());
                        return;
                    }
                };
                match update.spawn_worker(&target) {
                    Ok(_worker) => {
                        ui.set_update_confirm_visible(false);
                        delayed_tasks().cancel_pending();
                        let _ = slint::quit_event_loop();
                    }
                    Err(error) => {
                        ui.set_update_confirm_status(format!("更新未开始：{error}").into())
                    }
                }
            });
        }
        {
            let weak = ui.as_weak();
            ui.on_update_install_cancelled(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.set_update_confirm_visible(false);
                }
            });
        }
        // UI 体验修复（2026-11-07 第三批）：应用内设置面板的打开/关闭。
        // 打开前 refresh 一次，保证面板选项读到当前快照。
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            ui.on_settings_open_requested(move || {
                refresh_ui(&controller, &weak);
                if let Some(ui) = weak.upgrade() {
                    ui.set_settings_panel_visible(true);
                }
            });
        }
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            ui.on_settings_panel_closed(move || {
                refresh_ui(&controller, &weak);
            });
        }

        {
            let weak = ui.as_weak();
            let machine = Rc::clone(&focus_machine);
            let pending = Rc::clone(&pending_command_confirm);
            let controller = Rc::clone(&controller);
            ui.on_escape_pressed(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                if ui.get_settings_panel_visible() {
                    ui.set_settings_panel_visible(false);
                    refresh_ui(&controller, &weak);
                } else if ui.get_command_confirm_visible() {
                    pending.borrow_mut().take();
                    ui.set_command_confirm_visible(false);
                } else if ui.get_update_confirm_visible() {
                    ui.set_update_confirm_visible(false);
                } else if ui.get_combination_name_dialog_visible() {
                    ui.set_combination_name_dialog_visible(false);
                } else if ui.get_combination_dialog_visible() {
                    ui.set_combination_dialog_visible(false);
                } else if ui.get_transfer_dialog_visible() {
                    ui.set_transfer_dialog_visible(false);
                } else if ui.get_backup_dialog_visible() {
                    ui.set_backup_dialog_visible(false);
                } else if ui.get_item_dialog_visible() {
                    ui.set_item_dialog_visible(false);
                } else if ui.get_item_delete_confirm_visible() {
                    ui.set_item_delete_confirm_visible(false);
                } else if ui.get_move_dialog_visible() {
                    ui.set_move_dialog_visible(false);
                } else if ui.get_group_dialog_visible() {
                    ui.set_group_dialog_visible(false);
                } else if ui.get_group_delete_confirm_visible() {
                    ui.set_group_delete_confirm_visible(false);
                } else if ui.get_category_dialog_visible() {
                    ui.set_category_dialog_visible(false);
                } else if ui.get_category_delete_confirm_visible() {
                    ui.set_category_delete_confirm_visible(false);
                } else {
                    hide_panel(&ui, &machine);
                }
            });
        }
        if mode == RunMode::Interactive && controller.borrow().update_check_enabled() {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            slint::Timer::single_shot(Duration::from_secs(3), move || {
                if controller.borrow().update_check_enabled()
                    && let Some(ui) = weak.upgrade()
                {
                    start_update_check(&ui, &controller, false);
                }
            });
        }

        {
            // P1-13: manual backup entry in the status bar.
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            ui.on_backup_requested(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                match controller.borrow().backup_now() {
                    Ok(path) => {
                        ui.set_validation_status(format!("已备份：{}", path.display()).into());
                    }
                    Err(error) => {
                        ui.set_validation_status(format!("备份失败：{error}").into());
                    }
                }
                refresh_ui(&controller, &weak);
            });
        }

        {
            // P2-01: Ctrl+F / title-bar search opens the search center.
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            let all_groups = Rc::clone(&search_all_groups);
            let query_state = Rc::clone(&search_query_state);
            ui.on_search_requested(move || {
                if let Some(ui) = weak.upgrade() {
                    open_search_center(&ui, &controller, &all_groups, &query_state);
                }
            });
        }
        {
            // P2-01: real-time filtering as the user types.
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            let all_groups = Rc::clone(&search_all_groups);
            let query_state = Rc::clone(&search_query_state);
            ui.on_search_query_changed(move |query: slint::SharedString| {
                if let Some(ui) = weak.upgrade() {
                    *query_state.borrow_mut() = query.to_string();
                    refresh_search(&ui, &controller, all_groups.get(), query.as_str());
                }
            });
        }
        {
            // P2-01: toggle between the active group and the whole library.
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            let all_groups = Rc::clone(&search_all_groups);
            let query_state = Rc::clone(&search_query_state);
            ui.on_search_scope_toggled(move || {
                if let Some(ui) = weak.upgrade() {
                    let next = !all_groups.get();
                    all_groups.set(next);
                    let query = query_state.borrow().clone();
                    refresh_search(&ui, &controller, next, &query);
                }
            });
        }
        {
            // P2-01: clicking a result launches it and closes the panel.
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            let machine = Rc::clone(&focus_machine);
            let pending = Rc::clone(&pending_command_confirm);
            ui.on_search_result_activated(move |item_id: slint::SharedString| {
                if let Some(ui) = weak.upgrade() {
                    ui.set_search_visible(false);
                    // P3-09: the Everything row is not a library item.
                    if let Some(query) = item_id.strip_prefix("everything:") {
                        launch_everything_handoff(&ui, &controller, query);
                        return;
                    }
                    launch_item_with_confirm(
                        &ui,
                        &controller,
                        &machine,
                        &pending,
                        item_id.as_ref(),
                        false,
                    );
                }
            });
        }
        {
            // P2-01/baseline 13.4: Esc clears the query first, then closes.
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            let all_groups = Rc::clone(&search_all_groups);
            let query_state = Rc::clone(&search_query_state);
            ui.on_search_dismissed(move || {
                if let Some(ui) = weak.upgrade() {
                    if query_state.borrow().is_empty() {
                        ui.set_search_visible(false);
                    } else {
                        query_state.borrow_mut().clear();
                        ui.set_search_query("".into());
                        refresh_search(&ui, &controller, all_groups.get(), "");
                    }
                }
            });
        }
        {
            // P2-03: arrow keys move the selection and wrap around.
            let weak = weak.clone();
            ui.on_search_move(move |delta: i32| {
                if let Some(ui) = weak.upgrade() {
                    let count = ui.get_search_results().row_count();
                    let next = quicknest::search::move_selection(
                        ui.get_search_selected_index(),
                        count,
                        delta,
                    );
                    ui.set_search_selected_index(next);
                }
            });
        }
        {
            // P2-03: Enter launches the selected item.
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            let machine = Rc::clone(&focus_machine);
            let pending = Rc::clone(&pending_command_confirm);
            ui.on_search_activate(move || {
                if let Some(ui) = weak.upgrade()
                    && let Some(item_id) = selected_search_id(&ui)
                {
                    ui.set_search_visible(false);
                    // P3-09: the Everything row is not a library item.
                    if let Some(query) = item_id.strip_prefix("everything:") {
                        launch_everything_handoff(&ui, &controller, query);
                        return;
                    }
                    launch_item_with_confirm(&ui, &controller, &machine, &pending, &item_id, false);
                }
            });
        }
        {
            // P2-03: Ctrl+Enter launches as administrator (UAC path).
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            let machine = Rc::clone(&focus_machine);
            let pending = Rc::clone(&pending_command_confirm);
            ui.on_search_activate_admin(move || {
                if let Some(ui) = weak.upgrade()
                    && let Some(item_id) = selected_search_id(&ui)
                {
                    ui.set_search_visible(false);
                    // P3-09: the Everything row is not a library item.
                    if let Some(query) = item_id.strip_prefix("everything:") {
                        launch_everything_handoff(&ui, &controller, query);
                        return;
                    }
                    launch_item_with_confirm(&ui, &controller, &machine, &pending, &item_id, true);
                }
            });
        }
        {
            // P2-03: Ctrl+D opens the selected item's location.
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            ui.on_search_open_location(move || {
                if let Some(ui) = weak.upgrade()
                    && let Some(item_id) = selected_search_id(&ui)
                {
                    ui.set_search_visible(false);
                    open_item_location_by_id(&ui, &controller, &item_id);
                }
            });
        }

        {
            // P1-06: a single click selects the tile and launches it
            // (baseline 2.4 "单击启动").
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            let machine = Rc::clone(&focus_machine);
            let pending = Rc::clone(&pending_command_confirm);
            ui.on_item_selected(move |item_id: slint::SharedString| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                // P3-01: Ctrl+click picks the item for a batch launch instead
                // of starting it.
                if control_key_held() {
                    controller
                        .borrow_mut()
                        .toggle_batch_selection(item_id.as_ref());
                    ui.set_active_item_id(item_id.clone());
                    // P4 审核修复：同步批量计数与菜单标题——旧实现只刷新
                    // 卡片高亮，批量启动/保存组合仍保持禁用。
                    let count = controller.borrow().batch_selection_ids().len();
                    ui.set_batch_count(count as i32);
                    ui.set_batch_launch_title(if count == 0 {
                        "批量启动所选".into()
                    } else {
                        format!("批量启动所选（{count}）").into()
                    });
                    refresh_ui(&controller, &weak);
                    return;
                }
                ui.set_active_item_id(item_id.clone());
                launch_item_with_confirm(
                    &ui,
                    &controller,
                    &machine,
                    &pending,
                    item_id.as_ref(),
                    false,
                );
            });
        }
        {
            // P2-04: translate the dragged content coordinates into a slot.
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            let hover = Rc::clone(&drag_group_hover);
            ui.on_item_drag_moved(
                move |_item_id: slint::SharedString,
                      x: f32,
                      y: f32,
                      window_x: f32,
                      window_y: f32,
                      columns: i32| {
                    if let Some(ui) = weak.upgrade() {
                        if ui.get_any_modal_open() || !ui.get_item_drag_active() {
                            return;
                        }
                        let count = controller.borrow().item_rows().len();
                        if count == 0 || columns <= 0 {
                            ui.set_item_drop_index(-1);
                        } else {
                            let (cell_width, cell_height) = controller.borrow().icon_size().cell();
                            let column = (x / cell_width).floor().max(0.0) as i32;
                            let row = (y / cell_height).floor().max(0.0) as i32;
                            let index = (row * columns + column).clamp(0, count as i32 - 1);
                            ui.set_item_drop_index(index);
                        }
                        // P2-05: hovering a non-active category row targets it.
                        let size = ui.window().size().to_logical(ui.window().scale_factor());
                        let groups = controller.borrow().group_rows();
                        let categories = controller.borrow().category_rows();
                        let zone = quicknest::drop_layout::hit_test_scrolled(
                            window_x,
                            window_y,
                            size.width,
                            size.height,
                            &groups,
                            &categories,
                            quicknest::drop_layout::ScrollOffsets {
                                group_x: ui.get_group_scroll_offset(),
                                category_y: ui.get_category_scroll_offset(),
                            },
                        );
                        let active_category = controller.borrow().active_category_id();
                        let active_group = controller.borrow().active_group_id();
                        match zone {
                            quicknest::drop_layout::DropZone::Category(id)
                                if active_category.as_deref() != Some(id.as_str()) =>
                            {
                                let name =
                                    controller.borrow().category_name(&id).unwrap_or_default();
                                ui.set_drag_target_category_id(id.clone().into());
                                ui.set_drag_target_group_id("".into());
                                ui.set_item_drop_index(-1);
                                ui.set_item_drag_hint(format!("松开以移动到「{name}」").into());
                                hover.borrow_mut().take();
                            }
                            quicknest::drop_layout::DropZone::Group(id)
                                if active_group.as_deref() != Some(id.as_str()) =>
                            {
                                let name = controller
                                    .borrow()
                                    .group_rows()
                                    .into_iter()
                                    .find(|group| group.id == id)
                                    .map(|group| group.name)
                                    .unwrap_or_default();
                                ui.set_drag_target_category_id("".into());
                                ui.set_drag_target_group_id(id.clone().into());
                                ui.set_item_drop_index(-1);
                                ui.set_item_drag_hint(format!("松开移动到「{name}」").into());
                                let mut hover_state = hover.borrow_mut();
                                match hover_state.as_ref() {
                                    Some((hovered, _)) if hovered == &id => {}
                                    _ => *hover_state = Some((id, Instant::now())),
                                }
                            }
                            quicknest::drop_layout::DropZone::None => {
                                ui.set_drag_target_category_id("".into());
                                ui.set_drag_target_group_id("".into());
                                ui.set_item_drag_hint("".into());
                                // P4 审核修复：非目标区域（标题栏/状态栏/侧栏
                                // 空白）不提供插入位置——旧实现保留最后一次
                                // 有效索引，释放后仍会提交排序。
                                ui.set_item_drop_index(-1);
                                hover.borrow_mut().take();
                            }
                            quicknest::drop_layout::DropZone::Grid => {
                                ui.set_drag_target_category_id("".into());
                                ui.set_drag_target_group_id("".into());
                                ui.set_item_drag_hint("".into());
                                hover.borrow_mut().take();
                            }
                            _ => {
                                ui.set_drag_target_category_id("".into());
                                ui.set_drag_target_group_id("".into());
                                ui.set_item_drag_hint("".into());
                                ui.set_item_drop_index(-1);
                                hover.borrow_mut().take();
                            }
                        }
                    }
                },
            );
        }
        {
            // P2-04/P2-05/P2-06: drop commits a reorder, a cross-category
            // move, or a cross-group move.
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            let hover = Rc::clone(&drag_group_hover);
            ui.on_item_drop_requested(move |item_id: slint::SharedString, index: i32| {
                if let Some(ui) = weak.upgrade() {
                    if ui.get_any_modal_open() {
                        return;
                    }
                    let target = ui.get_drag_target_category_id().to_string();
                    let target_group = ui.get_drag_target_group_id().to_string();
                    ui.set_drag_target_category_id("".into());
                    ui.set_drag_target_group_id("".into());
                    ui.set_item_drag_hint("".into());
                    hover.borrow_mut().take();
                    // P2-05: a hovered category (of the active group) wins.
                    let active = controller.borrow().active_category_id();
                    if !target.is_empty() && active.as_deref() != Some(target.as_str()) {
                        let changed = controller
                            .borrow_mut()
                            .move_item_to_category(item_id.as_ref(), &target);
                        let name = controller
                            .borrow()
                            .category_name(&target)
                            .unwrap_or_default();
                        refresh_ui(&controller, &weak);
                        if changed {
                            ui.set_validation_status(format!("已移动到「{name}」").into());
                        }
                        return;
                    }
                    // P2-06: a hovered group tab (fast drop or spring-load)
                    // targets that group's current category. The lookups bind
                    // to locals first: borrows inside a let-chain condition
                    // live through the body, so a nested borrow_mut() would
                    // panic.
                    let group_category = if target_group.is_empty() {
                        None
                    } else {
                        controller
                            .borrow()
                            .active_category_id_of_group(&target_group)
                    };
                    let item_category = controller.borrow().category_id_of_item(item_id.as_ref());
                    if !target_group.is_empty()
                        && let Some(category_id) = group_category
                        && item_category.as_deref() != Some(category_id.as_str())
                    {
                        let scope = controller
                            .borrow()
                            .group_rows()
                            .into_iter()
                            .find(|group| group.id == target_group)
                            .map(|group| group.name)
                            .unwrap_or_default();
                        let name = controller
                            .borrow()
                            .category_name(&category_id)
                            .unwrap_or_default();
                        let changed = controller
                            .borrow_mut()
                            .move_item_to_group_category(item_id.as_ref(), &category_id);
                        refresh_ui(&controller, &weak);
                        if changed {
                            ui.set_validation_status(
                                format!("已移动到「{scope} · {name}」").into(),
                            );
                        }
                        return;
                    }
                    if index < 0 {
                        return;
                    }
                    // P2-04 reorder — but after a spring-load switch the item
                    // lives in another group, so a grid drop moves it into the
                    // newly active category instead.
                    let in_active = controller
                        .borrow()
                        .library()
                        .active_category()
                        .map(|category| {
                            category
                                .items
                                .iter()
                                .any(|item| item.id == item_id.as_str())
                        })
                        .unwrap_or(false);
                    if in_active {
                        let changed = controller
                            .borrow_mut()
                            .move_item_to_index(item_id.as_ref(), index as usize);
                        refresh_ui(&controller, &weak);
                        if changed {
                            ui.set_validation_status("已调整项目顺序".into());
                        }
                    } else {
                        let active = controller.borrow().active_category_id();
                        let Some(category_id) = active else {
                            return;
                        };
                        if controller
                            .borrow()
                            .category_id_of_item(item_id.as_ref())
                            .as_deref()
                            == Some(category_id.as_str())
                        {
                            return;
                        }
                        let scope = controller
                            .borrow()
                            .group_rows()
                            .into_iter()
                            .find(|group| group.active)
                            .map(|group| group.name)
                            .unwrap_or_default();
                        let name = controller
                            .borrow()
                            .category_name(&category_id)
                            .unwrap_or_default();
                        let changed = controller
                            .borrow_mut()
                            .move_item_to_group_category(item_id.as_ref(), &category_id);
                        refresh_ui(&controller, &weak);
                        if changed {
                            ui.set_validation_status(
                                format!("已移动到「{scope} · {name}」").into(),
                            );
                        }
                    }
                }
            });
        }
        {
            // P2-06: spring-load — hovering a group tab for 400 ms during an
            // internal drag switches to that group (sidebar only, the grid
            // keeps showing the source so the drag capture survives).
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            let hover = Rc::clone(&drag_group_hover);
            let timer = Timer::default();
            timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                if !ui.get_item_drag_active() {
                    hover.borrow_mut().take();
                    return;
                }
                let Some((group_id, since)) = hover.borrow().clone() else {
                    return;
                };
                if since.elapsed() < Duration::from_millis(400) {
                    return;
                }
                hover.borrow_mut().take();
                let active = controller.borrow().active_group_id();
                if active.as_deref() == Some(group_id.as_str()) {
                    return;
                }
                let name = controller
                    .borrow()
                    .group_rows()
                    .into_iter()
                    .find(|group| group.id == group_id)
                    .map(|group| group.name)
                    .unwrap_or_default();
                controller.borrow_mut().select_group(&group_id);
                refresh_sidebar(&controller, &weak);
                ui.set_drag_target_group_id("".into());
                ui.set_item_drag_hint("".into());
                ui.set_validation_status(format!("已切换到「{name}」").into());
            });
            _spring_timer_keepalive = Some(timer);
        }
        {
            let controller = Rc::clone(&controller);
            let weak = weak.clone();
            let editing = Rc::clone(&editing_item);
            ui.on_grid_add_requested(move || {
                if let Some(ui) = weak.upgrade() {
                    open_item_editor(&ui, &controller, &editing, None);
                }
            });
        }
        {
            // P2-11: "新建项目" acts on the right-clicked category, not only
            // the currently active one.
            let controller = Rc::clone(&controller);
            let weak = weak.clone();
            let editing = Rc::clone(&editing_item);
            ui.on_category_menu_add_item(move |category_id: slint::SharedString| {
                controller
                    .borrow_mut()
                    .select_category(category_id.as_ref());
                refresh_ui(&controller, &weak);
                if let Some(ui) = weak.upgrade() {
                    open_item_editor(&ui, &controller, &editing, None);
                }
            });
        }
        {
            let controller = Rc::clone(&controller);
            let weak = weak.clone();
            let editing = Rc::clone(&editing_item);
            ui.on_item_menu_edit(move |item_id: slint::SharedString| {
                if let Some(ui) = weak.upgrade() {
                    open_item_editor(&ui, &controller, &editing, Some(item_id.to_string()));
                }
            });
        }
        {
            let controller = Rc::clone(&controller);
            let weak = weak.clone();
            let pending = Rc::clone(&pending_item_delete);
            ui.on_item_menu_delete(move |item_id: slint::SharedString| {
                if let Some(ui) = weak.upgrade() {
                    let name = controller
                        .borrow()
                        .item_draft(item_id.as_ref())
                        .map(|draft| draft.name)
                        .unwrap_or_default();
                    ui.set_item_delete_confirm_text(
                        format!("确定删除“{name}”？此操作不可撤销。").into(),
                    );
                    *pending.borrow_mut() = Some(item_id.to_string());
                    ui.set_item_delete_confirm_visible(true);
                }
            });
        }
        {
            let controller = Rc::clone(&controller);
            let weak = weak.clone();
            // P3-01: pick or un-pick an item for a batch launch.
            {
                let weak = weak.clone();
                let controller = Rc::clone(&controller);
                ui.on_item_menu_toggle_batch(move |item_id: slint::SharedString| {
                    let Some(ui) = weak.upgrade() else {
                        return;
                    };
                    controller
                        .borrow_mut()
                        .toggle_batch_selection(item_id.as_ref());
                    refresh_ui(&controller, &weak);
                    let count = controller.borrow().batch_selection_ids().len();
                    ui.set_batch_count(count as i32);
                    ui.set_batch_launch_title(if count == 0 {
                        "批量启动所选".into()
                    } else {
                        format!("批量启动所选（{count}）").into()
                    });
                });
            }
            // P3-01: start every selected item, in grid order.
            {
                let weak = weak.clone();
                let controller = Rc::clone(&controller);
                ui.on_item_menu_batch_launch(move || {
                    let Some(ui) = weak.upgrade() else {
                        return;
                    };
                    let selected = controller.borrow().batch_selection_ids();
                    if selected.is_empty() {
                        ui.set_validation_status("未选择项目".into());
                        return;
                    }
                    // Grid order, so the batch follows the visible layout.
                    let mut ordered: Vec<String> = Vec::new();
                    for row in controller.borrow().item_rows() {
                        if selected.iter().any(|id| id == &row.id) {
                            ordered.push(row.id);
                        }
                    }
                    let total = ordered.len();
                    let mut started = 0_usize;
                    let mut delayed = 0_usize;
                    let mut failed = 0_usize;
                    let mut skipped = 0_usize;
                    for item_id in &ordered {
                        // P3-08 (baseline 8.3/8.4): unverified command items
                        // never run silently inside a batch; skip and report.
                        let unverified = controller.borrow().item(item_id).is_some_and(|item| {
                            item.item_type == quicknest::library_model::ItemKind::Command
                                && item.launch.confirm_on_first_run
                        });
                        if unverified {
                            skipped += 1;
                            continue;
                        }
                        let (launched, delay) =
                            launch_item_maybe_delayed(&controller, item_id, false);
                        if launched {
                            if delay > 0 {
                                delayed += 1;
                            } else {
                                started += 1;
                                controller.borrow_mut().record_launch(item_id);
                            }
                        } else {
                            failed += 1;
                        }
                    }
                    controller.borrow_mut().clear_batch_selection();
                    ui.set_batch_count(0);
                    ui.set_batch_launch_title("批量启动所选".into());
                    refresh_ui(&controller, &weak);
                    let mut message = format!("批量启动：{started}/{total} 个项目已启动");
                    if delayed > 0 {
                        message.push_str(&format!("，{delayed} 个已排队等待启动"));
                    }
                    if skipped > 0 {
                        message.push_str(&format!("，跳过 {skipped} 个未确认命令"));
                    }
                    if failed > 0 {
                        message.push_str(&format!("（{failed} 个失败）"));
                    }
                    ui.set_validation_status(message.into());
                });
            }
            ui.on_item_menu_duplicate(move |item_id: slint::SharedString| {
                controller.borrow_mut().duplicate_item(item_id.as_ref());
                refresh_ui(&controller, &weak);
            });
        }
        {
            let controller = Rc::clone(&controller);
            let weak = weak.clone();
            ui.on_item_menu_copy_path(move |item_id: slint::SharedString| {
                let path = controller
                    .borrow()
                    .item_draft(item_id.as_ref())
                    .map(|draft| draft.path)
                    .unwrap_or_default();
                if path.is_empty() {
                    return;
                }
                let copied = copy_path_to_clipboard(&path);
                if let Some(ui) = weak.upgrade() {
                    ui.set_validation_status(
                        if copied {
                            "已复制路径"
                        } else {
                            "复制路径失败"
                        }
                        .into(),
                    );
                }
            });
        }
        {
            let controller = Rc::clone(&controller);
            let weak = weak.clone();
            let pending = Rc::clone(&pending_item_move);
            ui.on_item_menu_move(move |item_id: slint::SharedString| {
                if let Some(ui) = weak.upgrade() {
                    let names: Vec<slint::SharedString> = controller
                        .borrow()
                        .category_rows()
                        .iter()
                        .map(|row| row.name.clone().into())
                        .collect();
                    ui.set_move_target_names(ModelRc::new(VecModel::from(names)));
                    ui.set_move_dialog_target_index(0);
                    *pending.borrow_mut() = Some(item_id.to_string());
                    ui.set_move_dialog_visible(true);
                }
            });
        }
        {
            let controller = Rc::clone(&controller);
            let weak = weak.clone();
            let pending = Rc::clone(&pending_item_move);
            ui.on_move_dialog_confirmed(move |index: i32| {
                let target = controller
                    .borrow()
                    .category_rows()
                    .get(index.max(0) as usize)
                    .map(|row| row.id.clone());
                let item = pending.borrow_mut().take();
                if let (Some(item_id), Some(target_id)) = (item, target) {
                    controller
                        .borrow_mut()
                        .move_item_to_category(&item_id, &target_id);
                }
                if let Some(ui) = weak.upgrade() {
                    ui.set_move_dialog_visible(false);
                }
                refresh_ui(&controller, &weak);
            });
        }
        {
            let weak = weak.clone();
            let pending = Rc::clone(&pending_item_move);
            ui.on_move_dialog_cancelled(move || {
                pending.borrow_mut().take();
                if let Some(ui) = weak.upgrade() {
                    ui.set_move_dialog_visible(false);
                }
            });
        }
        {
            // P1-06: launch entries; P1-08 binds reveal-in-explorer.
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            let machine = Rc::clone(&focus_machine);
            let pending = Rc::clone(&pending_command_confirm);
            ui.on_item_menu_open(move |item_id: slint::SharedString| {
                if let Some(ui) = weak.upgrade() {
                    launch_item_with_confirm(
                        &ui,
                        &controller,
                        &machine,
                        &pending,
                        item_id.as_ref(),
                        false,
                    );
                }
            });
        }
        {
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            let machine = Rc::clone(&focus_machine);
            let pending = Rc::clone(&pending_command_confirm);
            ui.on_item_menu_run_as_admin(move |item_id: slint::SharedString| {
                if let Some(ui) = weak.upgrade() {
                    launch_item_with_confirm(
                        &ui,
                        &controller,
                        &machine,
                        &pending,
                        item_id.as_ref(),
                        true,
                    );
                }
            });
        }
        {
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            ui.on_item_menu_open_location(move |item_id: slint::SharedString| {
                if let Some(ui) = weak.upgrade() {
                    open_item_location_by_id(&ui, &controller, item_id.as_ref());
                }
            });
        }
        {
            let weak = weak.clone();
            let editing = Rc::clone(&editing_item);
            ui.on_item_dialog_cancelled(move || {
                editing.borrow_mut().take();
                if let Some(ui) = weak.upgrade() {
                    ui.set_item_dialog_visible(false);
                }
            });
        }
        {
            let controller = Rc::clone(&controller);
            let weak = weak.clone();
            let editing = Rc::clone(&editing_item);
            ui.on_item_dialog_confirmed(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let mut draft = draft_from_ui(&ui);
                // P2-10: typed Shell/URI/UNC targets get their real kind and
                // a namespace display name before LNK enrichment runs.
                fill_shell_draft(&mut draft);
                fill_lnk_draft(&mut draft);
                let result = match editing.borrow().clone() {
                    Some(id) => controller.borrow_mut().update_item(&id, draft),
                    None => controller.borrow_mut().add_item(draft).map(|_| ()),
                };
                match result {
                    Ok(()) => {
                        editing.borrow_mut().take();
                        ui.set_item_dialog_visible(false);
                        refresh_ui(&controller, &weak);
                    }
                    Err(error) => ui.set_item_dialog_error(error.into()),
                }
            });
        }
        {
            let weak = weak.clone();
            ui.on_item_dialog_browse(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let Some(path) = browse_for_item(&ui) else {
                    return;
                };
                let text = path.to_string_lossy().into_owned();
                let kind = quicknest::library_model::ItemKind::from_path(&text);
                if ui.get_item_dialog_name().is_empty()
                    && let Some(name) = path.file_stem()
                {
                    ui.set_item_dialog_name(name.to_string_lossy().into_owned().into());
                }
                if kind == quicknest::library_model::ItemKind::Lnk {
                    fill_lnk_metadata(&ui, &path);
                }
                if ui.get_item_dialog_workdir().is_empty() {
                    ui.set_item_dialog_workdir(
                        quicknest::library_model::Item::default_workdir(kind, &text).into(),
                    );
                }
                ui.set_item_dialog_target(text.into());
                ui.set_item_dialog_type_index(kind.index() as i32);
            });
        }
        {
            // P3-08 (baseline 8.3): confirming runs the command once and
            // clears the unverified mark, so the prompt shows only once.
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            let machine = Rc::clone(&focus_machine);
            let pending = Rc::clone(&pending_command_confirm);
            ui.on_command_confirm_confirmed(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let Some((item_id, as_admin)) = pending.borrow_mut().take() else {
                    return;
                };
                ui.set_command_confirm_visible(false);
                controller.borrow_mut().confirm_command(&item_id);
                launch_item_by_id(&ui, &controller, &machine, &item_id, as_admin);
            });
        }
        {
            // P3-08 (baseline 8.3): cancelling never runs the command.
            let weak = weak.clone();
            let pending = Rc::clone(&pending_command_confirm);
            ui.on_command_confirm_cancelled(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                pending.borrow_mut().take();
                ui.set_command_confirm_visible(false);
                ui.set_validation_status("已取消执行".into());
            });
        }
        {
            let weak = weak.clone();
            let controller = Rc::clone(&controller);
            let pending = Rc::clone(&pending_item_delete);
            ui.on_item_delete_confirm_confirmed(move || {
                if let Some(item_id) = pending.borrow_mut().take() {
                    controller.borrow_mut().delete_item(&item_id);
                }
                if let Some(ui) = weak.upgrade() {
                    ui.set_item_delete_confirm_visible(false);
                }
                refresh_ui(&controller, &weak);
            });
        }
        {
            let weak = weak.clone();
            let pending = Rc::clone(&pending_item_delete);
            ui.on_item_delete_confirm_cancelled(move || {
                pending.borrow_mut().take();
                if let Some(ui) = weak.upgrade() {
                    ui.set_item_delete_confirm_visible(false);
                }
            });
        }

        {
            let controller = Rc::clone(&controller);
            let weak = weak.clone();
            ui.on_group_selected(move |group_id| {
                controller.borrow_mut().select_group(&group_id);
                refresh_ui(&controller, &weak);
            });
        }
        {
            let controller = Rc::clone(&controller);
            let weak = weak.clone();
            ui.on_category_selected(move |category_id| {
                controller
                    .borrow_mut()
                    .select_category(category_id.as_ref());
                refresh_ui(&controller, &weak);
            });
        }
        {
            let controller = Rc::clone(&controller);
            let weak_root = weak.clone();
            // P2-11: group create/rename share this dialog; the target id
            // distinguishes rename (Some) from create (None).
            let group_dialog_target: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
            let pending_group_delete: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
            let group_migrate_ids: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

            {
                let weak = weak_root.clone();
                let target = Rc::clone(&group_dialog_target);
                ui.on_group_create_requested(move || {
                    *target.borrow_mut() = None;
                    if let Some(ui) = weak.upgrade() {
                        ui.set_group_dialog_visible(true);
                        ui.set_group_dialog_title("新建分组".into());
                        ui.set_group_dialog_initial("".into());
                        ui.set_group_dialog_error("".into());
                    }
                });
            }
            {
                let weak = weak_root.clone();
                let target = Rc::clone(&group_dialog_target);
                let controller = Rc::clone(&controller);
                ui.on_group_dialog_confirmed(move |name: slint::SharedString| {
                    let trimmed = name.trim().to_string();
                    if trimmed.is_empty() {
                        if let Some(ui) = weak.upgrade() {
                            ui.set_group_dialog_error("名称不能为空".into());
                        }
                        return;
                    }
                    let target_id = target.borrow().clone();
                    let changed = match target_id {
                        Some(id) => controller.borrow_mut().rename_group(&id, &trimmed),
                        None => controller.borrow_mut().add_group(&trimmed).is_some(),
                    };
                    if !changed {
                        if let Some(ui) = weak.upgrade() {
                            ui.set_group_dialog_error("分组名称已存在".into());
                        }
                        return;
                    }
                    if let Some(ui) = weak.upgrade() {
                        ui.set_group_dialog_visible(false);
                    }
                    refresh_ui(&controller, &weak);
                });
            }
            {
                let weak = weak_root.clone();
                ui.on_group_dialog_cancelled(move || {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_group_dialog_visible(false);
                    }
                });
            }
            {
                let weak = weak_root.clone();
                let target = Rc::clone(&group_dialog_target);
                let controller = Rc::clone(&controller);
                ui.on_group_menu_rename(move |group_id: slint::SharedString| {
                    let id = group_id.to_string();
                    let name = controller
                        .borrow()
                        .library()
                        .group(&id)
                        .map(|group| group.name.clone())
                        .unwrap_or_default();
                    *target.borrow_mut() = Some(id);
                    if let Some(ui) = weak.upgrade() {
                        ui.set_group_dialog_visible(true);
                        ui.set_group_dialog_title("重命名分组".into());
                        ui.set_group_dialog_initial(name.into());
                        ui.set_group_dialog_error("".into());
                    }
                });
            }
            {
                let weak = weak_root.clone();
                let controller = Rc::clone(&controller);
                ui.on_group_menu_color(
                    move |group_id: slint::SharedString, color: slint::SharedString| {
                        controller
                            .borrow_mut()
                            .set_group_color(group_id.as_ref(), color.as_ref());
                        refresh_ui(&controller, &weak);
                    },
                );
            }
            {
                let weak = weak_root.clone();
                let controller = Rc::clone(&controller);
                ui.on_group_menu_left(move |group_id: slint::SharedString| {
                    let id = group_id.to_string();
                    let position = controller
                        .borrow()
                        .group_rows()
                        .iter()
                        .position(|row| row.id == id);
                    if let Some(position) = position {
                        controller
                            .borrow_mut()
                            .move_group(&id, position.saturating_sub(1));
                    }
                    refresh_ui(&controller, &weak);
                });
            }
            {
                let weak = weak_root.clone();
                let controller = Rc::clone(&controller);
                ui.on_group_menu_right(move |group_id: slint::SharedString| {
                    let id = group_id.to_string();
                    let position = controller
                        .borrow()
                        .group_rows()
                        .iter()
                        .position(|row| row.id == id);
                    if let Some(position) = position {
                        controller.borrow_mut().move_group(&id, position + 1);
                    }
                    refresh_ui(&controller, &weak);
                });
            }
            {
                let weak = weak_root.clone();
                let controller = Rc::clone(&controller);
                let pending = Rc::clone(&pending_group_delete);
                let migrate_ids = Rc::clone(&group_migrate_ids);
                ui.on_group_menu_delete(move |group_id: slint::SharedString| {
                    let id = group_id.to_string();
                    // Bind the borrow to a local: the temporary in a match
                    // scrutinee would live through the Err arm and panic the
                    // `borrow()` used to build the migration list.
                    let outcome = controller.borrow_mut().delete_group(&id);
                    match outcome {
                        Ok(_) => refresh_ui(&controller, &weak),
                        Err(pending_delete) => {
                            let others: Vec<(String, String)> = controller
                                .borrow()
                                .group_rows()
                                .into_iter()
                                .filter(|row| row.id != id)
                                .map(|row| (row.id, row.name))
                                .collect();
                            *pending.borrow_mut() = Some(id);
                            *migrate_ids.borrow_mut() =
                                others.iter().map(|(id, _)| id.clone()).collect();
                            if let Some(ui) = weak.upgrade() {
                                let names: Vec<slint::SharedString> = others
                                    .iter()
                                    .map(|(_, name)| slint::SharedString::from(name.clone()))
                                    .collect();
                                ui.set_group_delete_confirm_names(ModelRc::new(
                                    VecModel::from(names),
                                ));
                                ui.set_group_delete_confirm_text(
                                    format!(
                                        "分组「{}」包含 {} 个项目、{} 个分类。选择迁移到其他分组，或删除全部内容。",
                                        pending_delete.group_name,
                                        pending_delete.item_count,
                                        pending_delete.category_count
                                    )
                                    .into(),
                                );
                                ui.set_group_delete_confirm_visible(true);
                            }
                        }
                    }
                });
            }
            {
                let weak = weak_root.clone();
                let controller = Rc::clone(&controller);
                let pending = Rc::clone(&pending_group_delete);
                ui.on_group_delete_confirm_delete(move || {
                    if let Some(id) = pending.borrow_mut().take() {
                        controller.borrow_mut().delete_group_confirmed(&id);
                    }
                    if let Some(ui) = weak.upgrade() {
                        ui.set_group_delete_confirm_visible(false);
                    }
                    refresh_ui(&controller, &weak);
                });
            }
            {
                let weak = weak_root.clone();
                let controller = Rc::clone(&controller);
                let pending = Rc::clone(&pending_group_delete);
                let migrate_ids = Rc::clone(&group_migrate_ids);
                ui.on_group_delete_confirm_migrate(move |index: i32| {
                    let source = pending.borrow_mut().take();
                    let target = migrate_ids.borrow().get(index.max(0) as usize).cloned();
                    if let (Some(source), Some(target)) = (source, target) {
                        controller.borrow_mut().migrate_group_into(&source, &target);
                    }
                    if let Some(ui) = weak.upgrade() {
                        ui.set_group_delete_confirm_visible(false);
                    }
                    refresh_ui(&controller, &weak);
                });
            }
            {
                let weak = weak_root.clone();
                let pending = Rc::clone(&pending_group_delete);
                ui.on_group_delete_confirm_cancel(move || {
                    pending.borrow_mut().take();
                    if let Some(ui) = weak.upgrade() {
                        ui.set_group_delete_confirm_visible(false);
                    }
                });
            }
        }
        {
            let controller = Rc::clone(&controller);
            let weak_root = weak.clone();
            // The dialog doubles for create (empty target) and rename (target
            // id set); the mode rides in this cell.
            let dialog_target: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
            let dialog_target_for_open = Rc::clone(&dialog_target);
            ui.on_category_create_opened({
                let weak = weak_root.clone();
                move || {
                    *dialog_target_for_open.borrow_mut() = None;
                    if let Some(ui) = weak.upgrade() {
                        ui.set_category_dialog_visible(true);
                        ui.set_category_dialog_title("新建分类".into());
                        ui.set_category_dialog_initial("".into());
                        ui.set_category_dialog_error("".into());
                    }
                }
            });
            // P1-03: native ContextMenuArea entries act on the clicked row.
            let dialog_target_for_rename = Rc::clone(&dialog_target);
            let controller_for_rename = Rc::clone(&controller);
            let weak_for_rename = weak.clone();
            ui.on_category_menu_rename(move |category_id: slint::SharedString| {
                let id = category_id.to_string();
                controller_for_rename
                    .borrow_mut()
                    .select_category(category_id.as_ref());
                if let Some(ui) = weak_for_rename.upgrade() {
                    {
                        let name = controller_for_rename
                            .borrow()
                            .library()
                            .active_group()
                            .and_then(|group| group.category(&id))
                            .map(|category| category.name.clone())
                            .unwrap_or_default();
                        *dialog_target_for_rename.borrow_mut() = Some(id);
                        ui.set_category_dialog_visible(true);
                        ui.set_category_dialog_title("重命名分类".into());
                        ui.set_category_dialog_initial(name.into());
                        ui.set_category_dialog_error("".into());
                    }
                }
            });
            // UI 体验修复（2026-11-07）：删除确认对话框的迁移目标按索引对应
            // 这些 id；列表在打开时排除被删分类本身。
            let category_migrate_ids: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
            {
                let controller = Rc::clone(&controller);
                let weak = weak.clone();
                let dialog_target_for_delete = Rc::clone(&dialog_target);
                let migrate_ids = Rc::clone(&category_migrate_ids);
                ui.on_category_menu_delete(move |category_id: slint::SharedString| {
                    let id = category_id.to_string();
                    controller
                        .borrow_mut()
                        .select_category(category_id.as_ref());
                    if let Err(item_count) = controller.borrow_mut().delete_category(&id) {
                        // Baseline 2.3: confirm — migrate or drop.
                        // 旧实现从不写入 dialog_target，“删除项目”按钮会作用在
                        // 过期目标上（或静默无效）。
                        *dialog_target_for_delete.borrow_mut() = Some(id.clone());
                        let rows = controller.borrow().category_rows();
                        let mut names: Vec<slint::SharedString> = Vec::new();
                        let mut ids: Vec<String> = Vec::new();
                        for row in rows {
                            if row.id != id {
                                names.push(row.name.clone().into());
                                ids.push(row.id.clone());
                            }
                        }
                        *migrate_ids.borrow_mut() = ids;
                        if let Some(ui) = weak.upgrade() {
                            ui.set_category_migrate_names(ModelRc::new(VecModel::from(
                                names,
                            )));
                            ui.set_category_delete_confirm_visible(true);
                            ui.set_category_delete_confirm_text(
                                format!(
                                    "分类包含 {item_count} 个项目。选择迁移到其他分类，或删除这些项目。"
                                )
                                .into(),
                            );
                        }
                    } else {
                        refresh_ui(&controller, &weak);
                    }
                });
            }
            {
                let controller = Rc::clone(&controller);
                let weak = weak.clone();
                ui.on_category_menu_up(move |category_id: slint::SharedString| {
                    let id = category_id.to_string();
                    // Bind the borrow to its own statement: a temporary borrow in
                    // the `if let` scrutinee lives until the end of the block and
                    // would panic the `borrow_mut` below.
                    let position = controller
                        .borrow()
                        .category_rows()
                        .iter()
                        .position(|row| row.id == id);
                    if let Some(position) = position {
                        controller
                            .borrow_mut()
                            .move_category(&id, position.saturating_sub(1));
                    }
                    refresh_ui(&controller, &weak);
                });
            }
            {
                let controller = Rc::clone(&controller);
                let weak = weak.clone();
                ui.on_category_menu_down(move |category_id: slint::SharedString| {
                    let id = category_id.to_string();
                    let position = controller
                        .borrow()
                        .category_rows()
                        .iter()
                        .position(|row| row.id == id);
                    if let Some(position) = position {
                        controller.borrow_mut().move_category(&id, position + 1);
                    }
                    refresh_ui(&controller, &weak);
                });
            }
            {
                let controller = Rc::clone(&controller);
                let weak = weak.clone();
                let dialog_target = Rc::clone(&dialog_target);
                ui.on_category_dialog_confirmed(move |name: slint::SharedString| {
                    let trimmed = name.trim().to_string();
                    let target = dialog_target.borrow().clone();
                    if trimmed.is_empty() {
                        if let Some(ui) = weak.upgrade() {
                            ui.set_category_dialog_error("名称不能为空".into());
                        }
                        return;
                    }
                    let mut duplicate = false;
                    match target {
                        Some(id) => {
                            duplicate = !controller.borrow_mut().rename_category(&id, &trimmed);
                        }
                        None => {
                            if controller.borrow_mut().add_category(&trimmed).is_none() {
                                duplicate = true;
                            }
                        }
                    }
                    if duplicate {
                        if let Some(ui) = weak.upgrade() {
                            ui.set_category_dialog_error("分类名称已存在".into());
                        }
                        return;
                    }
                    if let Some(ui) = weak.upgrade() {
                        ui.set_category_dialog_visible(false);
                    }
                    refresh_ui(&controller, &weak);
                });
            }
            {
                let weak = weak.clone();
                ui.on_category_dialog_cancelled(move || {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_category_dialog_visible(false);
                    }
                });
            }
            {
                let controller = Rc::clone(&controller);
                let weak = weak.clone();
                let dialog_target = Rc::clone(&dialog_target);
                ui.on_category_delete_confirm_delete(move || {
                    let target = dialog_target.borrow().clone();
                    if let Some(id) = target {
                        controller.borrow_mut().delete_category_confirmed(&id);
                    }
                    if let Some(ui) = weak.upgrade() {
                        ui.set_category_delete_confirm_visible(false);
                    }
                    refresh_ui(&controller, &weak);
                });
            }
            {
                let controller = Rc::clone(&controller);
                let weak = weak.clone();
                let dialog_target = Rc::clone(&dialog_target);
                let migrate_ids = Rc::clone(&category_migrate_ids);
                ui.on_category_delete_confirm_migrate(move |index: i32| {
                    let source = dialog_target.borrow().clone();
                    let ids = migrate_ids.borrow();
                    let target = usize::try_from(index)
                        .ok()
                        .and_then(|position| ids.get(position).cloned());
                    drop(ids);
                    if let (Some(source), Some(target)) = (source, target) {
                        controller
                            .borrow_mut()
                            .migrate_category_items_into(&source, &target);
                    }
                    if let Some(ui) = weak.upgrade() {
                        ui.set_category_delete_confirm_visible(false);
                    }
                    refresh_ui(&controller, &weak);
                });
            }
            {
                let weak = weak.clone();
                ui.on_category_delete_confirm_cancel(move || {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_category_delete_confirm_visible(false);
                    }
                });
            }
        }

        // P1-17: external drag & drop from Explorer (baseline 5.4-5.6).
        // P0-04/05 own the OLE target and CF_HDROP reading; this layer only
        // maps the queued screen point to a Slint region and imports paths.
        #[cfg(windows)]
        {
            use quicknest::drop_layout::{DropZone, hit_test_scrolled};
            use quicknest::infrastructure::windows::coordinates;
            use quicknest::infrastructure::windows::drop_target::DropRegistration;
            use quicknest::infrastructure::windows::file_drop::{
                FileDropEvent, FileDropStats, receiving_target,
            };
            use quicknest::library_controller::LibraryController;
            use quicknest::library_model::ItemDraft;
            use quicknest::window_state::{SuppressionReason, SuppressionToken};
            use std::path::PathBuf;
            use std::sync::mpsc::sync_channel;
            use std::time::Instant;

            type SharedController = Rc<RefCell<LibraryController>>;

            fn clear_drop_highlight(ui: &MainWindow) {
                ui.set_drop_group_id("".into());
                ui.set_drop_category_id("".into());
                ui.set_drop_grid_active(false);
                ui.set_drop_hint("".into());
            }

            fn update_drop_highlight(
                ui: &MainWindow,
                controller: &SharedController,
                zone: &DropZone,
            ) {
                let mut group_id = String::new();
                let mut category_id = String::new();
                let mut grid = false;
                let mut hint = String::new();
                match zone {
                    DropZone::Group(id) => {
                        let name = controller
                            .borrow()
                            .group_rows()
                            .into_iter()
                            .find(|group| group.id == *id)
                            .map(|group| group.name)
                            .unwrap_or_default();
                        group_id = id.clone();
                        hint = format!("松开以添加到「{name}」");
                    }
                    DropZone::Category(id) => {
                        let name = controller.borrow().category_name(id).unwrap_or_default();
                        category_id = id.clone();
                        hint = format!("松开以添加到「{name}」");
                    }
                    DropZone::Grid => {
                        grid = true;
                        let active = controller.borrow().active_category_id();
                        match active {
                            Some(id) => {
                                let name =
                                    controller.borrow().category_name(&id).unwrap_or_default();
                                category_id = id;
                                hint = format!("松开以添加到「{name}」（追加到末尾）");
                            }
                            None => hint = "当前分组没有分类，无法添加".to_string(),
                        }
                    }
                    DropZone::None => {}
                }
                ui.set_drop_group_id(group_id.into());
                ui.set_drop_category_id(category_id.into());
                ui.set_drop_grid_active(grid);
                ui.set_drop_hint(hint.into());
            }

            fn zone_at(ui: &MainWindow, controller: &SharedController, x: i32, y: i32) -> DropZone {
                if ui.get_any_modal_open() {
                    return DropZone::None;
                }
                let Ok(snapshot) = coordinates::from_screen(ui.window(), (x, y)) else {
                    return DropZone::None;
                };
                if !snapshot.inside {
                    return DropZone::None;
                }
                let size = ui.window().size().to_logical(ui.window().scale_factor());
                let groups = controller.borrow().group_rows();
                let categories = controller.borrow().category_rows();
                hit_test_scrolled(
                    snapshot.logical.0,
                    snapshot.logical.1,
                    size.width,
                    size.height,
                    &groups,
                    &categories,
                    quicknest::drop_layout::ScrollOffsets {
                        group_x: ui.get_group_scroll_offset(),
                        category_y: ui.get_category_scroll_offset(),
                    },
                )
            }

            fn import_paths(
                ui: &MainWindow,
                controller: &SharedController,
                weak: &slint::Weak<MainWindow>,
                category_id: &str,
                paths: Vec<PathBuf>,
            ) -> bool {
                if paths.is_empty() {
                    return false;
                }
                let total = paths.len();
                let mut added = 0_usize;
                let mut first_error: Option<String> = None;
                for path in paths {
                    let mut draft = ItemDraft::from_path(&path);
                    // P2-09: external drops of MSI advertised shortcuts keep
                    // the original LNK and record the advertised flag, so a
                    // later broken entry can point at install repair.
                    fill_lnk_draft(&mut draft);
                    fill_shell_draft(&mut draft);
                    match controller
                        .borrow_mut()
                        .add_item_to_category(category_id, draft)
                    {
                        Ok(_) => added += 1,
                        Err(error) => {
                            if first_error.is_none() {
                                first_error = Some(error);
                            }
                        }
                    }
                }
                refresh_ui(controller, weak);
                let name = controller
                    .borrow()
                    .category_name(category_id)
                    .unwrap_or_default();
                let status = if added == total {
                    format!("已添加 {added} 个项目到「{name}」")
                } else if added == 0 {
                    format!("添加失败：{}", first_error.unwrap_or_default())
                } else {
                    format!(
                        "已添加 {added}/{total} 个项目到「{name}」：{}",
                        first_error.unwrap_or_default()
                    )
                };
                ui.set_validation_status(status.into());
                added > 0
            }

            fn hover_zone(
                ui: &MainWindow,
                controller: &SharedController,
                weak: &slint::Weak<MainWindow>,
                hover: &mut Option<(String, Instant)>,
                zone: &DropZone,
            ) {
                update_drop_highlight(ui, controller, zone);
                let DropZone::Group(id) = zone else {
                    *hover = None;
                    return;
                };
                let active = controller.borrow().active_group_id();
                if active.as_deref() == Some(id.as_str()) {
                    *hover = None;
                    return;
                }
                let ready = match hover {
                    Some((hovered, since)) if hovered == id => {
                        since.elapsed() >= Duration::from_millis(400)
                    }
                    _ => {
                        *hover = Some((id.clone(), Instant::now()));
                        false
                    }
                };
                if ready {
                    // Baseline 5.6: spring-load switches the target group after
                    // 400 ms of hovering; the next Over event re-highlights.
                    controller.borrow_mut().select_group(id);
                    refresh_ui(controller, weak);
                    *hover = None;
                }
            }

            fn drop_zone(
                ui: &MainWindow,
                controller: &SharedController,
                weak: &slint::Weak<MainWindow>,
                zone: DropZone,
                paths: Vec<PathBuf>,
            ) -> bool {
                if paths.is_empty() {
                    ui.set_validation_status("未添加项目：拖入内容为空".into());
                    return false;
                }
                match zone {
                    DropZone::Grid => {
                        let active = controller.borrow().active_category_id();
                        match active {
                            Some(category_id) => {
                                import_paths(ui, controller, weak, &category_id, paths)
                            }
                            None => {
                                ui.set_validation_status("未添加项目：当前分组没有分类".into());
                                false
                            }
                        }
                    }
                    DropZone::Category(category_id) => {
                        import_paths(ui, controller, weak, &category_id, paths)
                    }
                    DropZone::Group(group_id) => {
                        let active = controller.borrow().active_group_id();
                        if active.as_deref() != Some(group_id.as_str()) {
                            // A fast drop before the 400 ms spring-load still
                            // targets the group under the pointer.
                            controller.borrow_mut().select_group(&group_id);
                            refresh_ui(controller, weak);
                        }
                        let category = controller.borrow().active_category_id();
                        match category {
                            Some(category_id) => {
                                import_paths(ui, controller, weak, &category_id, paths)
                            }
                            None => {
                                ui.set_validation_status("未添加项目：目标分组没有分类".into());
                                false
                            }
                        }
                    }
                    DropZone::None => {
                        ui.set_validation_status("未添加项目：请拖到分类或项目网格".into());
                        false
                    }
                }
            }

            fn handle_drop_event(
                ui: &MainWindow,
                controller: &SharedController,
                weak: &slint::Weak<MainWindow>,
                machine: &SharedFocusMachine,
                suppression: &Rc<RefCell<Option<SuppressionToken>>>,
                hover: &mut Option<(String, Instant)>,
                event: FileDropEvent,
            ) {
                match event {
                    FileDropEvent::Enter { supported, x, y } => {
                        if supported {
                            // Baseline 10.5: hover holds an external-drag lock.
                            if suppression.borrow().is_none() {
                                *suppression.borrow_mut() = Some(
                                    machine
                                        .borrow()
                                        .acquire(SuppressionReason::ExternalDragHover),
                                );
                            }
                            hover_zone(ui, controller, weak, hover, &zone_at(ui, controller, x, y));
                        } else {
                            suppression.borrow_mut().take();
                            *hover = None;
                            clear_drop_highlight(ui);
                            ui.set_validation_status("仅支持从资源管理器拖入文件或文件夹".into());
                        }
                    }
                    FileDropEvent::Over { x, y } => {
                        hover_zone(ui, controller, weak, hover, &zone_at(ui, controller, x, y));
                    }
                    FileDropEvent::Leave => {
                        suppression.borrow_mut().take();
                        *hover = None;
                        clear_drop_highlight(ui);
                    }
                    FileDropEvent::Dropped { paths, x, y } => {
                        suppression.borrow_mut().take();
                        *hover = None;
                        clear_drop_highlight(ui);
                        let zone = zone_at(ui, controller, x, y);
                        if drop_zone(ui, controller, weak, zone, paths) {
                            // Keep the panel active after a successful drop so
                            // the release of the drag lock cannot immediately
                            // blur-hide it (baseline 10.5 release ordering).
                            show_and_focus(ui);
                        }
                    }
                    FileDropEvent::Failed { hresult } => {
                        suppression.borrow_mut().take();
                        *hover = None;
                        clear_drop_highlight(ui);
                        ui.set_validation_status(
                            format!("拖拽数据读取失败（0x{hresult:08X}）").into(),
                        );
                    }
                }
            }

            let (sender, receiver) = sync_channel(256);
            let stats = Rc::new(FileDropStats::default());
            let registration = Rc::new(RefCell::new(None::<DropRegistration>));
            let hover = Rc::new(RefCell::new(None::<(String, Instant)>));
            let drag_suppression = Rc::new(RefCell::new(None::<SuppressionToken>));
            let reported = Rc::new(Cell::new(false));
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            let machine = Rc::clone(&focus_machine);
            let registration_ref = Rc::clone(&registration);
            let hover_ref = Rc::clone(&hover);
            let suppression_ref = Rc::clone(&drag_suppression);
            let timer = Timer::default();
            timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                if registration_ref.borrow().is_none() {
                    match DropRegistration::register(
                        ui.window(),
                        receiving_target(sender.clone(), stats.clone()),
                    ) {
                        Ok(reg) => *registration_ref.borrow_mut() = Some(reg),
                        Err(error) => {
                            if !reported.replace(true) {
                                ui.set_validation_status(format!("拖拽注册失败：{error}").into());
                            }
                        }
                    }
                }
                while let Ok(event) = receiver.try_recv() {
                    handle_drop_event(
                        &ui,
                        &controller,
                        &weak,
                        &machine,
                        &suppression_ref,
                        &mut hover_ref.borrow_mut(),
                        event,
                    );
                }
            });
            _drop_timer_keepalive = Some(timer);
            _drop_registration_keepalive = Some(registration);
        }
        // P2-16 (baseline 15.7/17.4): historical backups. The dialog lists the
        // valid backups, backs the configuration up on demand and restores
        // the selected one; a restore reloads the library so the panel shows
        // the restored content immediately.
        let backup_paths: Rc<RefCell<Vec<std::path::PathBuf>>> = Rc::new(RefCell::new(Vec::new()));
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            let paths = Rc::clone(&backup_paths);
            ui.on_backup_menu_opened(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let entries = controller.borrow().backup_entries();
                let labels: Vec<slint::SharedString> = entries
                    .iter()
                    .map(|entry| entry.display_label().into())
                    .collect();
                *paths.borrow_mut() = entries.iter().map(|entry| entry.path.clone()).collect();
                let first = labels.first().cloned().unwrap_or_default();
                ui.set_backup_dialog_items(ModelRc::new(VecModel::from(labels)));
                ui.set_backup_dialog_selected(first);
                ui.set_backup_dialog_selected_index(if entries.is_empty() { -1 } else { 0 });
                ui.set_backup_dialog_status("".into());
                ui.set_backup_dialog_error(false);
                ui.set_backup_dialog_visible(true);
            });
        }
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            let paths = Rc::clone(&backup_paths);
            ui.on_backup_now_requested(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let result = controller.borrow().backup_now();
                let entries = controller.borrow().backup_entries();
                let labels: Vec<slint::SharedString> = entries
                    .iter()
                    .map(|entry| entry.display_label().into())
                    .collect();
                *paths.borrow_mut() = entries.iter().map(|entry| entry.path.clone()).collect();
                ui.set_backup_dialog_items(ModelRc::new(VecModel::from(labels.clone())));
                ui.set_backup_dialog_selected(labels.first().cloned().unwrap_or_default());
                ui.set_backup_dialog_selected_index(if labels.is_empty() { -1 } else { 0 });
                match result {
                    Ok(path) => {
                        let name = path
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        ui.set_backup_dialog_status(format!("已备份：{name}").into());
                        ui.set_backup_dialog_error(false);
                    }
                    Err(error) => {
                        ui.set_backup_dialog_status(format!("备份失败：{error}").into());
                        ui.set_backup_dialog_error(true);
                    }
                }
            });
        }
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            let paths = Rc::clone(&backup_paths);
            ui.on_backup_restore_requested(move |index: i32| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let Some(path) = usize::try_from(index)
                    .ok()
                    .and_then(|index| paths.borrow().get(index).cloned())
                else {
                    ui.set_backup_dialog_status("未选择备份".into());
                    ui.set_backup_dialog_error(true);
                    return;
                };
                let result = controller.borrow_mut().restore_backup(&path);
                match result {
                    Ok(message) => {
                        // Show the restored library right away.
                        refresh_ui(&controller, &weak);
                        let entries = controller.borrow().backup_entries();
                        let labels: Vec<slint::SharedString> = entries
                            .iter()
                            .map(|entry| entry.display_label().into())
                            .collect();
                        *paths.borrow_mut() =
                            entries.iter().map(|entry| entry.path.clone()).collect();
                        ui.set_backup_dialog_items(ModelRc::new(VecModel::from(labels)));
                        ui.set_backup_dialog_selected("".into());
                        ui.set_backup_dialog_selected_index(-1);
                        ui.set_backup_dialog_status(message.into());
                        ui.set_backup_dialog_error(false);
                    }
                    Err(error) => {
                        ui.set_backup_dialog_status(format!("恢复失败：{error}").into());
                        ui.set_backup_dialog_error(true);
                    }
                }
            });
        }
        {
            let weak = ui.as_weak();
            ui.on_backup_dialog_cancelled(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.set_backup_dialog_visible(false);
                }
            });
        }
        fn transfer_dialog_owner(ui: &MainWindow) -> isize {
            quicknest::infrastructure::windows::window_handle::inspect_window(ui.window())
                .map(|observation| observation.hwnd)
                .unwrap_or(0)
        }

        fn import_configuration(
            weak: &slint::Weak<MainWindow>,
            controller: &Rc<RefCell<quicknest::library_controller::LibraryController>>,
            mode: quicknest::config_transfer::ImportMode,
        ) {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let owner = transfer_dialog_owner(&ui);
            let Some(path) = quicknest::infrastructure::windows::dialogs::pick_file(owner) else {
                ui.set_transfer_dialog_status("已取消导入".into());
                ui.set_transfer_dialog_error(false);
                return;
            };
            let result = controller.borrow_mut().import_from(&path, mode);
            match result {
                Ok(report) => {
                    refresh_ui(controller, weak);
                    ui.set_transfer_dialog_status(
                        format!("{}：{}", report.summary(), path.display()).into(),
                    );
                    ui.set_transfer_dialog_error(false);
                }
                Err(error) => {
                    ui.set_transfer_dialog_status(format!("导入失败：{error}").into());
                    ui.set_transfer_dialog_error(true);
                }
            }
        }

        // P2-17 (baseline 17.4): configuration export and import. The file is
        // chosen with the native dialog; an import backs the live config up
        // before it applies anything (baseline 15.7).
        {
            let weak = ui.as_weak();
            ui.on_transfer_menu_opened(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.set_transfer_dialog_status("".into());
                    ui.set_transfer_dialog_error(false);
                    ui.set_transfer_dialog_visible(true);
                }
            });
        }
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            ui.on_transfer_export_all(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let owner = transfer_dialog_owner(&ui);
                let Some(path) = quicknest::infrastructure::windows::dialogs::save_file(
                    owner,
                    "quicknest-export.json",
                    "json",
                ) else {
                    ui.set_transfer_dialog_status("已取消导出".into());
                    ui.set_transfer_dialog_error(false);
                    return;
                };
                match controller.borrow().export_all_to(&path) {
                    Ok(bytes) => {
                        ui.set_transfer_dialog_status(
                            format!("已导出 {bytes} 字节到 {}", path.display()).into(),
                        );
                        ui.set_transfer_dialog_error(false);
                    }
                    Err(error) => {
                        ui.set_transfer_dialog_status(format!("导出失败：{error}").into());
                        ui.set_transfer_dialog_error(true);
                    }
                }
            });
        }
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            ui.on_transfer_export_group(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let Some(group_id) = controller.borrow().library().current_group_id.clone() else {
                    ui.set_transfer_dialog_status("当前没有分组".into());
                    ui.set_transfer_dialog_error(true);
                    return;
                };
                let owner = transfer_dialog_owner(&ui);
                let Some(path) = quicknest::infrastructure::windows::dialogs::save_file(
                    owner,
                    "quicknest-group.json",
                    "json",
                ) else {
                    ui.set_transfer_dialog_status("已取消导出".into());
                    ui.set_transfer_dialog_error(false);
                    return;
                };
                match controller.borrow().export_group_to(&path, &group_id) {
                    Ok(bytes) => {
                        ui.set_transfer_dialog_status(
                            format!("已导出当前分组（{bytes} 字节）到 {}", path.display()).into(),
                        );
                        ui.set_transfer_dialog_error(false);
                    }
                    Err(error) => {
                        ui.set_transfer_dialog_status(format!("导出失败：{error}").into());
                        ui.set_transfer_dialog_error(true);
                    }
                }
            });
        }
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            ui.on_transfer_merge_import(move || {
                import_configuration(
                    &weak,
                    &controller,
                    quicknest::config_transfer::ImportMode::Merge,
                );
            });
        }
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            ui.on_transfer_overwrite_import(move || {
                import_configuration(
                    &weak,
                    &controller,
                    quicknest::config_transfer::ImportMode::Overwrite,
                );
            });
        }
        {
            let weak = ui.as_weak();
            ui.on_transfer_dialog_cancelled(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.set_transfer_dialog_visible(false);
                }
            });
        }
        // P3-05/P3-06/P3-07: grid sort mode switching.
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            ui.on_sort_mode_selected(move |mode: slint::SharedString| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                controller.borrow_mut().set_sort_mode(mode.as_ref());
                refresh_ui(&controller, &weak);
                let label = match controller.borrow().sort_mode() {
                    "recent" => "最近使用",
                    "frequent" => "高频",
                    "favorite" => "收藏置顶",
                    _ => "手动",
                };
                ui.set_sort_mode_title(format!("排序：{label}").into());
                ui.set_validation_status(format!("排序模式：{label}").into());
            });
        }
        // UI 体验修复（2026-11-07）：底栏“视图”切换图标三档。
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            ui.on_view_size_selected(move |preset: slint::SharedString| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                controller.borrow_mut().set_icon_size(preset.as_ref());
                refresh_ui(&controller, &weak);
                let label = match controller.borrow().icon_size() {
                    quicknest::library_model::IconSize::Small => "小",
                    quicknest::library_model::IconSize::Large => "大",
                    quicknest::library_model::IconSize::Medium => "中",
                };
                ui.set_validation_status(format!("图标：{label}").into());
            });
        }
        // P3-09 (baseline 22 Everything 可选集成): settings-menu toggle.
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            ui.on_everything_toggle_requested(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let enabled = !controller.borrow().everything_enabled();
                controller.borrow_mut().set_everything_enabled(enabled);
                refresh_ui(&controller, &weak);
                ui.set_validation_status(if enabled {
                    "Everything 搜索集成：开".into()
                } else {
                    "Everything 搜索集成：关".into()
                });
            });
        }
        // 卡片规范 V1.0 §5: the badge hit test. The geometry lives here
        // so it can be unit-tested; the tip follows the pointer even when it
        // moves inside the card.
        {
            let weak = ui.as_weak();
            let owner: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
            ui.on_badge_tip_update(
                move |item_id: slint::SharedString,
                      reason: slint::SharedString,
                      path_missing: bool,
                      mouse_x: f32,
                      mouse_y: f32,
                      card_width: f32,
                      tip_x: f32,
                      tip_y: f32| {
                    let Some(ui) = weak.upgrade() else {
                        return false;
                    };
                    let on_badge =
                        quicknest::card_ui::badge_hit(path_missing, mouse_x, mouse_y, card_width);
                    let mut owner = owner.borrow_mut();
                    if on_badge {
                        *owner = item_id.to_string();
                        ui.set_hover_id("".into());
                        ui.set_hover_name("".into());
                        ui.set_badge_tooltip_visible(true);
                        ui.set_badge_tooltip_text(reason);
                        ui.set_badge_tooltip_x(tip_x);
                        ui.set_badge_tooltip_y(tip_y);
                    } else if owner.as_str() == item_id.as_str() {
                        owner.clear();
                        ui.set_badge_tooltip_visible(false);
                    }
                    on_badge
                },
            );
        }
        // P3-07: toggle the favorite flag on a card.
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            ui.on_item_menu_toggle_favorite(move |item_id: slint::SharedString| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let favorite = controller.borrow_mut().toggle_favorite(item_id.as_ref());
                refresh_ui(&controller, &weak);
                let name = controller
                    .borrow()
                    .item(item_id.as_ref())
                    .map(|item| item.name)
                    .unwrap_or_default();
                ui.set_validation_status(
                    if favorite {
                        format!("已收藏 {name}")
                    } else {
                        format!("已取消收藏 {name}")
                    }
                    .into(),
                );
            });
        }
        // P3-02 (baseline 22 启动组合): saved launch combinations. The card
        // menu saves the current batch selection under a name; the dialog
        // lists the saved combinations and launches or deletes them.
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            let refresh = move |ui: &MainWindow| {
                let rows = controller.borrow().combination_rows();
                let labels: Vec<slint::SharedString> = rows
                    .iter()
                    .map(|(_id, name, count)| format!("{name}（{count} 项）").into())
                    .collect();
                ui.set_combination_dialog_items(ModelRc::new(VecModel::from(labels)));
            };
            ui.on_combination_menu_opened(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                refresh(&ui);
                ui.set_combination_dialog_status("".into());
                ui.set_combination_dialog_error(false);
                ui.set_combination_dialog_visible(true);
            });
        }
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            ui.on_combination_save_opened(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let count = controller.borrow().batch_selection_ids().len();
                if count == 0 {
                    ui.set_combination_dialog_status("未选择项目".into());
                    ui.set_combination_dialog_error(true);
                    ui.set_combination_dialog_visible(true);
                    return;
                }
                ui.set_combination_dialog_visible(false);
                ui.set_combination_name_dialog_name(format!("组合 {}", count).into());
                ui.set_combination_dialog_status("".into());
                ui.set_combination_dialog_error(false);
                ui.set_combination_name_dialog_visible(true);
            });
        }
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            ui.on_combination_save_requested(move |name: slint::SharedString| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                // Scope the mutable borrow: the Ok arm reads the rows, and a
                // borrow held across the whole match expression would panic
                // the Slint callback (which aborts the process).
                let save_result = controller
                    .borrow_mut()
                    .save_batch_combination(name.as_ref());
                match save_result {
                    Ok((_id, saved_name)) => {
                        ui.set_combination_name_dialog_visible(false);
                        let rows = controller.borrow().combination_rows();
                        let labels: Vec<slint::SharedString> = rows
                            .iter()
                            .map(|(_id, name, count)| format!("{name}（{count} 项）").into())
                            .collect();
                        ui.set_combination_dialog_items(ModelRc::new(VecModel::from(labels)));
                        ui.set_combination_dialog_visible(true);
                        ui.set_combination_dialog_status(
                            format!("已保存组合“{saved_name}”").into(),
                        );
                        ui.set_combination_dialog_error(false);
                    }
                    Err(error) => {
                        ui.set_combination_dialog_status(format!("保存失败：{error}").into());
                        ui.set_combination_dialog_error(true);
                    }
                }
            });
        }
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            ui.on_combination_launch_requested(move |index: i32| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let rows = controller.borrow().combination_rows();
                let Some((_id, name, _count)) = usize::try_from(index)
                    .ok()
                    .and_then(|index| rows.get(index))
                else {
                    return;
                };
                let name = name.clone();
                let Some(item_ids) = controller.borrow().combination_item_ids(_id) else {
                    return;
                };
                let total = item_ids.len();
                if total == 0 {
                    ui.set_combination_dialog_status(
                        format!("组合“{name}”中没有可启动的项目").into(),
                    );
                    ui.set_combination_dialog_error(true);
                    return;
                }
                let mut started = 0_usize;
                let mut delayed = 0_usize;
                let mut failed = 0_usize;
                let mut skipped = 0_usize;
                for item_id in &item_ids {
                    // P3-08 (baseline 8.3/8.4): unverified command items are
                    // skipped and reported, never run silently.
                    let unverified = controller.borrow().item(item_id).is_some_and(|item| {
                        item.item_type == quicknest::library_model::ItemKind::Command
                            && item.launch.confirm_on_first_run
                    });
                    if unverified {
                        skipped += 1;
                        continue;
                    }
                    let (launched, delay) = launch_item_maybe_delayed(&controller, item_id, false);
                    if launched {
                        if delay > 0 {
                            delayed += 1;
                        } else {
                            started += 1;
                            controller.borrow_mut().record_launch(item_id);
                        }
                    } else {
                        failed += 1;
                    }
                }
                refresh_ui(&controller, &weak);
                let mut summary = format!("组合“{name}”：{started}/{total} 个项目已启动");
                if delayed > 0 {
                    summary.push_str(&format!("，{delayed} 个延时启动"));
                }
                if skipped > 0 {
                    summary.push_str(&format!("，跳过 {skipped} 个未确认命令"));
                }
                ui.set_combination_dialog_status(summary.into());
                ui.set_combination_dialog_error(failed > 0);
            });
        }
        {
            let weak = ui.as_weak();
            let controller = Rc::clone(&controller);
            ui.on_combination_delete_requested(move |index: i32| {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let rows = controller.borrow().combination_rows();
                let Some((id, name, _)) = usize::try_from(index)
                    .ok()
                    .and_then(|index| rows.get(index))
                else {
                    return;
                };
                let name = name.clone();
                let _ = controller.borrow_mut().delete_combination(id);
                let rows = controller.borrow().combination_rows();
                let labels: Vec<slint::SharedString> = rows
                    .iter()
                    .map(|(_id, name, count)| format!("{name}（{count} 项）").into())
                    .collect();
                ui.set_combination_dialog_items(ModelRc::new(VecModel::from(labels)));
                ui.set_combination_dialog_status(format!("已删除组合“{name}”").into());
                ui.set_combination_dialog_error(false);
            });
        }
        {
            let weak = ui.as_weak();
            ui.on_combination_dialog_cancelled(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.set_combination_dialog_visible(false);
                    ui.set_combination_name_dialog_visible(false);
                }
            });
        }
    }

    let restore_timer = Rc::new(Timer::default());
    let weak = ui.as_weak();
    let restore = restore_timer.clone();
    let interactive_mode = mode == RunMode::Interactive;
    let hide_machine = Rc::clone(&focus_machine);
    ui.on_hide_requested(move || {
        if let Some(ui) = weak.upgrade() {
            if interactive_mode {
                // Baseline 2.5: the close button hides the panel and keeps it
                // hidden; the global hotkey (P1-14) or the tray (P1-15)
                // brings it back.
                hide_panel(&ui, &hide_machine);
                ui.set_validation_status("已隐藏（Ctrl+Alt+Space 呼出）".into());
                return;
            }
            if let Err(error) = ui.hide() {
                ui.set_validation_status(format!("隐藏失败：{error}").into());
                return;
            }
            let weak = ui.as_weak();
            restore.start(TimerMode::SingleShot, Duration::from_secs(2), move || {
                if let Some(ui) = weak.upgrade() {
                    match ui.show() {
                        Ok(()) => ui.set_validation_status("已隐藏并恢复同一窗口".into()),
                        Err(error) => {
                            eprintln!("Failed to restore validation window: {error}");
                            std::process::exit(1);
                        }
                    }
                }
            });
        }
    });
    ui.on_exit_requested(|| {
        if let Err(error) = slint::quit_event_loop() {
            eprintln!("Failed to exit validation loop: {error}");
            std::process::exit(1);
        }
    });
    // P1-18: the title-bar "固定" toggle now owns the real pinned state
    // (baseline 10.7); it is persisted and mirrored into the state machine.
    {
        let weak = ui.as_weak();
        let controller = Rc::clone(&controller);
        let machine = Rc::clone(&focus_machine);
        ui.on_title_pin_toggled(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let next = !controller.borrow().library().settings.pinned;
            controller.borrow_mut().library_mut().settings.pinned = next;
            let _ = controller.borrow().persist();
            machine.borrow_mut().set_pinned(next);
            ui.set_pin_toggled(next);
            ui.set_validation_status(if next {
                "已固定（不自动隐藏）".into()
            } else {
                "已取消固定（失焦自动隐藏）".into()
            });
        });
    }
    // P2-12: theme + accent menu (title-bar "设置"). Both apply immediately
    // and persist; the system mode is polled by the timer above.
    {
        let weak = ui.as_weak();
        let controller = Rc::clone(&controller);
        ui.on_theme_menu_selected(move |mode: slint::SharedString| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let parsed = quicknest::theme::ThemeMode::parse(mode.as_ref());
            controller.borrow_mut().library_mut().settings.theme = parsed.as_str().to_string();
            let _ = controller.borrow().persist();
            apply_theme(&ui, &controller);
            let label = match parsed {
                quicknest::theme::ThemeMode::Light => "浅色",
                quicknest::theme::ThemeMode::Dark => "深色",
                quicknest::theme::ThemeMode::System => "跟随系统",
            };
            ui.set_validation_status(format!("主题：{label}").into());
        });
    }
    {
        let weak = ui.as_weak();
        let controller = Rc::clone(&controller);
        ui.on_accent_menu_selected(move |color: slint::SharedString| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let accent = quicknest::theme::normalize_accent(color.as_ref());
            controller.borrow_mut().library_mut().settings.accent_color = accent.clone();
            let _ = controller.borrow().persist();
            apply_theme(&ui, &controller);
            ui.set_validation_status(format!("强调色：{accent}").into());
        });
    }
    // P2-13 (baseline 16.2): opacity presets + rounded-corner toggle. Both
    // apply immediately through apply_theme and persist.
    {
        let weak = ui.as_weak();
        let controller = Rc::clone(&controller);
        ui.on_opacity_menu_selected(move |value: slint::SharedString| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let parsed: f64 = value.to_string().parse().unwrap_or(0.95);
            let opacity = quicknest::library_model::normalize_opacity(parsed);
            controller.borrow_mut().library_mut().settings.opacity = opacity;
            let _ = controller.borrow().persist();
            apply_theme(&ui, &controller);
            ui.set_validation_status(
                format!("透明度：{}%", (opacity * 100.0).round() as i32).into(),
            );
        });
    }
    {
        let weak = ui.as_weak();
        let controller = Rc::clone(&controller);
        ui.on_corners_menu_selected(move |value: slint::SharedString| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let rounded = &*value == "on";
            controller
                .borrow_mut()
                .library_mut()
                .settings
                .rounded_corners = rounded;
            let _ = controller.borrow().persist();
            apply_theme(&ui, &controller);
            ui.set_validation_status(if rounded {
                "圆角：开".into()
            } else {
                "圆角：关".into()
            });
        });
    }
    // P2-14 (baseline 16.3): DWM system backdrop selection; applies and
    // persists immediately through apply_theme.
    {
        let weak = ui.as_weak();
        let controller = Rc::clone(&controller);
        ui.on_backdrop_menu_selected(move |value: slint::SharedString| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let backdrop = quicknest::theme::normalize_backdrop(value.as_ref());
            controller.borrow_mut().library_mut().settings.backdrop = backdrop.to_string();
            let _ = controller.borrow().persist();
            apply_theme(&ui, &controller);
            ui.set_validation_status(format!("背景：{backdrop}").into());
        });
    }
    ui.window().on_close_requested(|| {
        if let Err(error) = slint::quit_event_loop() {
            eprintln!("Failed to close validation window: {error}");
            std::process::exit(1);
        }
        slint::CloseRequestResponse::HideWindow
    });

    let mut _hot_zone_timer_keepalive: Option<Timer> = None;
    let mut _focus_timer_keepalive: Option<Timer> = None;
    ui.show()?;
    // P2-13: the layered-window style can only be applied once the winit
    // window is live inside the event loop (before run_event_loop the raw
    // handle is not exposed), so refresh the appearance on a short timer.
    // P2-13: the layered-window style can only be applied once the winit
    // window is live inside the event loop (before run_event_loop the raw
    // handle is not exposed), so refresh the appearance on a short timer.
    // P2-15 restores the remembered position in the same tick for the same
    // reason: the native window does not exist before the event loop runs.
    let interactive_restore = mode == RunMode::Interactive;
    let _appearance_timer_keepalive: Option<Timer> = {
        let weak = ui.as_weak();
        let controller = Rc::clone(&controller);
        let timer = Timer::default();
        timer.start(
            TimerMode::SingleShot,
            Duration::from_millis(150),
            move || {
                if let Some(ui) = weak.upgrade() {
                    apply_theme(&ui, &controller);
                    #[cfg(windows)]
                    if interactive_restore {
                        restore_window_placement(&ui, &controller);
                    }
                }
            },
        );
        Some(timer)
    };
    // P3-04 (baseline 22 鼠标热区): hovering the configured screen edge for
    // ~600ms toggles the panel. Only in interactive mode; requires the cursor
    // to leave the edge before the next trigger, so a parked cursor cannot
    // oscillate the panel.
    if mode == RunMode::Interactive {
        let hot_zone_enabled = controller.borrow().library().settings.hot_zone_enabled;
        if hot_zone_enabled {
            let weak = ui.as_weak();
            let machine = Rc::clone(&focus_machine);
            let controller = Rc::clone(&controller);
            let at_edge_since: Rc<RefCell<Option<Instant>>> = Rc::new(RefCell::new(None));
            let triggered: Rc<Cell<bool>> = Rc::new(Cell::new(false));
            let timer = Timer::default();
            timer.start(TimerMode::Repeated, Duration::from_millis(200), move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let (enabled, edge) = {
                    let library = controller.borrow();
                    let enabled = library.library().settings.hot_zone_enabled;
                    // normalize returns a &'static str, so it outlives the borrow.
                    let edge = quicknest::library_model::normalize_hot_zone_edge(
                        &library.library().settings.hot_zone_edge,
                    );
                    (enabled, edge)
                };
                if !enabled {
                    return;
                }
                let Some((x, y)) = quicknest::infrastructure::windows::launch::cursor_position()
                else {
                    return;
                };
                let Some(bounds) = quicknest::infrastructure::windows::placement::primary_bounds()
                else {
                    return;
                };
                // A few pixels inside the edge count as "on the edge".
                const EDGE_BAND: i32 = 2;
                let at_edge = match edge {
                    "right" => x >= bounds.right - EDGE_BAND,
                    "top" => y <= bounds.top + EDGE_BAND,
                    "bottom" => y >= bounds.bottom - EDGE_BAND,
                    _ => x <= bounds.left + EDGE_BAND,
                };
                let mut dwell = at_edge_since.borrow_mut();
                if !at_edge {
                    *dwell = None;
                    triggered.set(false);
                    return;
                }
                let Some(since) = *dwell else {
                    *dwell = Some(Instant::now());
                    return;
                };
                if triggered.get() || since.elapsed() < Duration::from_millis(600) {
                    return;
                }
                // Dwell satisfied: toggle the panel once for this visit.
                triggered.set(true);
                let visible =
                    machine.borrow().state() == quicknest::window_state::PanelState::Visible;
                if visible {
                    hide_panel(&ui, &machine);
                } else {
                    show_panel(&ui, &machine);
                }
            });
            _hot_zone_timer_keepalive = Some(timer);
        }
    }
    // P1-18: blur/auto-hide polling (baseline 10.3). Runs only in interactive
    // mode; validation modes keep their deterministic show/hide scripts.
    if mode == RunMode::Interactive {
        let _ = focus_machine.borrow_mut().show();
        let machine = Rc::clone(&focus_machine);
        let dialogs = Rc::clone(&dialog_suppression);
        let weak = ui.as_weak();
        let pending: Rc<RefCell<Option<Instant>>> = Rc::new(RefCell::new(None));
        let hwnd_slot: Rc<Cell<isize>> = Rc::new(Cell::new(0));
        // Startup guard: a launch from a terminal/launcher may not receive the
        // foreground immediately. Auto-hide only arms after the panel has been
        // foreground once, so it cannot vanish before the user ever sees it.
        let ever_foreground = Rc::new(Cell::new(false));
        let timer = Timer::default();
        timer.start(TimerMode::Repeated, Duration::from_millis(60), move || {
            use quicknest::window_state::{
                BlurObservation, BlurRecheck, PanelState, SuppressionReason,
            };
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let visible = machine.borrow().state() == PanelState::Visible;
            if !visible {
                *pending.borrow_mut() = None;
                return;
            }
            // Baseline 10.5: visible modal dialogs hold a suppression lock.
            let dialogs_open = ui.get_any_modal_open();
            {
                let mut token = dialogs.borrow_mut();
                if dialogs_open {
                    if token.is_none() {
                        *token = Some(machine.borrow().acquire(SuppressionReason::ModalDialogOpen));
                    }
                } else {
                    token.take();
                }
            }
            let hwnd = if hwnd_slot.get() == 0 {
                match quicknest::infrastructure::windows::window_handle::inspect_window(ui.window())
                {
                    Ok(observed) => {
                        hwnd_slot.set(observed.hwnd);
                        observed.hwnd
                    }
                    Err(_) => return,
                }
            } else {
                hwnd_slot.get()
            };
            // SAFETY: the HWND was returned by inspect_window for this window;
            // the PID is our own process.
            let ours = unsafe {
                quicknest::infrastructure::windows::focus::foreground_belongs_to(
                    hwnd as *mut core::ffi::c_void,
                    std::process::id(),
                )
            };
            machine.borrow_mut().set_foreground_hint(ours);
            if ours {
                ever_foreground.set(true);
            }
            if !ever_foreground.get() {
                *pending.borrow_mut() = None;
                return;
            }
            let now = Instant::now();
            let due = matches!(*pending.borrow(), Some(deadline) if now >= deadline);
            if due {
                *pending.borrow_mut() = None;
            }
            if due {
                let hide = machine.borrow().evaluate_blur_recheck(ours) == BlurRecheck::Hide;
                if hide {
                    hide_panel(&ui, &machine);
                }
            } else {
                // Baseline 10.3: schedule the single 120 ms re-check once and
                // never postpone it by later ticks.
                let can_start = pending.borrow().is_none();
                let observation = if can_start {
                    machine.borrow().observe_blur()
                } else {
                    BlurObservation::Ignore
                };
                if observation == BlurObservation::StartDelayedCheck {
                    *pending.borrow_mut() = Some(now + Duration::from_millis(120));
                }
            }
        });
        _focus_timer_keepalive = Some(timer);
    }
    // P2-15 (baseline 15.7/16.4): remember where the user left the panel.
    // The poll interval is the debounce window (300--500ms) the baseline
    // asks for, so a drag writes the position once it settles.
    let _position_timer_keepalive: Option<Timer> = {
        let weak = ui.as_weak();
        let controller = Rc::clone(&controller);
        let timer = Timer::default();
        if mode == RunMode::Interactive {
            timer.start(TimerMode::Repeated, Duration::from_millis(400), move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let position = ui.window().position();
                let size = ui.window().size();
                let width = i32::try_from(size.width).unwrap_or(i32::MAX);
                let height = i32::try_from(size.height).unwrap_or(i32::MAX);
                let changed = {
                    let mut borrowed = controller.borrow_mut();
                    let current = borrowed.library().settings.window_bounds.clone();
                    let unchanged = current.as_ref().is_some_and(|bounds| {
                        bounds.x == position.x
                            && bounds.y == position.y
                            && bounds.width == width
                            && bounds.height == height
                    });
                    if unchanged {
                        false
                    } else {
                        let mut bounds = current.unwrap_or_default();
                        bounds.x = position.x;
                        bounds.y = position.y;
                        bounds.width = width;
                        bounds.height = height;
                        #[cfg(windows)]
                        if let Some(device) = current_monitor_device(&ui) {
                            bounds.monitor_id = device;
                        }
                        borrowed.library_mut().settings.window_bounds = Some(bounds);
                        true
                    }
                };
                if changed {
                    let _ = controller.borrow().persist();
                }
            });
        }
        Some(timer)
    };
    let smoke_timer = Timer::default();
    if mode == RunMode::SmokeTest {
        ui.set_validation_status("自动验证：3 轮隐藏/恢复，完成后退出".into());
        let step = Cell::new(0_u8);
        let weak = ui.as_weak();
        smoke_timer.start(TimerMode::Repeated, Duration::from_secs(2), move || {
            let current = step.get() + 1;
            step.set(current);
            if current > 6 {
                if let Err(error) = slint::quit_event_loop() {
                    eprintln!("Failed to finish validation: {error}");
                    std::process::exit(1);
                }
                return;
            }
            let result = if let Some(ui) = weak.upgrade() {
                if current % 2 == 1 {
                    ui.hide()
                } else {
                    ui.show()
                }
            } else {
                eprintln!("Validation window was destroyed unexpectedly");
                std::process::exit(1);
            };
            if let Err(error) = result {
                eprintln!("Display/hide validation failed at step {current}: {error}");
                std::process::exit(1);
            }
        });
    }
    let event_loop_result = slint::run_event_loop_until_quit();
    delayed_tasks().cancel_pending();
    #[cfg(windows)]
    UPDATE_UI.with(|state| {
        state
            .borrow()
            .cancel
            .store(true, std::sync::atomic::Ordering::SeqCst)
    });
    event_loop_result?;
    Ok(())
}

#[cfg(all(test, windows))]
mod update_ui_tests {
    use super::UpdateUiState;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn cancelled_queued_result_cannot_finish_a_check() {
        let mut state = UpdateUiState {
            busy: true,
            ..Default::default()
        };
        let token = Arc::clone(&state.cancel);
        token.store(true, Ordering::SeqCst);
        assert!(!state.finish_check(&token));
        assert!(state.busy);
        assert!(state.pending.is_none());
    }

    #[test]
    fn stale_queued_result_cannot_finish_a_new_check() {
        let mut state = UpdateUiState {
            busy: true,
            ..Default::default()
        };
        let old = Arc::clone(&state.cancel);
        state.cancel = Arc::new(AtomicBool::new(false));
        assert!(!state.finish_check(&old));
        assert!(state.busy);
        assert!(state.pending.is_none());
    }

    #[test]
    fn active_result_finishes_its_own_check() {
        let mut state = UpdateUiState {
            busy: true,
            ..Default::default()
        };
        let token = Arc::clone(&state.cancel);
        assert!(state.finish_check(&token));
        assert!(!state.busy);
    }
}
