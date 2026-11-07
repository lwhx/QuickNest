//! P1-02 group controller: bridges the library model to the slint UI and
//! persists changes through the atomic save chain (P0-13).

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use crate::infrastructure::windows::config_save::{
    SaveOutcome, prune_backups, recover_latest_backup, save_atomic,
};
use crate::library_model::{DeleteOutcome, IconSize, Item, ItemDraft, ItemKind, Library, new_uuid};
use crate::pinyin_index::PinyinIndex;

/// A group row for the UI. The slint view type is generated in the binary
/// crate, so the library stays free of slint types.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupRow {
    pub id: String,
    pub name: String,
    pub color: String,
    pub active: bool,
    pub item_count: usize,
    /// Tab width in pixels, estimated (slint 1.18 has no string measurement).
    pub tab_width: f32,
}

/// A category row for the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct CategoryRow {
    pub id: String,
    pub name: String,
    pub item_count: usize,
}

/// A project-grid row (baseline 2.4). Carries only what the grid renders and
/// the hover tooltip needs; launching/editing belongs to P1-05/P1-06.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemRow {
    pub id: String,
    pub name: String,
    /// P3-07: favorites sort to the top in favorite mode.
    pub favorite: bool,
    /// P3-06: launch count for the tooltip.
    pub launch_count: u64,
    /// P3-05: unix millis of the last launch (0 = never).
    pub last_used: i64,
    /// Launch target shown in the tooltip (empty for commands/automations).
    pub path: String,
    pub note: String,
    /// Human-readable kind label (baseline 3.1).
    pub item_type: String,
    /// Red "!" badge when the file-like launch target no longer exists.
    pub path_missing: bool,
    /// 卡片规范 V1.0 §5: the concrete reason behind the badge (hover tip).
    pub missing_reason: String,
    /// 卡片规范 V1.0 §2: the default artwork for items without a native
    /// icon — app (EXE/file/folder), browser (URL) or terminal (command).
    pub artwork: String,
    /// File whose icon represents the item (P1-10); None uses the default.
    pub icon_source: Option<String>,
}

/// A search result row (P2-01, baseline 13.1/13.2).
#[derive(Debug, Clone, PartialEq)]
pub struct SearchRow {
    pub id: String,
    pub name: String,
    pub path: String,
    /// "分组 · 分类", or the group alone when the category set is empty.
    pub scope: String,
    /// Human-readable kind label (baseline 3.1).
    pub type_label: String,
}

/// A delete request that needs the user's decision (baseline 2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingDelete {
    pub group_id: String,
    pub group_name: String,
    pub content_summary: String,
    pub item_count: usize,
    pub category_count: usize,
}

/// What happened when the library was loaded (baseline 15.8/15.9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadState {
    /// Normal load, or first run when the file was missing.
    Loaded,
    /// The live config was corrupt; a backup was restored and the corrupt
    /// file was renamed out of the way.
    Recovered {
        backup: PathBuf,
        corrupted: Option<PathBuf>,
    },
    /// The config cannot be used (schema mismatch or no valid backup). The
    /// original file is left untouched and automatic writes are disabled.
    SafeMode { reason: String },
}

/// The slint-facing view of the library.
pub struct LibraryController {
    library: Library,
    /// Where the library is persisted; None disables persistence (tests).
    config_path: Option<PathBuf>,
    load_state: LoadState,
    /// The last save failure, shown by the UI instead of failing silently.
    save_error: RefCell<Option<String>>,
    /// P2-02 pinyin index cache keyed by item id; the stored name detects
    /// renames so the index is rebuilt (baseline 13.3).
    pinyin_cache: RefCell<HashMap<String, (String, PinyinIndex)>>,
    /// P3-01: the items picked for a batch launch. Transient UI state; it is
    /// never part of the persisted library.
    batch_selection: RefCell<Vec<String>>,
}

impl LibraryController {
    /// Loads the library from `config_path` (baseline 15.8/15.9): a valid
    /// version-2 file loads normally; a corrupt file triggers backup recovery;
    /// an unsupported schema version or a corrupt file without a valid backup
    /// enters safe mode, leaving the original file untouched.
    ///
    /// A UTF-8 BOM is stripped first: editors and PowerShell 5.1 write it on
    /// Windows, and `serde_json` rejects it.
    pub fn load(config_path: Option<PathBuf>) -> Self {
        let Some(path) = config_path.clone() else {
            return Self::loaded(sample_library(), config_path, LoadState::Loaded);
        };
        match std::fs::read_to_string(&path) {
            Ok(content) => match parse_library(&content) {
                Some(library) if library.schema_version == 2 => {
                    Self::loaded(library, config_path, LoadState::Loaded)
                }
                Some(library) => Self::safe_mode(
                    config_path,
                    format!(
                        "配置版本 {} 与当前版本 2 不兼容，未写入",
                        library.schema_version
                    ),
                ),
                None => Self::recover(&path, config_path),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Self::loaded(sample_library(), config_path, LoadState::Loaded)
            }
            Err(error) => Self::safe_mode(config_path, format!("配置读取失败：{error}")),
        }
    }

    fn loaded(library: Library, config_path: Option<PathBuf>, state: LoadState) -> Self {
        Self {
            library,
            config_path,
            load_state: state,
            save_error: RefCell::new(None),
            pinyin_cache: RefCell::new(HashMap::new()),
            batch_selection: RefCell::new(Vec::new()),
        }
    }

    fn safe_mode(config_path: Option<PathBuf>, reason: String) -> Self {
        Self::loaded(
            sample_library(),
            config_path,
            LoadState::SafeMode { reason },
        )
    }

    fn recover(path: &std::path::Path, config_path: Option<PathBuf>) -> Self {
        match recover_latest_backup(path) {
            Ok(report) => {
                let Some(backup) = report.restored_from else {
                    return Self::safe_mode(
                        config_path,
                        "配置损坏且没有有效备份，未写入".to_string(),
                    );
                };
                let restored = std::fs::read_to_string(path)
                    .ok()
                    .as_deref()
                    .and_then(parse_library);
                match restored {
                    Some(library) if library.schema_version == 2 => Self::loaded(
                        library,
                        config_path,
                        LoadState::Recovered {
                            backup,
                            corrupted: report.corrupted_path,
                        },
                    ),
                    _ => Self::safe_mode(config_path, "恢复的备份也无法使用，未写入".to_string()),
                }
            }
            Err(error) => Self::safe_mode(config_path, format!("配置恢复失败：{error}")),
        }
    }

    pub fn load_state(&self) -> &LoadState {
        &self.load_state
    }

    pub fn save_error(&self) -> Option<String> {
        self.save_error.borrow().clone()
    }

    pub fn library(&self) -> &Library {
        &self.library
    }

    pub fn library_mut(&mut self) -> &mut Library {
        &mut self.library
    }

    /// The group rows for the group bar.
    pub fn group_rows(&self) -> Vec<GroupRow> {
        let active_id = self.library.active_group_id().map(str::to_string);
        self.library
            .groups
            .iter()
            .map(|group| GroupRow {
                id: group.id.clone(),
                name: group.name.clone(),
                color: group.color.clone(),
                active: Some(group.id.as_str()) == active_id.as_deref(),
                item_count: group.item_count(),
                // Slint 1.18 cannot measure strings; estimate the CJK-aware
                // tab width here (CJK glyphs are about twice as wide).
                tab_width: tab_width_px(&group.name),
            })
            .collect()
    }

