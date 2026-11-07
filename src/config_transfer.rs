//! P2-17 (baseline 17.4/15.7/8.4): configuration export and import.
//!
//! Pure document logic without Win32 or file IO, so every rule is unit
//! testable: the export document shape, the acceptance of both an export
//! wrapper and a plain configuration file, schema gating, id regeneration and
//! name collision handling on merge, and the "imported command items are
//! unverified" rule from baseline 8.4.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::library_model::{Category, Group, Item, ItemKind, LaunchCombination, Library, Settings};

/// Marks the export wrapper so an import can tell an export from a plain
/// configuration file.
pub const EXPORT_FORMAT: &str = "quicknest-export";
/// Version of the export wrapper itself (not the library schema).
pub const EXPORT_VERSION: u32 = 1;
/// The only library schema this build reads or writes (baseline 15.9).
pub const SUPPORTED_SCHEMA: u32 = 2;

/// What an export contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportScope {
    /// Every group plus the settings.
    All,
    /// One group; settings are not exported.
    Group,
}

impl ExportScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Group => "group",
        }
    }
}

/// The document written by an export (baseline 17.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportDocument {
    pub format: String,
    pub version: u32,
    pub schema_version: u32,
    pub scope: ExportScope,
    /// Only present for a full export.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<Settings>,
    pub groups: Vec<Group>,
    #[serde(default)]
    pub combinations: Vec<LaunchCombination>,
}

/// Builds the document for a full export.
pub fn export_all(library: &Library) -> ExportDocument {
    ExportDocument {
        format: EXPORT_FORMAT.to_string(),
        version: EXPORT_VERSION,
        schema_version: SUPPORTED_SCHEMA,
        scope: ExportScope::All,
        settings: Some(library.settings.clone()),
        groups: library.groups.clone(),
        combinations: library.combinations.clone(),
    }
}

/// Builds the document for a single group; `None` when the group is gone.
pub fn export_group(library: &Library, group_id: &str) -> Option<ExportDocument> {
    let group = library
        .groups
        .iter()
        .find(|group| group.id == group_id)?
        .clone();
    Some(ExportDocument {
        format: EXPORT_FORMAT.to_string(),
        version: EXPORT_VERSION,
        schema_version: SUPPORTED_SCHEMA,
        scope: ExportScope::Group,
        settings: None,
        combinations: group_combinations(&group, &library.combinations),
        groups: vec![group],
    })
}

// A cross-group combination cannot be exported faithfully with one group.
fn group_combinations(group: &Group, combinations: &[LaunchCombination]) -> Vec<LaunchCombination> {
    let ids: HashSet<&str> = group
        .categories
        .iter()
        .flat_map(|c| &c.items)
        .map(|item| item.id.as_str())
        .collect();
    combinations
        .iter()
        .filter(|combination| {
            !combination.item_ids.is_empty()
                && combination
                    .item_ids
                    .iter()
                    .all(|id| ids.contains(id.as_str()))
        })
        .cloned()
        .collect()
}

/// What an imported document carries, independent of the file layout.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDocument {
    pub scope: ExportScope,
    pub settings: Option<Settings>,
    pub groups: Vec<Group>,
    pub combinations: Vec<LaunchCombination>,
}

/// How an import applies to the live library (baseline 17.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    /// Append the imported groups with fresh ids; settings are kept.
    Merge,
    /// Replace the whole library with the imported document.
    Overwrite,
}

impl ImportMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Overwrite => "overwrite",
        }
    }
}

/// What an import changed, for the user-facing report.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportReport {
    pub mode: Option<ImportMode>,
    pub groups_added: usize,
    pub groups_renamed: usize,
    pub items_added: usize,
    pub unverified_items: usize,
    pub settings_replaced: bool,
}

