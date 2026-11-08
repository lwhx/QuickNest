use super::*;

type PendingId = Rc<RefCell<Option<String>>>;

#[derive(Clone, Default)]
pub(super) struct Pending {
    pub editing_item: PendingId,
    pub item_delete: PendingId,
    pub item_move: PendingId,
    pub command: Rc<RefCell<Option<(String, bool)>>>,
    pub group_target: PendingId,
    pub group_delete: PendingId,
    pub category_target: PendingId,
}

pub(super) fn register(ui: &MainWindow, machine: &SharedFocusMachine, pending: &Pending) {
    let weak = ui.as_weak();
    let state = pending.editing_item.clone();
    ui.on_item_dialog_cancelled(move || {
        state.borrow_mut().take();
        if let Some(ui) = weak.upgrade() {
            ui.set_item_dialog_visible(false);
        }
    });
    let weak = ui.as_weak();
    let state = pending.item_delete.clone();
    ui.on_item_delete_confirm_cancelled(move || {
        state.borrow_mut().take();
        if let Some(ui) = weak.upgrade() {
            ui.set_item_delete_confirm_visible(false);
        }
    });
    let weak = ui.as_weak();
    let state = pending.item_move.clone();
    ui.on_move_dialog_cancelled(move || {
        state.borrow_mut().take();
        if let Some(ui) = weak.upgrade() {
            ui.set_move_dialog_visible(false);
        }
    });
    let weak = ui.as_weak();
    let state = pending.command.clone();
    ui.on_command_confirm_cancelled(move || {
        state.borrow_mut().take();
        if let Some(ui) = weak.upgrade() {
            ui.set_command_confirm_visible(false);
            ui.set_validation_status("已取消执行".into());
        }
    });
    let weak = ui.as_weak();
    let state = pending.group_target.clone();
    ui.on_group_dialog_cancelled(move || {
        state.borrow_mut().take();
        if let Some(ui) = weak.upgrade() {
            ui.set_group_dialog_visible(false);
        }
    });
    let weak = ui.as_weak();
    let state = pending.group_delete.clone();
    ui.on_group_delete_confirm_cancel(move || {
        state.borrow_mut().take();
        if let Some(ui) = weak.upgrade() {
            ui.set_group_delete_confirm_visible(false);
        }
    });
    let weak = ui.as_weak();
    let state = pending.category_target.clone();
    ui.on_category_dialog_cancelled(move || {
        state.borrow_mut().take();
        if let Some(ui) = weak.upgrade() {
            ui.set_category_dialog_visible(false);
        }
    });
    let weak = ui.as_weak();
    let state = pending.category_target.clone();
    ui.on_category_delete_confirm_cancel(move || {
        state.borrow_mut().take();
        if let Some(ui) = weak.upgrade() {
            ui.set_category_delete_confirm_visible(false);
        }
    });
    let weak = ui.as_weak();
    ui.on_backup_dialog_cancelled(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_backup_dialog_visible(false);
        }
    });
    let weak = ui.as_weak();
    ui.on_transfer_dialog_cancelled(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_transfer_dialog_visible(false);
        }
    });
    let weak = ui.as_weak();
    ui.on_update_install_cancelled(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_update_confirm_visible(false);
        }
    });
    let weak = ui.as_weak();
    ui.on_combination_dialog_cancelled(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_combination_dialog_visible(false);
            ui.set_combination_name_dialog_visible(false);
        }
    });
    let weak = ui.as_weak();
    ui.on_search_dismissed(move || {
        if let Some(ui) = weak.upgrade() {
            if ui.get_search_query().is_empty() {
                ui.set_search_visible(false);
            } else {
                ui.set_search_query("".into());
                ui.invoke_search_query_changed("".into());
            }
        }
    });

    let weak = ui.as_weak();
    let machine = machine.clone();
    ui.on_escape_pressed(move || {
        let Some(ui) = weak.upgrade() else { return };
        if ui.get_settings_panel_visible() {
            ui.invoke_settings_panel_closed();
        } else if ui.get_command_confirm_visible() {
            ui.invoke_command_confirm_cancelled();
        } else if ui.get_update_confirm_visible() {
            ui.invoke_update_install_cancelled();
        } else if ui.get_combination_name_dialog_visible() || ui.get_combination_dialog_visible() {
            ui.invoke_combination_dialog_cancelled();
        } else if ui.get_transfer_dialog_visible() {
            ui.invoke_transfer_dialog_cancelled();
        } else if ui.get_backup_dialog_visible() {
            ui.invoke_backup_dialog_cancelled();
        } else if ui.get_item_dialog_visible() {
            ui.invoke_item_dialog_cancelled();
        } else if ui.get_item_delete_confirm_visible() {
            ui.invoke_item_delete_confirm_cancelled();
        } else if ui.get_move_dialog_visible() {
            ui.invoke_move_dialog_cancelled();
        } else if ui.get_group_dialog_visible() {
            ui.invoke_group_dialog_cancelled();
        } else if ui.get_group_delete_confirm_visible() {
            ui.invoke_group_delete_confirm_cancel();
        } else if ui.get_category_dialog_visible() {
            ui.invoke_category_dialog_cancelled();
        } else if ui.get_category_delete_confirm_visible() {
            ui.invoke_category_delete_confirm_cancel();
        } else if ui.get_search_visible() {
            ui.invoke_search_dismissed();
        } else {
            hide_panel(&ui, &machine);
        }
    });
}
