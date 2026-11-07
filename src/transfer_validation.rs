//! P2-17 (baseline 17.4/15.7/8.4): end-to-end export/import validation.
//!
//! Runs the real controller against a temporary configuration directory:
//! exports all and one group, imports a merge document (which carries a
//! command item) and an overwrite document, and records every observation in
//! a TSV file. No window is created, so the acceptance script can verify the
//! logic deterministically while the live script covers the UI wiring.

use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use quicknest::config_transfer::{ImportMode, parse_import};
use quicknest::library_controller::LibraryController;
use quicknest::library_model::{Category, Group, Item, ItemKind, Library};

fn temp_dir() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir =
        std::env::temp_dir().join(format!("quicknest-transfer-{}-{stamp}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    dir
}

fn seed_library() -> Library {
    let mut library = Library::default();
    let mut tools = Group::new("g-tools", "Tools");
    let mut category = Category::new("c-main", "Main");
    category.items.push(Item::new(
        "i-note",
        "Notepad",
        ItemKind::Exe,
        "C:/Windows/System32/notepad.exe",
    ));
    tools.categories.push(category);
    tools.last_category_id = Some("c-main".to_string());
    let mut media = Group::new("g-media", "Media");
    media.categories.push(Category::new("c-play", "Play"));
    media.last_category_id = Some("c-play".to_string());
    library.groups.push(tools);
    library.groups.push(media);
    library.current_group_id = Some("g-tools".to_string());
    library
}

fn item_count(library: &Library) -> usize {
    library
        .groups
        .iter()
        .flat_map(|group| &group.categories)
        .map(|category| category.items.len())
        .sum()
}

/// Runs the transfer validation and writes `elapsed_ms\tphase\tdetail` rows.
pub fn run(report_path: &Path) -> Result<(), Box<dyn Error>> {
    let started = std::time::Instant::now();
    let mut report = fs::File::create(report_path)?;
    let mut row = |phase: &str, detail: &str| -> Result<(), Box<dyn Error>> {
        writeln!(
            report,
            "{}\t{}\t{}",
            started.elapsed().as_millis(),
            phase,
            detail
        )?;
        Ok(())
    };

    let dir = temp_dir();
    let config = dir.join("config.json");
    fs::write(&config, serde_json::to_string_pretty(&seed_library())?)?;
    let mut controller = LibraryController::load(Some(config.clone()));
    row(
        "seeded",
        &format!(
            "groups={} items={}",
            controller.library().groups.len(),
            item_count(controller.library())
        ),
    )?;

    // a) Export all: the document carries every group and the settings.
    let all = dir.join("export-all.json");
    let bytes = controller.export_all_to(&all)?;
    let text = fs::read_to_string(&all)?;
    let document = parse_import(&text)?;
    if document.groups.len() != 2 || document.settings.is_none() {
        return Err("export all did not carry both groups and the settings".into());
    }
    row(
        "export_all",
        &format!(
            "bytes={bytes} groups={} settings=true",
            document.groups.len()
        ),
    )?;

    // b) Export one group: only that group, no settings.
    let one = dir.join("export-group.json");
    controller.export_group_to(&one, "g-tools")?;
    let text = fs::read_to_string(&one)?;
    let document = parse_import(&text)?;
    if document.groups.len() != 1 || document.settings.is_some() {
        return Err("group export carried the wrong content".into());
    }
    row(
        "export_group",
        &format!("group={}", document.groups[0].name),
    )?;

    // c) Merge import: a document with a new group and a command item.
    let merge = dir.join("import-merge.json");
    fs::write(
        &merge,
        r#"{
            "format": "quicknest-export", "version": 1, "schema_version": 2, "scope": "group",
            "groups": [{
                "id": "xg", "name": "Imported", "last_category_id": "xc",
                "categories": [{
                    "id": "xc", "name": "Cat",
                    "items": [{
                        "id": "xi", "name": "Run", "item_type": "command",
                        "window_state": "normal",
                        "launch": {"path": "cmd.exe", "args": ["/c", "echo hi"]}
                    }]
                }]
            }]
        }"#,
    )?;
    let backups_before = count_backups(&dir)?;
    let report_row = controller.import_from(&merge, ImportMode::Merge)?;
    let names: Vec<String> = controller
        .library()
        .groups
        .iter()
        .map(|group| group.name.clone())
        .collect();
    if !names.iter().any(|name| name == "Imported") || !names.iter().any(|name| name == "Tools") {
        return Err(format!("merge groups: {}", names.join(",")).into());
    }
    let imported = controller
        .library()
        .groups
        .iter()
        .find(|group| group.name == "Imported")
        .ok_or("imported group missing")?;
    let command = imported
        .categories
        .first()
        .and_then(|category| category.items.first())
        .ok_or("imported command item missing")?;
    if command.item_type != ItemKind::Command || !command.launch.confirm_on_first_run {
        return Err("imported command item is not marked unverified".into());
    }
    let backups_after = count_backups(&dir)?;
    if backups_after <= backups_before {
        return Err("no pre-import backup was written".into());
    }
    row(
        "merge_import",
        &format!(
            "groups={} added={} unverified={} backups={backups_before}->{backups_after}",
            names.join(","),
            report_row.groups_added,
            report_row.unverified_items
        ),
    )?;

    // d) Overwrite import: groups and settings are replaced.
    let overwrite = dir.join("import-overwrite.json");
    fs::write(
        &overwrite,
        r#"{
            "format": "quicknest-export", "version": 1, "schema_version": 2, "scope": "all",
            "settings": {"theme": "dark"},
            "groups": [{"id": "og", "name": "Replaced", "last_category_id": "oc",
                "categories": [{"id": "oc", "name": "Cat", "items": []}]}]
        }"#,
    )?;
    controller.import_from(&overwrite, ImportMode::Overwrite)?;
    let names: Vec<String> = controller
        .library()
        .groups
        .iter()
        .map(|group| group.name.clone())
        .collect();
    if names != vec!["Replaced".to_string()] || controller.library().settings.theme != "dark" {
        return Err(format!(
            "overwrite result: {} theme={}",
            names.join(","),
            controller.library().settings.theme
        )
        .into());
    }
    row(
        "overwrite_import",
        &format!(
            "groups={} theme={}",
            names.join(","),
            controller.library().settings.theme
        ),
    )?;

    // e) A broken document is rejected and the live configuration is kept.
    let broken = dir.join("import-broken.json");
    fs::write(&broken, "not json")?;
    let rejected = controller.import_from(&broken, ImportMode::Merge).is_err();
    if !rejected {
        return Err("a broken document was accepted".into());
    }
    if controller.library().groups.len() != 1 {
        return Err("the rejected import changed the library".into());
    }
    row("broken_rejected", "library unchanged")?;

    row("completed", "all P2-17 transfer phases passed")?;
    let _ = fs::remove_dir_all(&dir);
    Ok(())
}

fn count_backups(dir: &Path) -> Result<usize, Box<dyn Error>> {
    let mut count = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("config.backup.") && name.ends_with(".json") {
            count += 1;
        }
    }
    Ok(count)
}