impl ImportReport {
    pub fn summary(&self) -> String {
        let mode = self.mode.map(ImportMode::as_str).unwrap_or("merge");
        let mut text = format!(
            "导入（{}）：新增 {} 个分组、{} 个项目",
            mode, self.groups_added, self.items_added
        );
        if self.groups_renamed > 0 {
            text.push_str(&format!("，重命名 {} 个同名分组", self.groups_renamed));
        }
        if self.unverified_items > 0 {
            text.push_str(&format!(
                "，{} 个命令项目标记为未验证",
                self.unverified_items
            ));
        }
        if self.settings_replaced {
            text.push_str("，设置已替换");
        }
        text
    }
}

/// Parses an export wrapper or a plain configuration document.
///
/// Both shapes are accepted so a user can import either a file produced by
/// "导出" or a copy of `config.json`. Schema versions other than 2 are
/// rejected instead of being migrated destructively (baseline 15.9).
pub fn parse_import(text: &str) -> Result<ImportDocument, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("不是有效的 JSON：{error}"))?;
    let format = value
        .get("format")
        .and_then(|format| format.as_str())
        .unwrap_or("");
    if format == EXPORT_FORMAT {
        let version = value
            .get("version")
            .and_then(|version| version.as_u64())
            .unwrap_or(0);
        if version != EXPORT_VERSION as u64 {
            return Err(format!(
                "导出文件版本 {version} 与当前版本 {EXPORT_VERSION} 不兼容"
            ));
        }
        let document: ExportDocument =
            serde_json::from_value(value).map_err(|error| format!("导出文件结构无效：{error}"))?;
        if document.schema_version != SUPPORTED_SCHEMA {
            return Err(format!(
                "导出文件的配置版本 {} 与当前版本 {SUPPORTED_SCHEMA} 不兼容",
                document.schema_version
            ));
        }
        return Ok(ImportDocument {
            scope: document.scope,
            settings: if document.scope == ExportScope::All {
                document.settings
            } else {
                None
            },
            groups: document.groups,
            combinations: document.combinations,
        });
    }
    // A plain configuration file.
    let library: Library =
        serde_json::from_value(value).map_err(|error| format!("配置文件结构无效：{error}"))?;
    if library.schema_version != SUPPORTED_SCHEMA {
        return Err(format!(
            "配置版本 {} 与当前版本 {SUPPORTED_SCHEMA} 不兼容，未导入",
            library.schema_version
        ));
    }
    Ok(ImportDocument {
        scope: ExportScope::All,
        settings: Some(library.settings),
        groups: library.groups,
        combinations: library.combinations,
    })
}

/// Applies an import to `library` and returns what changed.
///
/// Merge regenerates every imported id so the result cannot collide with the
/// live data, renames groups whose name is already used, and marks imported
/// command/automation items unverified (baseline 8.4).
pub fn apply_import(
    library: &mut Library,
    document: &ImportDocument,
    mode: ImportMode,
) -> ImportReport {
    match mode {
        ImportMode::Overwrite => apply_overwrite(library, document),
        ImportMode::Merge => apply_merge(library, document),
    }
}

fn apply_overwrite(library: &mut Library, document: &ImportDocument) -> ImportReport {
    let (mut groups, combinations) = normalize_document(document, &Library::default());
    let items_added: usize = groups
        .iter()
        .map(|group| {
            group
                .categories
                .iter()
                .map(|category| category.items.len())
                .sum::<usize>()
        })
        .sum();
    // 修复（2026-11-07 审核）：必须标记即将提交的集合本身——旧代码
    // 标记的是立刻丢弃的克隆，覆盖导入的命令项目会绕过首次执行确认。
    let unverified_items = mark_unverified(&mut groups);
    library.groups = groups;
    library.combinations = combinations;
    if let Some(settings) = &document.settings {
        library.settings = settings.clone();
    }
    library.current_group_id = library.groups.first().map(|group| group.id.clone());
    for group in &mut library.groups {
        if group.last_category_id.is_none() {
            group.last_category_id = group.categories.first().map(|category| category.id.clone());
        }
    }
    let _ = unverified_items;
    ImportReport {
        mode: Some(ImportMode::Overwrite),
        groups_added: library.groups.len(),
        groups_renamed: 0,
        items_added,
        unverified_items: count_unverified(&library.groups),
        settings_replaced: document.settings.is_some(),
    }
}