    /// The category rows of the active group (baseline 2.3).
    pub fn category_rows(&self) -> Vec<CategoryRow> {
        self.library
            .active_group()
            .map(|group| {
                group
                    .categories
                    .iter()
                    .map(|category| CategoryRow {
                        id: category.id.clone(),
                        name: category.name.clone(),
                        item_count: category.items.len(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The items of the active category for the project grid (baseline 2.4).
    pub fn item_rows(&self) -> Vec<ItemRow> {
        // P3-05/P3-06/P3-07: the grid order follows the active sort mode
        // (manual keeps the stored order).
        let mode = self.sort_mode();
        let ordered_ids = {
            let ids: Vec<String> = self
                .library
                .active_category()
                .map(|category| category.items.iter().map(|item| item.id.clone()).collect())
                .unwrap_or_default();
            let usage: Vec<crate::library_model::UsageSnapshot> = self
                .library
                .active_category()
                .map(|category| {
                    category
                        .items
                        .iter()
                        .map(|item| crate::library_model::UsageSnapshot {
                            id: item.id.clone(),
                            name: item.name.clone(),
                            last_used: item.last_used,
                            launch_count: item.launch_count,
                            favorite: item.favorite,
                        })
                        .collect()
                })
                .unwrap_or_default();
            crate::library_model::sort_item_ids_by_mode(&ids, &usage, mode)
        };
        let mut rows: Vec<ItemRow> = self
            .library
            .active_category()
            .map(|category| {
                category
                    .items
                    .iter()
                    .map(|item| ItemRow {
                        id: item.id.clone(),
                        name: item.name.clone(),
                        path: item.launch.path.clone(),
                        note: item.note.clone(),
                        item_type: item.item_type.label().to_string(),
                        path_missing: item.path_missing(),
                        missing_reason: item.missing_reason(),
                        artwork: item.default_artwork().to_string(),
                        icon_source: item.icon_source(),
                        favorite: item.favorite,
                        launch_count: item.launch_count,
                        last_used: item.last_used,
                    })
                    .collect()
            })
            .unwrap_or_default();
        // P3-05/P3-06/P3-07: reorder the rows by the active sort mode (the
        // usage snapshot was taken before this borrow).
        rows.sort_by_key(|row| {
            ordered_ids
                .iter()
                .position(|id| id == &row.id)
                .unwrap_or(usize::MAX)
        });
        rows
    }

    /// The icon preset from `settings.icon_size` (baseline 2.4, default 中).
    pub fn icon_size(&self) -> IconSize {
        IconSize::from_settings(&self.library.settings.icon_size)
    }

    /// UI 体验修复（2026-11-07 底栏“视图”）：切换图标三档并持久化；
    /// 非法值按 IconSize 规则归一化后存储。
    pub fn set_icon_size(&mut self, preset: &str) {
        let previous = self.library.clone();
        self.library.settings.icon_size = IconSize::from_settings(preset).key().to_string();
        let _ = self.persist_change(previous);
    }

    /// Searches registered items (P2-01, baseline 13.1/13.2). The default
    /// scope is the active group; `all_groups` widens it to the whole library.
    /// Results are ordered by match score, then by name.
    pub fn search_items(&self, query: &str, all_groups: bool) -> Vec<SearchRow> {
        let active_group = self.library.active_group_id();
        let mut scored: Vec<(u32, SearchRow)> = Vec::new();
        for group in &self.library.groups {
            if !all_groups && active_group != Some(group.id.as_str()) {
                continue;
            }
            for category in &group.categories {
                for item in &category.items {
                    // P2-02: build once per item name; a rename changes the
                    // stored name and rebuilds the pinyin index.
                    let index = {
                        let mut cache = self.pinyin_cache.borrow_mut();
                        match cache.get(&item.id) {
                            Some((name, index)) if name == &item.name => index.clone(),
                            _ => {
                                let index = PinyinIndex::build(&item.name);
                                cache.insert(item.id.clone(), (item.name.clone(), index.clone()));
                                index
                            }
                        }
                    };
                    let Some(score) = crate::search::score_with_pinyin(
                        query,
                        &item.name,
                        &item.launch.path,
                        Some(&index),
                    ) else {
                        continue;
                    };
                    let scope = if category.name.is_empty() {
                        group.name.clone()
                    } else {
                        format!("{} · {}", group.name, category.name)
                    };
                    scored.push((
                        score,
                        SearchRow {
                            id: item.id.clone(),
                            name: item.name.clone(),
                            path: item.launch.path.clone(),
                            scope,
                            type_label: item.item_type.label().to_string(),
                        },
                    ));
                }
            }
        }
        scored.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.name.cmp(&right.1.name))
        });
        scored.into_iter().map(|(_, row)| row).collect()
    }

    /// Selects a group (baseline 2.2) and reports whether it changed.
    pub fn select_group(&mut self, group_id: &str) -> bool {
        let previous = self.library.clone();
        let changed = self.library.select_group(group_id);
        if changed {
            return self.persist_change(previous).is_ok();
        }
        changed
    }

    /// Selects a category inside the active group.
    pub fn select_category(&mut self, category_id: &str) -> bool {
        let previous = self.library.clone();
        let changed = self.library.select_category(category_id);
        if changed {
            return self.persist_change(previous).is_ok();
        }
        changed
    }

    /// Creates a group with a fresh UUID and makes it active.
    pub fn add_group(&mut self, name: &str) -> Option<String> {
        let id = new_uuid();
        let previous = self.library.clone();
        let created = self.library.add_group(name, id);
        if created.is_some() && self.persist_change(previous).is_err() {
            return None;
        }
        created
    }

    pub fn rename_group(&mut self, group_id: &str, name: &str) -> bool {
        let previous = self.library.clone();
        let changed = self.library.rename_group(group_id, name);
        if changed {
            return self.persist_change(previous).is_ok();
        }
        changed
    }

    pub fn set_group_color(&mut self, group_id: &str, color: &str) -> bool {
        let previous = self.library.clone();
        let changed = self.library.set_group_color(group_id, color);
        if changed {
            return self.persist_change(previous).is_ok();
        }
        changed
    }

    /// Moves a group to a new position (drag reorder).
    pub fn move_group(&mut self, group_id: &str, new_index: usize) -> bool {
        let previous = self.library.clone();
        let changed = self.library.move_group(group_id, new_index);
        if changed {
            return self.persist_change(previous).is_ok();
        }
        changed
    }

    /// Deletes a group. A group with content needs the user's decision first
    /// (baseline 2.2); the returned request describes what is at stake.
    pub fn delete_group(&mut self, group_id: &str) -> Result<DeleteOutcome, PendingDelete> {
        let previous = self.library.clone();
        match self.library.delete_group(group_id) {
            DeleteOutcome::Removed => {
                let _ = self.persist_change(previous);
                Ok(DeleteOutcome::Removed)
            }
            DeleteOutcome::NotFound => Ok(DeleteOutcome::NotFound),
            DeleteOutcome::NeedsConfirmation {
                content_summary,
                item_count,
                category_count,
            } => {
                let group_name = self
                    .library
                    .group(group_id)
                    .map(|group| group.name.clone())
                    .unwrap_or_default();
                Err(PendingDelete {
                    group_id: group_id.to_string(),
                    group_name,
                    content_summary,
                    item_count,
                    category_count,
                })
            }
        }
    }

    /// Deletes a group after the user confirmed dropping its content.
    pub fn delete_group_confirmed(&mut self, group_id: &str) -> bool {
        let previous = self.library.clone();
        let changed = self.library.delete_group_confirmed(group_id);
        if changed {
            return self.persist_change(previous).is_ok();
        }
        changed
    }

    /// Deletes a group after the user chose to migrate its content elsewhere.
    pub fn migrate_group_into(&mut self, group_id: &str, target_group_id: &str) -> bool {
        let previous = self.library.clone();
        let changed = self.library.migrate_group_into(group_id, target_group_id);
        if changed {
            return self.persist_change(previous).is_ok();
        }
        changed
    }

    /// Creates a category in the active group.
    pub fn add_category(&mut self, name: &str) -> Option<String> {
        let group_id = self.library.active_group_id()?.to_string();
        let id = new_uuid();
        let previous = self.library.clone();
        let created = self.library.add_category(&group_id, name, id);
        if created.is_some() && self.persist_change(previous).is_err() {
            return None;
        }
        created
    }

    /// Renames a category of the active group.
    pub fn rename_category(&mut self, category_id: &str, name: &str) -> bool {
        let previous = self.library.clone();
        let changed = self.library.rename_category(category_id, name);
        if changed {
            return self.persist_change(previous).is_ok();
        }
        changed
    }

    /// Deletes a category. A category with items returns a pending request
    /// describing how many items are at stake (baseline 2.3).
    pub fn delete_category(
        &mut self,
        category_id: &str,
    ) -> Result<crate::library_model::CategoryDeleteOutcome, usize> {
        let previous = self.library.clone();
        match self.library.delete_category(category_id) {
            crate::library_model::CategoryDeleteOutcome::Removed => {
                let _ = self.persist_change(previous);
                Ok(crate::library_model::CategoryDeleteOutcome::Removed)
            }
            crate::library_model::CategoryDeleteOutcome::NotFound => {
                Ok(crate::library_model::CategoryDeleteOutcome::NotFound)
            }
            crate::library_model::CategoryDeleteOutcome::NeedsConfirmation { item_count } => {
                Err(item_count)
            }
        }
    }

    /// Deletes a category after the user confirmed dropping its items.
    pub fn delete_category_confirmed(&mut self, category_id: &str) -> bool {
        let previous = self.library.clone();
        let changed = self.library.delete_category_confirmed(category_id);
        if changed {
            return self.persist_change(previous).is_ok();
        }
        changed
    }

    /// Moves the items of one category into another (migrate choice), then
    /// removes the source category.
    pub fn migrate_category_items_into(&mut self, category_id: &str, target_id: &str) -> bool {
        let previous = self.library.clone();
        let changed = self
            .library
            .migrate_category_items_into(category_id, target_id);
        if changed {
            return self.persist_change(previous).is_ok();
        }
        changed
    }

    /// Moves a category to a new position (drag reorder / up-down buttons).
    pub fn move_category(&mut self, category_id: &str, new_index: usize) -> bool {
        let previous = self.library.clone();
        let changed = self.library.move_category(category_id, new_index);
        if changed {
            return self.persist_change(previous).is_ok();
        }
        changed
    }

    /// Creates a project in the active category (baseline 3.2/3.4).
    pub fn add_item(&mut self, draft: ItemDraft) -> Result<String, String> {
        draft.validate().map_err(str::to_string)?;
        let id = new_uuid();
        let previous = self.library.clone();
        if !self.library.add_item(draft.into_item(id.clone())) {
            return Err("添加项目失败".to_string());
        }
        self.persist_change(previous)?;
        Ok(id)
    }

    /// Creates a project in a specific category (P1-17 external drop onto a
    /// sidebar row or a non-active group tab).
    pub fn add_item_to_category(
        &mut self,
        category_id: &str,
        draft: ItemDraft,
    ) -> Result<String, String> {
        draft.validate().map_err(str::to_string)?;
        if !self.category_exists(category_id) {
            return Err("目标分类不存在".to_string());
        }
        let id = new_uuid();
        let previous = self.library.clone();
        if !self
            .library
            .add_item_to_category(category_id, draft.into_item(id.clone()))
        {
            return Err("添加项目失败".to_string());
        }
        self.persist_change(previous)?;
        Ok(id)
    }

    fn category_exists(&self, category_id: &str) -> bool {
        self.library
            .groups
            .iter()
            .flat_map(|group| group.categories.iter())
            .any(|category| category.id == category_id)
    }

    /// The active group id (P1-17 drop targeting).
    pub fn active_group_id(&self) -> Option<String> {
        self.library.active_group_id().map(str::to_string)
    }

    /// The active category id (P1-17 grid drops append here).
    pub fn active_category_id(&self) -> Option<String> {
        self.library
            .active_category()
            .map(|category| category.id.clone())
    }

    /// The display name of a category, used by drop status messages.
    pub fn category_name(&self, category_id: &str) -> Option<String> {
        self.library
            .groups
            .iter()
            .flat_map(|group| group.categories.iter())
            .find(|category| category.id == category_id)
            .map(|category| category.name.clone())
    }

    /// Updates an existing project in place.
    pub fn update_item(&mut self, item_id: &str, draft: ItemDraft) -> Result<(), String> {
        draft.validate().map_err(str::to_string)?;
        let mut item = draft.into_item(item_id.to_string());
        // P2-17: the verification mark is not an editor field; editing an
        // imported command item must not silently clear it.
        // P4 审核修复（2026-11-07）：同时保留使用统计、收藏与导入元数据——
        // 它们都不是编辑器字段，旧实现会把它们清零。
        if let Some(existing) = self.library.find_item(item_id) {
            item.launch.confirm_on_first_run = existing.launch.confirm_on_first_run;
            // P3-03: the delay is not an editor field either.
            item.launch.delay_seconds = existing.launch.delay_seconds;
            if existing.item_type == ItemKind::Lnk
                && item.item_type == ItemKind::Lnk
                && existing.launch.path == item.launch.path
            {
                item.launch.aumid = existing.launch.aumid.clone();
                item.launch.msi_advertised = existing.launch.msi_advertised;
            }
            item.last_used = existing.last_used;
            item.launch_count = existing.launch_count;
            item.favorite = existing.favorite;
            let same_identity = existing.item_type == item.item_type
                && existing.launch.path == item.launch.path
                && existing.launch.args == item.launch.args
                && existing.launch.execution_mode == item.launch.execution_mode
                && existing.launch.workdir == item.launch.workdir
                && existing.launch.run_as_admin == item.launch.run_as_admin;
            if matches!(item.item_type, ItemKind::Command | ItemKind::Automation) && !same_identity
            {
                item.launch.confirm_on_first_run = true;
            }
        }
        let previous = self.library.clone();
        if !self.library.update_item(item) {
            return Err("项目不存在".to_string());
        }
        self.persist_change(previous)?;
        Ok(())
    }

    pub fn delete_item(&mut self, item_id: &str) -> bool {
        let previous = self.library.clone();
        let changed = self.library.remove_item(item_id);
        if changed {
            return self.persist_change(previous).is_ok();
        }
        changed
    }

    /// Duplicates a project (baseline 3.5) with a fresh UUID; the copy is
    /// appended to the active category and named "<name> 副本".
    pub fn duplicate_item(&mut self, item_id: &str) -> Option<String> {
        let mut copy = self.library.active_category_item(item_id)?.clone();
        let id = new_uuid();
        copy.id = id.clone();
        copy.name = format!("{} 副本", copy.name);
        let previous = self.library.clone();
        if !self.library.add_item(copy) {
            return None;
        }
        self.persist_change(previous).ok()?;
        Some(id)
    }

    /// Moves a project to another category of the active group (baseline 3.5).
    pub fn move_item_to_category(&mut self, item_id: &str, target_category_id: &str) -> bool {
        let previous = self.library.clone();
        let changed = self
            .library
            .move_item_to_category(item_id, target_category_id);
        if changed {
            return self.persist_change(previous).is_ok();
        }
        changed
    }

    /// Reorders a project inside the active category (P2-04 grid drag).
    pub fn move_item_to_index(&mut self, item_id: &str, new_index: usize) -> bool {
        let previous = self.library.clone();
        let changed = self.library.move_item_to_index(item_id, new_index);
        if changed {
            return self.persist_change(previous).is_ok();
        }
        changed
    }

    /// Moves a project into a category of any group (P2-06 cross-group drag).
    pub fn move_item_to_group_category(&mut self, item_id: &str, target_category_id: &str) -> bool {
        let previous = self.library.clone();
        let changed = self
            .library
            .move_item_across_groups(item_id, target_category_id);
        if changed {
            return self.persist_change(previous).is_ok();
        }
        changed
    }

    /// The active category of an arbitrary group (P2-06 spring-load drop
    /// targets the group's current category).
    pub fn active_category_id_of_group(&self, group_id: &str) -> Option<String> {
        self.library
            .active_category_of_group(group_id)
            .map(|category| category.id.clone())
    }

    /// The category that currently holds the item, in any group (P2-06).
    pub fn category_id_of_item(&self, item_id: &str) -> Option<String> {
        self.library.category_id_of_item(item_id)
    }

    /// The dialog draft for an existing project (edit prefill).
    pub fn item_draft(&self, item_id: &str) -> Option<ItemDraft> {
        self.library
            .active_category_item(item_id)
            .map(ItemDraft::from_item)
    }

    /// A clone of the project for launching (P1-06). Search can launch items
    /// from any group (P2-01), so the lookup is library-wide.
    pub fn item(&self, item_id: &str) -> Option<Item> {
        self.library.find_item(item_id).cloned()
    }

    /// Commit a fallible mutation, rolling memory back if the disk commit fails.
    /// Controllers without a path are explicitly in-memory (used by the UI tests).
    fn persist_change(&mut self, previous: Library) -> Result<(), String> {
        if self.config_path.is_none() && !matches!(self.load_state, LoadState::SafeMode { .. }) {
            return Ok(());
        }
        if let Err(error) = self.persist() {
            self.library = previous;
            self.pinyin_cache.borrow_mut().clear();
            return Err(error);
        }
        Ok(())
    }

    /// Writes the library through the atomic save chain. Failures are
    /// reported and recorded for the UI; safe mode refuses writes so the
    /// original file is never overwritten (baseline 15.8).
    pub fn persist(&self) -> Result<SaveOutcome, String> {
        if let LoadState::SafeMode { reason } = &self.load_state {
            return Err(format!("安全模式：{reason}"));
        }
        let Some(path) = &self.config_path else {
            return Err("no config path".to_string());
        };
        let json = serde_json::to_string_pretty(&self.library)
            .map_err(|error| format!("serialize failed: {error}"))?;
        match save_atomic(path, &json) {
            Ok(outcome) => {
                self.save_error.borrow_mut().take();
                let _ = prune_backups(path, 10);
                Ok(outcome)
            }
            Err(error) => {
                let message = error.to_string();
                *self.save_error.borrow_mut() = Some(message.clone());
                Err(message)
            }
        }
    }

    /// Creates an explicit manual backup of the current config (baseline
    /// 15.7) and prunes old backups. Safe mode refuses to write.
    pub fn backup_now(&self) -> Result<PathBuf, String> {
        if let LoadState::SafeMode { reason } = &self.load_state {
            return Err(format!("安全模式：{reason}"));
        }
        let Some(path) = &self.config_path else {
            return Err("no config path".to_string());
        };
        if !path.exists() {
            self.persist()?;
        }
        crate::infrastructure::windows::config_save::backup_now(path)
            .map_err(|error| error.to_string())
    }

    /// P2-16 (baseline 17.4): the historical backups the user can restore,
    /// newest first. Only backups whose content parses are offered.
    pub fn backup_entries(&self) -> Vec<crate::infrastructure::windows::config_save::BackupEntry> {
        let Some(path) = &self.config_path else {
            return Vec::new();
        };
        crate::infrastructure::windows::config_save::list_backups(path)
    }

    /// P3-01: toggles an item in the batch selection; returns true when the
    /// item is selected afterwards.
    pub fn toggle_batch_selection(&self, item_id: &str) -> bool {
        let mut selection = self.batch_selection.borrow_mut();
        match selection.iter().position(|id| id == item_id) {
            Some(index) => {
                selection.remove(index);
                false
            }
            None => {
                selection.push(item_id.to_string());
                true
            }
        }
    }

    /// P3-01: whether the item is part of the batch selection.
    pub fn is_batch_selected(&self, item_id: &str) -> bool {
        self.batch_selection.borrow().iter().any(|id| id == item_id)
    }

    /// P3-01: the selected item ids in selection order.
    pub fn batch_selection_ids(&self) -> Vec<String> {
        self.batch_selection.borrow().clone()
    }

    /// P3-01: drops the batch selection.
    pub fn clear_batch_selection(&self) {
        self.batch_selection.borrow_mut().clear();
    }

    /// P3-05/P3-06 (baseline 22): records a successful launch — last_used =
    /// now (unix ms), launch_count += 1 — and persists.
    pub fn record_launch(&mut self, item_id: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0);
        let previous = self.library.clone();
        if self.library.record_launch(item_id, now) {
            let _ = self.persist_change(previous);
        }
    }

    /// P3-05/P3-06/P3-07: the grid sort mode for the active panel.
    pub fn sort_mode(&self) -> &'static str {
        crate::library_model::normalize_sort_mode(&self.library.settings.sort_mode)
    }

    /// P3-08 (baseline 8.3): records the user's confirmation for a command
    /// item so the first-run prompt shows only once.
    pub fn confirm_command(&mut self, item_id: &str) -> bool {
        let previous = self.library.clone();
        let Some(item) = self.library.find_item_mut(item_id) else {
            return false;
        };
        item.launch.confirm_on_first_run = false;
        self.persist_change(previous).is_ok()
    }

    /// P3-09 (baseline 22 Everything 可选集成): the opt-in state.
    pub fn everything_enabled(&self) -> bool {
        self.library.settings.everything_enabled
    }

    /// P3-09: toggles the Everything integration and persists.
    pub fn set_everything_enabled(&mut self, enabled: bool) {
        let previous = self.library.clone();
        self.library.settings.everything_enabled = enabled;
        let _ = self.persist_change(previous);
    }

    /// P4-04（基线 23 章）：用户可关闭更新检查。
    pub fn update_check_enabled(&self) -> bool {
        self.library.settings.update_check_enabled
    }

    /// P4-04：切换更新检查并持久化。
    pub fn set_update_check_enabled(&mut self, enabled: bool) {
        let previous = self.library.clone();
        self.library.settings.update_check_enabled = enabled;
        let _ = self.persist_change(previous);
    }

    /// P4-04：更新源清单地址。
    pub fn update_url(&self) -> String {
        self.library.settings.update_url.clone()
    }

    /// P3-05/P3-06/P3-07: switches the grid sort mode and persists.
    pub fn set_sort_mode(&mut self, mode: &str) {
        let previous = self.library.clone();
        self.library.settings.sort_mode =
            crate::library_model::normalize_sort_mode(mode).to_string();
        let _ = self.persist_change(previous);
    }

    /// P3-07: toggles an item's favorite flag; returns the new value.
    pub fn toggle_favorite(&mut self, item_id: &str) -> bool {
        let previous = self.library.clone();
        let favorite = self.library.toggle_favorite(item_id);
        // Persist on both directions: clearing a favorite must reach disk too.
        if self.persist_change(previous).is_err() {
            return self
                .library
                .find_item(item_id)
                .is_some_and(|item| item.favorite);
        }
        favorite
    }

    /// P2-17 (baseline 22 启动组合): saves the current batch selection as a
    /// named combination and clears the selection. The rows for the dialog.
    pub fn save_batch_combination(&mut self, name: &str) -> Result<(String, String), String> {
        let ids = self.batch_selection_ids();
        if ids.is_empty() {
            return Err("未选择项目".to_string());
        }
        let Some(path) = &self.config_path else {
            return Err("no config path".to_string());
        };
        let _ = path;
        if let LoadState::SafeMode { reason } = &self.load_state {
            return Err(format!("安全模式：{reason}"));
        }
        let previous = self.library.clone();
        let combination = self.library.add_combination(name, ids);
        self.persist_change(previous)?;
        self.clear_batch_selection();
        Ok((combination.id, combination.name))
    }

    /// P3-02: the saved combinations for the dialog, newest last.
    pub fn combination_rows(&self) -> Vec<(String, String, usize)> {
        self.library
            .combinations
            .iter()
            .map(|combination| {
                (
                    combination.id.clone(),
                    combination.name.clone(),
                    combination.item_ids.len(),
                )
            })
            .collect()
    }

    /// P3-02: the item ids of a combination; `None` when it is gone. Ids
    /// that no longer resolve to a live item are dropped so a deleted project
    /// never breaks the launch.
    pub fn combination_item_ids(&self, id: &str) -> Option<Vec<String>> {
        let combination = self.library.combination(id)?.item_ids.clone();
        let live: Vec<String> = combination
            .into_iter()
            .filter(|item_id| self.library.find_item(item_id).is_some())
            .collect();
        Some(live)
    }

    /// P3-02: removes a combination and persists.
    pub fn delete_combination(&mut self, id: &str) -> Result<(), String> {
        let previous = self.library.clone();
        if self.library.remove_combination(id) {
            self.persist_change(previous)?;
        }
        Ok(())
    }

    /// P2-17 (baseline 17.4): writes the full configuration to an export file.
    /// Returns the number of bytes written.
    pub fn export_all_to(&self, path: &std::path::Path) -> Result<usize, String> {
        let document = crate::config_transfer::export_all(&self.library);
        write_export(path, &document)
    }

    /// P2-17 (baseline 17.4): writes one group to an export file.
    pub fn export_group_to(&self, path: &std::path::Path, group_id: &str) -> Result<usize, String> {
        let Some(document) = crate::config_transfer::export_group(&self.library, group_id) else {
            return Err("分组不存在".to_string());
        };
        write_export(path, &document)
    }

    /// P2-17 (baseline 17.4/15.7/8.4): imports an export or configuration
    /// file. A backup of the live configuration is written first, the document
    /// is applied in the requested mode and the result persisted.
    pub fn import_from(
        &mut self,
        path: &std::path::Path,
        mode: crate::config_transfer::ImportMode,
    ) -> Result<crate::config_transfer::ImportReport, String> {
        if let LoadState::SafeMode { reason } = &self.load_state {
            return Err(format!("安全模式：{reason}"));
        }
        let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let document = crate::config_transfer::parse_import(&text)?;
        // Baseline 15.7 "导入前备份": the current configuration is kept before
        // anything is applied, so an unwanted import can be undone from the
        // backup dialog.
        if let Some(config) = &self.config_path
            && config.exists()
        {
            crate::infrastructure::windows::config_save::backup_now(config)
                .map_err(|error| error.to_string())?;
        }
        // P4 审核修复（2026-11-07）：保存失败时回滚内存状态——旧实现先改
        // 内存再落盘，失败后界面报“导入失败”但内存已是导入内容，随后的
        // 普通保存会把“失败的导入”正式写入磁盘。
        let previous = self.library.clone();
        let report = crate::config_transfer::apply_import(&mut self.library, &document, mode);
        self.pinyin_cache.borrow_mut().clear();
        if let Err(error) = self.persist() {
            self.library = previous;
            self.pinyin_cache.borrow_mut().clear();
            return Err(error);
        }
        Ok(report)
    }

    /// P2-16 (baseline 17.4): restores a historical backup and reloads it
    /// into memory. The live configuration is preserved as a backup by the
    /// atomic replace, so the restore can itself be undone.
    pub fn restore_backup(&mut self, backup: &std::path::Path) -> Result<String, String> {
        if let LoadState::SafeMode { reason } = &self.load_state {
            return Err(format!("安全模式：{reason}"));
        }
        let Some(path) = &self.config_path else {
            return Err("no config path".to_string());
        };
        let (replaced, library) =
            crate::infrastructure::windows::config_save::restore_backup_with_library(path, backup)
                .map_err(|error| error.to_string())?;
        self.library = library;
        self.load_state = LoadState::Loaded;
        self.pinyin_cache.borrow_mut().clear();
        self.save_error.borrow_mut().take();
        Ok(format!("已恢复；恢复前的配置保存在 {}", replaced.display()))
    }
}

/// Writes an export document as pretty JSON (P2-17).
fn write_export(
    path: &std::path::Path,
    document: &crate::config_transfer::ExportDocument,
) -> Result<usize, String> {
    let json = serde_json::to_string_pretty(document).map_err(|error| error.to_string())?;
    std::fs::write(path, json.as_bytes()).map_err(|error| error.to_string())?;
    Ok(json.len())
}

/// Parses a config document, tolerating a UTF-8 BOM.
fn parse_library(content: &str) -> Option<Library> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    serde_json::from_str::<Library>(content).ok()
}