fn apply_merge(library: &mut Library, document: &ImportDocument) -> ImportReport {
    let (mut imported, combinations) = normalize_document(document, library);
    let unverified_items = mark_unverified(&mut imported);
    let mut report = ImportReport {
        mode: Some(ImportMode::Merge),
        unverified_items,
        ..Default::default()
    };
    let mut existing_names: Vec<String> = library
        .groups
        .iter()
        .map(|group| group.name.to_lowercase())
        .collect();
    for mut group in imported {
        let base = if group.name.trim().is_empty() {
            "导入分组".to_string()
        } else {
            group.name.trim().to_string()
        };
        let name = unique_name(&existing_names, &base);
        if name != base {
            report.groups_renamed += 1;
        }
        existing_names.push(name.to_lowercase());
        group.name = name;
        report.items_added += group
            .categories
            .iter()
            .map(|category| category.items.len())
            .sum::<usize>();
        report.groups_added += 1;
        library.groups.push(group);
    }
    library.combinations.extend(combinations);
    if library.current_group_id.is_none() {
        library.current_group_id = library.groups.first().map(|group| group.id.clone());
    }
    report
}

/// Allocate once at application time, repairing every reference from the same
/// mapping. Ambiguous duplicate source item ids are deliberately not resolved.
fn normalize_document(
    document: &ImportDocument,
    existing: &Library,
) -> (Vec<Group>, Vec<LaunchCombination>) {
    let mut used: HashSet<String> = existing
        .groups
        .iter()
        .flat_map(|group| {
            std::iter::once(group.id.clone()).chain(group.categories.iter().flat_map(|category| {
                std::iter::once(category.id.clone())
                    .chain(category.items.iter().map(|item| item.id.clone()))
            }))
        })
        .chain(existing.combinations.iter().map(|c| c.id.clone()))
        .collect();
    let mut fresh = || loop {
        let id = crate::library_model::new_uuid();
        if used.insert(id.clone()) {
            break id;
        }
    };
    let mut item_ids: HashMap<String, Option<String>> = HashMap::new();
    let mut groups = document.groups.clone();
    for group in &mut groups {
        group.id = fresh();
        let previous_category = group.last_category_id.clone();
        let mut selected = None;
        for category in &mut group.categories {
            let old_id = std::mem::replace(&mut category.id, fresh());
            if previous_category.as_ref() == Some(&old_id) {
                selected = Some(category.id.clone());
            }
            for item in &mut category.items {
                let old_id = std::mem::replace(&mut item.id, fresh());
                item_ids
                    .entry(old_id)
                    .and_modify(|id| *id = None)
                    .or_insert_with(|| Some(item.id.clone()));
            }
        }
        group.last_category_id =
            selected.or_else(|| group.categories.first().map(|c| c.id.clone()));
    }
    let combinations = document
        .combinations
        .iter()
        .filter_map(|combination| {
            let mapped: Option<Vec<String>> = combination
                .item_ids
                .iter()
                .map(|id| item_ids.get(id).cloned().flatten())
                .collect();
            let item_ids = mapped?;
            if item_ids.is_empty() {
                return None;
            }
            Some(LaunchCombination {
                id: fresh(),
                name: combination.name.clone(),
                item_ids,
            })
        })
        .collect();
    (groups, combinations)
}