/// Estimates the pixel width of a group tab. Slint 1.18 has no string
/// measurement API, so the controller estimates: CJK glyphs are about twice
/// as wide as latin ones, plus the horizontal padding.
fn tab_width_px(name: &str) -> f32 {
    let mut width = 16.0_f32; // horizontal padding
    for character in name.chars() {
        width += if character as u32 > 0x2E80 { 15.0 } else { 8.0 };
    }
    width
}

/// The baseline sample library: 常用工具 / 应用 / 自动化 (baseline 2.2).
pub fn sample_library() -> Library {
    let mut library = Library::default();
    let tools = library
        .add_group("常用工具", new_uuid())
        .expect("first group");
    let apps = library.add_group("应用", new_uuid()).expect("second group");
    let automation = library
        .add_group("自动化", new_uuid())
        .expect("third group");
    let tool_categories = [
        ("必备软件", 1),
        ("系统工具", 2),
        ("浏览器", 3),
        ("办公软件", 4),
        ("视频音乐", 5),
        ("系统优化", 6),
        ("AI", 7),
    ];
    for (name, index) in tool_categories {
        library.add_category(&tools, name, format!("tool-category-{index}"));
    }
    library.add_category(&apps, "桌面应用", "app-category-1");
    library.add_category(&apps, "便携应用", "app-category-2");
    library.add_category(&automation, "脚本", "automation-category-1");
    // P1-04: the sample library needs projects so the grid renders real model
    // rows. UI 体验修复（2026-11-07 第三批）：文件/文件夹示例改用真实存在的
    // 系统位置——首次启动全部示例有效可启动，不再出现红色失效角标
    //（badge 演示可由用户删除目标后自见）。URI/command rows must never be
    // flagged.
    let demo_items = [
        (
            "应用示例",
            ItemKind::Exe,
            "C:\\Windows\\System32\\notepad.exe",
            "",
        ),
        (
            "工具示例",
            ItemKind::Lnk,
            "C:\\Windows\\System32\\calc.exe",
            "",
        ),
        (
            "文件示例",
            ItemKind::File,
            "C:\\Windows\\System32\\drivers\\etc\\hosts",
            "系统文件示例",
        ),
        (
            "文件夹示例",
            ItemKind::Folder,
            "C:\\Windows\\System32",
            "系统文件夹示例",
        ),
        ("链接示例", ItemKind::Uri, "https://example.com", "网址示例"),
        ("命令示例", ItemKind::Command, "", "自定义命令示例"),
    ];
    if let Some(category) = library
        .group_mut(&tools)
        .and_then(|group| group.category_mut("tool-category-1"))
    {
        for (index, (name, kind, target, note)) in demo_items.iter().enumerate() {
            let mut item = Item::new(format!("demo-item-{}", index + 1), *name, *kind, *target);
            item.note = (*note).to_string();
            category.items.push(item);
        }
    }
    library.select_group(&tools);
    library.select_category("tool-category-1");
    library
}

/// A shared, mutable controller for the UI thread.
pub type SharedController = Rc<RefCell<LibraryController>>;

#[cfg(test)]
mod tests {
    use super::*;

    fn controller() -> LibraryController {
        LibraryController::load(None)
    }

    #[test]
    fn editing_shortcut_preserves_metadata_only_for_identical_lnk_target() {
        for (kind, path, expected) in [
            (ItemKind::Lnk, "old.lnk", "old-aumid"),
            (ItemKind::Lnk, "new.lnk", "new-aumid"),
            (ItemKind::Exe, "old.lnk", "new-aumid"),
        ] {
            let mut controller = controller();
            let mut old = Item::new("edit", "Shortcut", ItemKind::Lnk, "old.lnk");
            old.launch.aumid = "old-aumid".into();
            old.launch.msi_advertised = true;
            old.launch.delay_seconds = 7;
            old.favorite = true;
            old.last_used = 123;
            old.launch_count = 42;
            controller.library.add_item(old.clone());
            let mut draft = ItemDraft::from_item(&old);
            draft.item_type = kind;
            draft.path = path.into();
            draft.aumid = "new-aumid".into();
            draft.msi_advertised = false;
            draft.favorite = false;
            draft.last_used = 0;
            draft.launch_count = 0;
            controller.update_item("edit", draft).unwrap();
            let updated = controller.item("edit").unwrap();
            assert_eq!(updated.launch.aumid, expected);
            assert_eq!(updated.launch.msi_advertised, expected == "old-aumid");
            assert!(updated.favorite);
            assert_eq!(updated.last_used, 123);
            assert_eq!(updated.launch_count, 42);
            assert_eq!(updated.launch.delay_seconds, 7);
        }
    }