/// Returns `<base> (导入)`, `<base> (导入 2)`, ... until the name is unused.
fn unique_name(existing: &[String], base: &str) -> String {
    let base = base.trim();
    if !existing.iter().any(|name| name.eq_ignore_ascii_case(base)) {
        return base.to_string();
    }
    for index in 1..1000 {
        let candidate = if index == 1 {
            format!("{base} (导入)")
        } else {
            format!("{base} (导入 {index})")
        };
        if !existing
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&candidate))
        {
            return candidate;
        }
    }
    format!("{base} (导入 {})", crate::library_model::new_uuid())
}

fn is_command_like(kind: ItemKind) -> bool {
    matches!(kind, ItemKind::Command | ItemKind::Automation)
}

/// Marks every command/automation item of the groups as unverified and
/// returns how many were marked (baseline 8.4).
fn mark_unverified(groups: &mut [Group]) -> usize {
    let mut marked = 0;
    for group in groups {
        for category in &mut group.categories {
            for item in &mut category.items {
                if is_command_like(item.item_type) {
                    item.launch.confirm_on_first_run = true;
                    marked += 1;
                }
            }
        }
    }
    marked
}

fn count_unverified(groups: &[Group]) -> usize {
    groups
        .iter()
        .flat_map(|group| &group.categories)
        .flat_map(|category: &Category| &category.items)
        .filter(|item: &&Item| is_command_like(item.item_type) && item.launch.confirm_on_first_run)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library_model::{Item, Library};

    fn item(id: &str, kind: ItemKind, target: &str) -> Item {
        Item::new(id, format!("item-{id}"), kind, target)
    }

    fn library_with_one_group() -> Library {
        let mut library = Library::default();
        let mut group = crate::library_model::Group::new("g1", "Alpha");
        let mut category = crate::library_model::Category::new("c1", "Main");
        category.items.push(item(
            "seed",
            ItemKind::Exe,
            "C:/Windows/System32/notepad.exe",
        ));
        group.categories.push(category);
        group.last_category_id = Some("c1".to_string());
        library.groups.push(group);
        library.current_group_id = Some("g1".to_string());
        library
    }

    #[test]
    fn export_all_carries_groups_and_settings() {
        let library = library_with_one_group();
        let document = export_all(&library);
        assert_eq!(document.format, EXPORT_FORMAT);
        assert_eq!(document.scope, ExportScope::All);
        assert_eq!(document.schema_version, SUPPORTED_SCHEMA);
        assert_eq!(document.groups.len(), library.groups.len());
        assert_eq!(document.settings.as_ref(), Some(&library.settings));
    }

    #[test]
    fn export_group_carries_only_that_group_without_settings() {
        let library = library_with_one_group();
        let first = library.groups[0].id.clone();
        let document = export_group(&library, &first).expect("export");
        assert_eq!(document.scope, ExportScope::Group);
        assert_eq!(document.groups.len(), 1);
        assert_eq!(document.groups[0].id, first);
        assert!(document.settings.is_none());
        assert!(export_group(&library, "missing").is_none());
    }

    #[test]
    fn export_round_trips_through_parse_import() {
        let library = library_with_one_group();
        let json = serde_json::to_string(&export_all(&library)).expect("serialize");
        let imported = parse_import(&json).expect("parse");
        assert_eq!(imported.scope, ExportScope::All);
        assert_eq!(imported.groups.len(), library.groups.len());
        assert!(imported.settings.is_some());
    }

    #[test]
    fn plain_configuration_files_are_accepted() {
        let library = library_with_one_group();
        let json = serde_json::to_string(&library).expect("serialize");
        let imported = parse_import(&json).expect("parse");
        assert_eq!(imported.scope, ExportScope::All);
        assert_eq!(imported.groups.len(), library.groups.len());
    }

    #[test]
    fn unsupported_schema_and_garbage_are_rejected() {
        let mut library = library_with_one_group();
        library.schema_version = 3;
        let json = serde_json::to_string(&library).expect("serialize");
        let error = parse_import(&json).expect_err("schema 3 rejected");
        assert!(error.contains("不兼容"), "{error}");
        assert!(parse_import("not json").is_err());
        assert!(parse_import("{}").is_err());
    }

    #[test]
    fn export_wrapper_version_is_checked() {
        let library = library_with_one_group();
        let mut document = export_all(&library);
        document.version = 99;
        let json = serde_json::to_string(&document).expect("serialize");
        assert!(parse_import(&json).is_err());
    }

    #[test]
    fn merge_regenerates_ids_and_renames_collisions() {
        let mut library = library_with_one_group();
        let existing_name = library.groups[0].name.clone();
        let existing_id = library.groups[0].id.clone();
        let mut document = export_all(&library);
        document.groups[0]
            .categories
            .first_mut()
            .expect("category")
            .items
            .push(item(
                "i1",
                ItemKind::Exe,
                "C:\\Windows\\System32\\notepad.exe",
            ));
        let imported =
            parse_import(&serde_json::to_string(&document).expect("json")).expect("parse");
        let report = apply_import(&mut library, &imported, ImportMode::Merge);
        assert_eq!(report.groups_added, 1);
        assert_eq!(report.groups_renamed, 1, "same name is renamed");
        // The exported group already held the seed item; the pushed one is the second.
        assert_eq!(report.items_added, 2);
        let merged = library.groups.last().expect("merged group");
        assert!(merged.name.starts_with(&existing_name) && merged.name.contains("导入"));
        assert_ne!(merged.id, existing_id, "ids are regenerated");
        let merged_item = &merged.categories[0].items[0];
        assert_ne!(merged_item.id, "i1", "item ids are regenerated");
    }

    #[test]
    fn imported_command_items_are_marked_unverified() {
        let mut library = library_with_one_group();
        let mut document = export_group(&library, &library.groups[0].id).expect("export");
        document.groups[0].categories[0]
            .items
            .push(item("cmd", ItemKind::Command, "cmd.exe"));
        document.groups[0].categories[0].items.push(item(
            "auto",
            ItemKind::Automation,
            "powershell.exe",
        ));
        document.groups[0].categories[0].items.push(item(
            "exe",
            ItemKind::Exe,
            "C:\\Windows\\System32\\notepad.exe",
        ));
        let imported =
            parse_import(&serde_json::to_string(&document).expect("json")).expect("parse");
        let report = apply_import(&mut library, &imported, ImportMode::Merge);
        assert_eq!(report.unverified_items, 2, "command + automation");
        let group = library.groups.last().expect("group");
        let items = &group.categories[0].items;
        // [0] is the exported seed EXE, then the command, the automation and
        // a plain EXE in the order they were pushed.
        assert!(
            !items[0].launch.confirm_on_first_run,
            "plain EXE is verified"
        );
        assert!(
            items[1].launch.confirm_on_first_run,
            "command is unverified"
        );
        assert!(
            items[2].launch.confirm_on_first_run,
            "automation is unverified"
        );
        assert!(
            !items[3].launch.confirm_on_first_run,
            "plain EXE is verified"
        );
    }

    #[test]
    fn overwrite_replaces_groups_and_settings() {
        let mut library = library_with_one_group();
        let mut document = export_all(&library);
        document.settings.as_mut().expect("settings").theme = "dark".to_string();
        document.groups[0].name = "Replaced".to_string();
        let imported =
            parse_import(&serde_json::to_string(&document).expect("json")).expect("parse");
        let report = apply_import(&mut library, &imported, ImportMode::Overwrite);
        assert_eq!(report.groups_added, 1);
        assert_eq!(library.groups.len(), 1);
        assert_eq!(library.groups[0].name, "Replaced");
        assert_eq!(library.settings.theme, "dark");
    }

    #[test]
    fn combinations_round_trip_plain_and_full_with_repaired_ids() {
        let mut source = library_with_one_group();
        source
            .combinations
            .push(LaunchCombination::new("Run", vec!["seed".into()]));
        for json in [
            serde_json::to_string(&source).unwrap(),
            serde_json::to_string(&export_all(&source)).unwrap(),
        ] {
            let document = parse_import(&json).unwrap();
            assert_eq!(document.combinations.len(), 1);
            let mut target = source.clone();
            let old = target.combinations[0].id.clone();
            apply_import(&mut target, &document, ImportMode::Merge);
            apply_import(&mut target, &document, ImportMode::Merge);
            assert_eq!(target.combinations.len(), 3);
            let ids: HashSet<_> = target.combinations.iter().map(|c| &c.id).collect();
            assert_eq!(ids.len(), 3);
            assert_eq!(target.combinations[0].id, old);
            for index in 1..3 {
                assert_eq!(
                    target.combinations[index].item_ids,
                    vec![target.groups[index].categories[0].items[0].id.clone()]
                );
                assert_ne!(target.combinations[index].item_ids[0], "seed");
            }
            apply_import(&mut target, &document, ImportMode::Overwrite);
            assert_eq!(target.combinations.len(), 1);
            assert_eq!(
                target.combinations[0].item_ids,
                vec![target.groups[0].categories[0].items[0].id.clone()]
            );
            assert_ne!(target.combinations[0].id, old);
        }
    }

    #[test]
    fn group_export_excludes_cross_group_dangling_and_empty_combinations() {
        let mut source = library_with_one_group();
        source.combinations = vec![
            LaunchCombination::new("local", vec!["seed".into()]),
            LaunchCombination::new("cross", vec!["seed".into(), "other".into()]),
            LaunchCombination::new("empty", vec![]),
        ];
        let exported = export_group(&source, "g1").unwrap();
        assert_eq!(exported.combinations.len(), 1);
        assert_eq!(exported.combinations[0].name, "local");
        let parsed = parse_import(&serde_json::to_string(&exported).unwrap()).unwrap();
        let mut target = Library::default();
        apply_import(&mut target, &parsed, ImportMode::Overwrite);
        assert_eq!(
            target.combinations[0].item_ids[0],
            target.groups[0].categories[0].items[0].id
        );
    }

    #[test]
    fn overwrite_legacy_document_clears_old_combinations_and_preserves_category_selection() {
        let mut source = library_with_one_group();
        source.groups[0]
            .categories
            .push(Category::new("c2", "Second"));
        source.groups[0].last_category_id = Some("c2".into());
        let mut value = serde_json::to_value(export_all(&source)).unwrap();
        value.as_object_mut().unwrap().remove("combinations");
        let parsed = parse_import(&value.to_string()).unwrap();
        source
            .combinations
            .push(LaunchCombination::new("old", vec!["seed".into()]));
        apply_import(&mut source, &parsed, ImportMode::Overwrite);
        assert!(source.combinations.is_empty());
        assert_eq!(
            source.groups[0].last_category_id.as_ref(),
            Some(&source.groups[0].categories[1].id)
        );
    }

    #[test]
    fn ambiguous_or_missing_combination_references_never_bind_to_live_items() {
        let mut source = library_with_one_group();
        source.groups[0].categories[0]
            .items
            .push(item("seed", ItemKind::Exe, "other.exe"));
        source.combinations = vec![
            LaunchCombination::new("ambiguous", vec!["seed".into()]),
            LaunchCombination::new("missing", vec!["missing".into()]),
        ];
        let parsed = parse_import(&serde_json::to_string(&source).unwrap()).unwrap();
        let mut target = library_with_one_group();
        apply_import(&mut target, &parsed, ImportMode::Merge);
        assert!(target.combinations.is_empty());
        let items = &target.groups[1].categories[0].items;
        assert_ne!(items[0].id, items[1].id);
    }

    #[test]
    fn unique_name_counts_up() {
        let existing = vec!["alpha".to_string(), "alpha (导入)".to_string()];
        assert_eq!(unique_name(&existing, "Alpha"), "Alpha (导入 2)");
        assert_eq!(unique_name(&existing, "beta"), "beta");
    }
}