    #[test]
    fn command_confirmation_covers_every_execution_identity_field() {
        for kind in [ItemKind::Command, ItemKind::Automation] {
            for field in ["type", "path", "args", "mode", "workdir", "admin", "name"] {
                let mut controller = controller();
                let mut old = Item::new("edit", "Command", kind, "program.exe");
                old.launch.execution_mode = "direct".into();
                old.launch.confirm_on_first_run = false;
                controller.library.add_item(old.clone());
                let mut draft = ItemDraft::from_item(&old);
                match field {
                    "type" => {
                        draft.item_type = if kind == ItemKind::Command {
                            ItemKind::Automation
                        } else {
                            ItemKind::Command
                        }
                    }
                    "path" => draft.path = "changed.exe".into(),
                    "args" => draft.args = "--changed".into(),
                    "mode" => draft.execution_mode = "cmd".into(),
                    "workdir" => draft.workdir = "changed".into(),
                    "admin" => draft.run_as_admin = true,
                    "name" => draft.name = "Cosmetic rename".into(),
                    _ => unreachable!(),
                }
                controller.update_item("edit", draft).unwrap();
                assert_eq!(
                    controller.item("edit").unwrap().launch.confirm_on_first_run,
                    field != "name",
                    "{kind:?} {field}"
                );
            }
        }
    }

    #[test]
    fn failed_item_and_combination_commits_roll_back_memory_and_keep_selection() {
        let _serial = crate::infrastructure::windows::config_save::test_lock();
        let dir = temp_config_dir();
        let path = dir.join("missing-parent").join("config.json");
        let mut controller = LibraryController::load(Some(path.clone()));
        let baseline = controller.library.clone();
        let item = baseline.groups[0].categories[0].items[0].clone();
        let mut draft = ItemDraft::from_item(&item);
        draft.name = "Must roll back".into();
        assert!(controller.update_item(&item.id, draft.clone()).is_err());
        assert_eq!(controller.library, baseline);
        assert!(controller.add_item(draft.clone()).is_err());
        assert_eq!(controller.library, baseline);
        let category = baseline.groups[0].categories[0].id.clone();
        assert!(controller.add_item_to_category(&category, draft).is_err());
        assert_eq!(controller.library, baseline);
        controller.toggle_batch_selection(&item.id);
        assert!(controller.save_batch_combination("failed").is_err());
        assert_eq!(controller.library, baseline);
        assert_eq!(controller.batch_selection_ids(), vec![item.id.clone()]);
        let combination = controller.library.add_combination("keep", vec![item.id]);
        let with_combination = controller.library.clone();
        assert!(controller.delete_combination(&combination.id).is_err());
        assert_eq!(controller.library, with_combination);
        assert!(!path.exists());
        assert!(controller.save_error().is_some());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_import_rolls_back_and_invalid_restore_leaves_memory_and_disk_unchanged() {
        let _serial = crate::infrastructure::windows::config_save::test_lock();
        let dir = temp_config_dir();
        let path = dir.join("missing").join("config.json");
        let imported = dir.join("import.json");
        std::fs::write(
            &imported,
            serde_json::to_string(&Library::default()).unwrap(),
        )
        .unwrap();
        let mut controller = LibraryController::load(Some(path));
        let before = controller.library.clone();
        assert!(
            controller
                .import_from(&imported, crate::config_transfer::ImportMode::Overwrite)
                .is_err()
        );
        assert_eq!(controller.library, before);
        let live = dir.join("config.json");
        let json = serde_json::to_string(&before).unwrap();
        std::fs::write(&live, &json).unwrap();
        let mut controller = LibraryController::load(Some(live.clone()));
        for bad in ["{}", "{\"schema_version\":3,\"groups\":[]}"] {
            std::fs::write(&imported, bad).unwrap();
            assert!(controller.restore_backup(&imported).is_err());
            assert_eq!(controller.library, before);
            assert_eq!(std::fs::read_to_string(&live).unwrap(), json);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn locked_live_file_rolls_back_update_import_and_restore() {
        use std::os::windows::fs::OpenOptionsExt;
        let _serial = crate::infrastructure::windows::config_save::test_lock();
        let dir = temp_config_dir();
        let path = dir.join("config.json");
        let before = sample_library();
        let json = serde_json::to_string(&before).unwrap();
        std::fs::write(&path, &json).unwrap();
        let mut controller = LibraryController::load(Some(path.clone()));
        let imported = dir.join("import.json");
        std::fs::write(
            &imported,
            serde_json::to_string(&Library::default()).unwrap(),
        )
        .unwrap();
        // Real Win32 delete-sharing denial forces ReplaceFileW to fail.
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        let item = before.groups[0].categories[0].items[0].clone();
        let mut draft = ItemDraft::from_item(&item);
        draft.name = "uncommitted".into();
        assert!(controller.update_item(&item.id, draft).is_err());
        assert_eq!(controller.library, before);
        assert!(
            controller
                .import_from(&imported, crate::config_transfer::ImportMode::Overwrite)
                .is_err()
        );
        assert_eq!(controller.library, before);
        assert!(controller.restore_backup(&imported).is_err());
        assert_eq!(controller.library, before);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), json);
        drop(lock);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn bool_option_and_setting_mutations_roll_back_on_save_failure() {
        let _serial = crate::infrastructure::windows::config_save::test_lock();
        let dir = temp_config_dir();
        let path = dir.join("missing").join("config.json");
        let mut controller = LibraryController::load(Some(path));
        let before = controller.library.clone();
        let group = &before.groups[0];
        let item = &group.categories[0].items[0];
        assert!(!controller.rename_group(&group.id, "uncommitted"));
        assert!(controller.add_group("uncommitted").is_none());
        assert!(controller.add_category("uncommitted").is_none());
        assert!(!controller.delete_item(&item.id));
        assert!(!controller.toggle_favorite(&item.id));
        controller.record_launch(&item.id);
        controller.set_everything_enabled(!before.settings.everything_enabled);
        controller.set_update_check_enabled(!before.settings.update_check_enabled);
        controller.set_sort_mode("favorite");
        assert_eq!(controller.library, before);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sample_library_has_the_baseline_groups_and_categories() {
        let controller = controller();
        let library = controller.library();
        assert_eq!(library.groups.len(), 3);
        assert_eq!(
            library
                .groups
                .iter()
                .map(|group| group.name.as_str())
                .collect::<Vec<_>>(),
            vec!["常用工具", "应用", "自动化"]
        );
        assert_eq!(
            library
                .group(&library.groups[0].id)
                .expect("group")
                .categories
                .len(),
            7
        );
        assert_eq!(
            library
                .group(&library.groups[1].id)
                .expect("group")
                .categories
                .len(),
            2
        );
        assert_eq!(
            library
                .group(&library.groups[2].id)
                .expect("group")
                .categories
                .len(),
            1
        );
    }

    #[test]
    fn group_rows_mark_the_active_group() {
        let mut controller = controller();
        let second_id = controller.library().groups[1].id.clone();
        assert!(controller.select_group(&second_id));
        let rows = controller.group_rows();
        assert_eq!(rows.len(), 3);
        assert!(!rows[0].active);
        assert!(rows[1].active);
        assert_eq!(rows[1].name, "应用");
    }

    #[test]
    fn category_rows_follow_the_active_group() {
        let mut controller = controller();
        assert_eq!(controller.category_rows().len(), 7);
        let third_id = controller.library().groups[2].id.clone();
        assert!(controller.select_group(&third_id));
        let rows = controller.category_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "脚本");
    }

    #[test]
    fn adding_a_group_switches_the_selection_and_the_view() {
        let mut controller = controller();
        let id = controller.add_group("新分组").expect("group");
        assert_eq!(controller.library().active_group_id(), Some(id.as_str()));
        assert!(
            controller
                .group_rows()
                .iter()
                .any(|row| row.active && row.name == "新分组"),
            "the new group is highlighted"
        );
    }

    #[test]
    fn deleting_a_non_empty_group_returns_a_pending_request() {
        let mut controller = controller();
        let first_id = controller.library().groups[0].id.clone();
        let request = controller
            .delete_group(&first_id)
            .expect_err("a group with categories needs confirmation");
        assert_eq!(request.group_name, "常用工具");
        assert_eq!(request.category_count, 7);
        assert_eq!(
            controller.library().groups.len(),
            3,
            "the group stays until the user decides"
        );
        assert!(controller.delete_group_confirmed(&first_id));
        assert_eq!(controller.library().groups.len(), 2);
    }

    #[test]
    fn migrating_content_keeps_it_and_removes_the_source_group() {
        let mut controller = controller();
        let first_id = controller.library().groups[0].id.clone();
        let second_id = controller.library().groups[1].id.clone();
        controller.migrate_group_into(&first_id, &second_id);
        assert_eq!(controller.library().groups.len(), 2);
        assert_eq!(
            controller
                .library()
                .group(&second_id)
                .expect("group")
                .categories
                .len(),
            9,
            "the migrated categories arrive in the target group"
        );
    }

    #[test]
    fn renaming_and_color_reach_the_model() {
        let mut controller = controller();
        let first_id = controller.library().groups[0].id.clone();
        assert!(controller.rename_group(&first_id, "常用工具集"));
        assert_eq!(
            controller.library().group(&first_id).expect("group").name,
            "常用工具集"
        );
        assert!(controller.set_group_color(&first_id, "#ff8800"));
        assert_eq!(
            controller.library().group(&first_id).expect("group").color,
            "#ff8800"
        );
    }

    #[test]
    fn moving_a_group_reorders_the_rows() {
        let mut controller = controller();
        let third_id = controller.library().groups[2].id.clone();
        assert!(controller.move_group(&third_id, 0));
        let names: Vec<String> = controller
            .group_rows()
            .iter()
            .map(|row| row.name.clone())
            .collect();
        assert_eq!(names[0], "自动化");
    }

    #[test]
    fn adding_a_category_lands_in_the_active_group() {
        let mut controller = controller();
        let second_id = controller.library().groups[1].id.clone();
        assert!(controller.select_group(&second_id));
        let id = controller.add_category("新分类").expect("category");
        let group = controller.library().group(&second_id).expect("group");
        assert_eq!(group.categories.len(), 3);
        assert!(group.category(&id).is_some());
    }

    #[test]
    fn renaming_a_category_reaches_the_model() {
        let mut controller = controller();
        let first_category = controller.category_rows()[0].id.clone();
        assert!(controller.rename_category(&first_category, "装机必备"));
        assert_eq!(
            controller.category_rows()[0].name,
            "装机必备",
            "the row view reflects the rename"
        );
        assert!(!controller.rename_category(&first_category, "   "));
    }

    #[test]
    fn deleting_an_empty_category_removes_it_without_confirmation() {
        let mut controller = controller();
        let id = controller.add_category("临时分类").expect("category");
        let outcome = controller
            .delete_category(&id)
            .expect("empty category deletes directly");
        assert!(matches!(
            outcome,
            crate::library_model::CategoryDeleteOutcome::Removed
        ));
        assert!(!controller.category_rows().iter().any(|row| row.id == id));
    }

    #[test]
    fn deleting_a_category_with_items_needs_confirmation_then_migrates() {
        let mut controller = controller();
        // Give an empty category an item so deletion requires confirmation.
        let source = controller.category_rows()[1].id.clone();
        assert!(controller.select_category(&source));
        let item_id = new_uuid();
        controller
            .library_mut()
            .active_category_mut()
            .expect("category")
            .items
            .push(crate::library_model::Item::new(
                item_id,
                "记事本",
                crate::library_model::ItemKind::Exe,
                "notepad.exe",
            ));
        let pending = controller
            .delete_category(&source)
            .expect_err("needs confirmation");
        assert_eq!(pending, 1, "one item at stake");
        assert!(
            controller
                .category_rows()
                .iter()
                .any(|row| row.id == source),
            "the category survives until the user decides"
        );
        // The user chooses migration into the third category.
        let target = controller.category_rows()[2].id.clone();
        assert!(controller.migrate_category_items_into(&source, &target));
        // UI 体验修复（2026-11-07）：迁移到自己必须无效且不改动数据——
        // 迁移下拉已在 UI 层排除被删分类，此处锁定模型守卫。
        assert!(!controller.migrate_category_items_into(&source, &source));
        assert!(
            !controller
                .category_rows()
                .iter()
                .any(|row| row.id == source)
        );
        assert_eq!(
            controller
                .library()
                .active_group()
                .and_then(|group| group.category(&target))
                .expect("target category")
                .items
                .len(),
            1,
            "the item travelled into the target category"
        );
    }

    #[test]
    fn deleting_a_category_with_items_confirmed_drops_them() {
        let mut controller = controller();
        let source = controller.category_rows()[1].id.clone();
        assert!(controller.select_category(&source));
        controller
            .library_mut()
            .active_category_mut()
            .expect("category")
            .items
            .push(crate::library_model::Item::new(
                new_uuid(),
                "记事本",
                crate::library_model::ItemKind::Exe,
                "notepad.exe",
            ));
        let pending = controller
            .delete_category(&source)
            .expect_err("needs confirmation");
        assert_eq!(pending, 1);
        assert!(controller.delete_category_confirmed(&source));
        assert!(
            !controller
                .category_rows()
                .iter()
                .any(|row| row.id == source)
        );
    }

    #[test]
    fn moving_a_category_reorders_the_rows() {
        let mut controller = controller();
        let rows = controller.category_rows();
        let last_id = rows[rows.len() - 1].id.clone();
        assert!(controller.move_category(&last_id, 0));
        assert_eq!(controller.category_rows()[0].id, last_id);
        assert!(controller.move_category(&last_id, 99), "clamps");
        assert_eq!(controller.category_rows().last().expect("row").id, last_id);
    }

    #[test]
    fn item_rows_show_the_active_category_projects() {
        let controller = controller();
        let rows = controller.item_rows();
        assert_eq!(rows.len(), 6);
        assert_eq!(rows[0].name, "应用示例");
        assert_eq!(rows[0].item_type, "Windows 程序");
        assert!(
            !rows[0].path_missing,
            "notepad.exe exists on the test machine"
        );
        assert!(!rows[1].path_missing, "calc.exe exists on the test machine");
        // 第三批示例优化：文件/文件夹示例指向真实系统位置，全部有效。
        assert!(
            !rows[2].path_missing,
            "the demo file now points at the real hosts file"
        );
        assert!(
            !rows[3].path_missing,
            "the demo folder now points at System32"
        );
        assert!(!rows[4].path_missing, "URIs are never flagged");
        assert!(!rows[5].path_missing, "commands are never flagged");
    }

    #[test]
    fn item_rows_follow_the_selected_category() {
        let mut controller = controller();
        let second_id = controller.category_rows()[1].id.clone();
        assert!(controller.select_category(&second_id));
        assert!(controller.item_rows().is_empty());
        let first_id = controller.category_rows()[0].id.clone();
        assert!(controller.select_category(&first_id));
        assert_eq!(controller.item_rows().len(), 6);
    }

    #[test]
    fn path_missing_only_checks_file_like_targets() {
        let existing = std::env::temp_dir();
        let mut exe = Item::new("a", "存在", ItemKind::Exe, existing.to_string_lossy());
        assert!(!exe.path_missing());
        let missing = existing.join(format!("quicknest-missing-{}", new_uuid()));
        exe.launch.path = missing.to_string_lossy().into_owned();
        assert!(exe.path_missing());
        let mut folder = Item::new("b", "文件夹", ItemKind::Folder, missing.to_string_lossy());
        assert!(folder.path_missing());
        let uri = Item::new("c", "网址", ItemKind::Uri, "https://example.com");
        assert!(!uri.path_missing());
        let command = Item::new("d", "命令", ItemKind::Command, "");
        assert!(!command.path_missing());
        folder.launch.path = "   ".to_string();
        assert!(!folder.path_missing(), "blank path is not broken");
    }

    #[test]
    fn icon_size_parsing_and_metrics() {
        assert_eq!(IconSize::from_settings("small"), IconSize::Small);
        assert_eq!(IconSize::from_settings(" LARGE "), IconSize::Large);
        assert_eq!(IconSize::from_settings("medium"), IconSize::Medium);
        assert_eq!(IconSize::from_settings("bogus"), IconSize::Medium);
        assert_eq!(IconSize::default(), IconSize::Medium);
        let (sw, sh) = IconSize::Small.cell();
        let (mw, mh) = IconSize::Medium.cell();
        let (lw, lh) = IconSize::Large.cell();
        assert!(sw < mw && mw < lw && sh < mh && mh < lh);
        assert!(IconSize::Small.icon_px() < IconSize::Medium.icon_px());
        assert!(IconSize::Medium.icon_px() < IconSize::Large.icon_px());
    }

    #[test]
    fn controller_reports_the_persisted_icon_size() {
        let mut controller = controller();
        assert_eq!(controller.icon_size(), IconSize::Medium);
        controller.library_mut().settings.icon_size = "large".to_string();
        assert_eq!(controller.icon_size(), IconSize::Large);
    }

    #[test]
    fn set_icon_size_normalizes_and_persists() {
        // UI 体验修复（2026-11-07 底栏“视图”）：非法值归一化为 medium，
        // 合法值（含大小写/空白）持久化到配置。
        let mut controller = controller();
        controller.set_icon_size(" LARGE ");
        assert_eq!(controller.icon_size(), IconSize::Large);
        assert_eq!(controller.library().settings.icon_size, "large");
        controller.set_icon_size("bogus");
        assert_eq!(controller.icon_size(), IconSize::Medium);
        assert_eq!(controller.library().settings.icon_size, "medium");
    }

    #[test]
    fn loading_strips_a_utf8_bom() {
        // PowerShell 5.1 and some editors save JSON with a UTF-8 BOM; without
        // stripping it serde falls back to the sample library silently.
        let directory = std::env::temp_dir().join(format!("quicknest-bom-{}", new_uuid()));
        std::fs::create_dir_all(&directory).expect("temp dir");
        let path = directory.join("config.json");
        let mut library = sample_library();
        library.settings.icon_size = "large".to_string();
        let json = serde_json::to_string(&library).expect("serialize");
        std::fs::write(&path, format!("\u{feff}{json}")).expect("write");
        let controller = LibraryController::load(Some(path));
        assert_eq!(controller.library().groups.len(), 3);
        assert_eq!(controller.icon_size(), IconSize::Large);
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn item_crud_round_trip() {
        let mut controller = controller();
        let first_category = controller.category_rows()[0].id.clone();
        assert!(controller.select_category(&first_category));
        let before = controller.item_rows().len();
        let mut draft = ItemDraft::empty();
        draft.name = "新增项目".to_string();
        draft.path = "C:\\Windows\\notepad.exe".to_string();
        draft.args = "-x \"a b\"".to_string();
        let id = controller.add_item(draft).expect("add");
        assert_eq!(controller.item_rows().len(), before + 1);
        let draft = controller.item_draft(&id).expect("draft");
        // P4 审核修复：参数编辑往返无损（旧行为会丢引号变成 -x a b）。
        assert_eq!(draft.args, "-x \"a b\"");
        // Update in place.
        let mut edited = draft;
        edited.name = "改名项目".to_string();
        edited.window_state = crate::library_model::WindowState::Maximized;
        assert!(controller.update_item(&id, edited).is_ok());
        let row = controller
            .item_rows()
            .into_iter()
            .find(|row| row.id == id)
            .expect("row");
        assert_eq!(row.name, "改名项目");
        // Duplicate appends "<name> 副本" with a fresh id.
        let copy = controller.duplicate_item(&id).expect("copy");
        assert_eq!(controller.item_rows().len(), before + 2);
        let copy_row = controller
            .item_rows()
            .into_iter()
            .find(|row| row.id == copy)
            .expect("copy row");
        assert_eq!(copy_row.name, "改名项目 副本");
        // Move to the empty second category.
        let second = controller.category_rows()[1].id.clone();
        assert!(controller.move_item_to_category(&copy, &second));
        assert!(!controller.item_rows().iter().any(|row| row.id == copy));
        assert!(controller.select_category(&second));
        assert!(controller.item_rows().iter().any(|row| row.id == copy));
        // Delete the copy; the original stays in the first category.
        assert!(controller.delete_item(&copy));
        assert!(controller.item_rows().is_empty());
        assert!(controller.select_category(&first_category));
        assert!(controller.item_rows().iter().any(|row| row.id == id));
    }

    #[test]
    fn item_validation_reaches_the_caller() {
        let mut controller = controller();
        let mut draft = ItemDraft::empty();
        draft.name = " ".to_string();
        assert_eq!(controller.add_item(draft).unwrap_err(), "名称不能为空");
        let mut draft = ItemDraft::empty();
        draft.name = "没有目标".to_string();
        assert_eq!(controller.add_item(draft).unwrap_err(), "目标不能为空");
        assert!(!controller.delete_item("missing"));
        assert!(!controller.move_item_to_category("missing", "anything"));
    }

    fn temp_config_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("quicknest-config-{}", new_uuid()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn load_rejects_incompatible_schema_versions_without_touching_the_file() {
        let dir = temp_config_dir();
        let path = dir.join("config.json");
        let mut library = sample_library();
        library.schema_version = 3;
        std::fs::write(&path, serde_json::to_string(&library).expect("json")).expect("write");
        let before = std::fs::read_to_string(&path).expect("read");
        let controller = LibraryController::load(Some(path.clone()));
        assert!(matches!(
            controller.load_state(),
            LoadState::SafeMode { .. }
        ));
        assert!(controller.persist().is_err(), "safe mode refuses writes");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            before,
            "file untouched"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_enters_safe_mode_for_corrupt_config_without_backup() {
        let dir = temp_config_dir();
        let path = dir.join("config.json");
        std::fs::write(&path, b"{ this is not json").expect("write");
        let controller = LibraryController::load(Some(path.clone()));
        assert!(matches!(
            controller.load_state(),
            LoadState::SafeMode { .. }
        ));
        assert!(controller.persist().is_err());
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "{ this is not json",
            "corrupt file is never overwritten"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_recovers_from_the_newest_valid_backup() {
        let _serial = crate::infrastructure::windows::config_save::test_lock();
        let dir = temp_config_dir();
        let path = dir.join("config.json");
        std::fs::write(&path, b"garbage").expect("corrupt");
        let backup_path = dir.join("config.backup.20260101-000000-000.json");
        let mut backup = sample_library();
        backup.settings.icon_size = "large".to_string();
        std::fs::write(&backup_path, serde_json::to_string(&backup).expect("json"))
            .expect("backup");
        let controller = LibraryController::load(Some(path.clone()));
        match controller.load_state() {
            LoadState::Recovered {
                backup: restored,
                corrupted,
            } => {
                assert_eq!(restored, &backup_path);
                assert!(
                    corrupted.as_ref().is_some_and(|location| location.exists()),
                    "corrupt file renamed out of the way"
                );
            }
            other => panic!("unexpected state: {other:?}"),
        }
        let restored = std::fs::read_to_string(&path).expect("restored");
        assert!(restored.contains("\"large\""), "backup content restored");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_failures_are_recorded_for_the_ui() {
        let _serial = crate::infrastructure::windows::config_save::test_lock();
        let dir = temp_config_dir();
        let path = dir.join("missing-subdir").join("config.json");
        let mut controller = LibraryController::load(Some(path));
        assert_eq!(controller.save_error(), None);
        let created = controller.add_category("新分类");
        assert!(
            created.is_none(),
            "failed transaction does not expose an uncommitted id"
        );
        assert!(
            controller.save_error().is_some(),
            "the failed write is recorded"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn manual_backup_creates_a_recoverable_file() {
        let _serial = crate::infrastructure::windows::config_save::test_lock();
        let dir = temp_config_dir();
        let path = dir.join("config.json");
        let mut controller = LibraryController::load(Some(path.clone()));
        assert!(matches!(controller.load_state(), LoadState::Loaded));
        controller.add_category("备份测试");
        // The write lock is process-global; other tests saving concurrently
        // can briefly hold it, so retry the Busy case.
        let mut backup = None;
        for _ in 0..100 {
            match controller.backup_now() {
                Ok(created) => {
                    backup = Some(created);
                    break;
                }
                Err(error) if error.contains("another write transaction") => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => panic!("manual backup failed: {error}"),
            }
        }
        let backup = backup.expect("manual backup after retries");
        assert!(backup.exists());
        assert_eq!(controller.save_error(), None);
        let content = std::fs::read_to_string(&backup).expect("read backup");
        let parsed: Library = serde_json::from_str(&content).expect("valid backup json");
        assert_eq!(parsed.schema_version, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn safe_mode_refuses_manual_backups() {
        let dir = temp_config_dir();
        let path = dir.join("config.json");
        std::fs::write(&path, b"garbage").expect("corrupt");
        let controller = LibraryController::load(Some(path));
        assert!(controller.backup_now().is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_respects_scope_and_ranks_by_match() {
        let mut controller = controller();
        let active_count = controller
            .library()
            .active_group()
            .map(|group| group.item_count())
            .unwrap_or(0);
        assert_eq!(
            controller.search_items("示例", false).len(),
            active_count,
            "default scope is the active group"
        );
        let all = controller.search_items("示例", true);
        assert_eq!(all.len(), 6, "all-groups scope covers the sample library");
        assert!(
            all.iter().all(|row| row.scope.contains(" · ")),
            "result rows carry group · category scope"
        );

        let mut draft = ItemDraft::empty();
        draft.name = "Chrome".to_string();
        draft.path = "C:\\Program Files\\Chrome.exe".to_string();
        controller.add_item(draft).expect("in-memory add");
        let results = controller.search_items("chrome", false);
        assert_eq!(results[0].name, "Chrome", "exact match ranks first");
        assert_eq!(results[0].type_label, ItemKind::Exe.label());

        let path_hits = controller.search_items("notepad", false);
        assert!(
            path_hits.iter().any(|row| row.name == "应用示例"),
            "path matching finds the item"
        );
        assert!(controller.search_items("no-such-item-xyz", true).is_empty());

        // An all-group result must be launchable even though it is outside the
        // active category (P2-01).
        let other_category = controller.library().groups[1].categories[0].id.clone();
        let mut beta = ItemDraft::empty();
        beta.name = "BetaItem".to_string();
        beta.path = "C:\\Beta.exe".to_string();
        let beta_id = controller
            .add_item_to_category(&other_category, beta)
            .expect("add to other group");
        assert!(
            controller.search_items("BetaItem", false).is_empty(),
            "the other group stays out of the default scope"
        );
        let all_beta = controller.search_items("BetaItem", true);
        assert_eq!(all_beta.len(), 1);
        assert_eq!(all_beta[0].id, beta_id);
        assert!(
            controller.item(&beta_id).is_some(),
            "an all-group search result must be launchable"
        );

        // P2-02: full pinyin and initials find Chinese names; a rename
        // rebuilds the cached index.
        let mut pinyin = ItemDraft::empty();
        pinyin.name = "微信".to_string();
        pinyin.path = "C:\\WeChat.exe".to_string();
        let wechat = controller.add_item(pinyin).expect("add wechat");
        let full = controller.search_items("weixin", false);
        assert_eq!(full[0].name, "微信", "full pinyin matches");
        let initials = controller.search_items("wx", false);
        assert!(
            initials.iter().any(|row| row.name == "微信"),
            "initial pinyin matches"
        );
        let mut renamed = controller.item_draft(&wechat).expect("draft");
        renamed.name = "音乐".to_string();
        controller.update_item(&wechat, renamed).expect("rename");
        assert!(
            controller
                .search_items("weixin", false)
                .iter()
                .all(|row| row.name != "音乐"),
            "the old pinyin index is gone after the rename"
        );
        assert!(
            controller
                .search_items("yinyue", false)
                .iter()
                .any(|row| row.name == "音乐"),
            "the renamed item gets a fresh polyphone-aware index"
        );
    }

    #[test]
    fn external_drop_adds_to_the_requested_category() {
        let mut controller = controller();
        let (group_id, active_id, target_id, target_name) = {
            let library = controller.library();
            let group = &library.groups[0];
            let active = library.active_category().expect("active").id.clone();
            let target = &group.categories[1];
            (
                group.id.clone(),
                active,
                target.id.clone(),
                target.name.clone(),
            )
        };
        let draft = ItemDraft::from_path(std::path::Path::new("C:\\Temp\\dropped.txt"));
        let id = controller
            .add_item_to_category(&target_id, draft)
            .expect("drop add");
        {
            let library = controller.library();
            let target = library
                .group(&group_id)
                .expect("group")
                .category(&target_id)
                .expect("category");
            assert!(target.items.iter().any(|item| item.id == id));
        }
        assert_eq!(
            controller.active_category_id().as_deref(),
            Some(active_id.as_str()),
            "the drop does not change the visible selection"
        );
        assert_eq!(
            controller.category_name(&target_id).as_deref(),
            Some(target_name.as_str())
        );
        assert!(
            controller
                .add_item_to_category("missing", ItemDraft::empty())
                .is_err()
        );
    }
}
