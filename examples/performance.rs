//! Isolated Release measurements. No GUI, application launches or user config.
//! `cargo run --locked --offline --release --example performance -- --out result.json`
use std::hint::black_box;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use quicknest::infrastructure::windows::config_save::prune_backups;
use quicknest::library_controller::LibraryController;
use quicknest::library_model::{Item, ItemKind, Library, new_uuid};
use serde_json::{Value, json};

struct OwnedRun(PathBuf);
impl OwnedRun {
    fn create() -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!("quicknest-performance-{}", new_uuid()));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for OwnedRun {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}
fn median_ms(mut work: impl FnMut(), count: usize) -> f64 {
    let mut samples = Vec::with_capacity(count);
    for _ in 0..count {
        let start = Instant::now();
        work();
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    median(samples)
}
fn persistence_fixture(count: usize) -> Library {
    let mut library = Library::default();
    let group = library
        .add_group("Optimization fixture", "fixture-group")
        .unwrap();
    library.add_category(&group, "Selected", "fixture-category");
    library.add_category(&group, "Other", "fixture-other");
    let category = library
        .group_mut(&group)
        .unwrap()
        .category_mut("fixture-category")
        .unwrap();
    category.items = (0..count).map(|index| {
        let mut item = Item::new(format!("fixture-item-{index}"), format!("Synthetic tool {index}"), ItemKind::Exe,
            format!("C:/QuickNestOptimizationFixture/tool-{index}.exe"));
        item.note = "Synthetic data used only to time cloning, JSON serialization and temporary-file persistence.".repeat(2);
        item
    }).collect();
    library.select_group(&group);
    library.select_category("fixture-category");
    library
}
fn row_fixture(count: usize) -> Library {
    let mut library = Library::default();
    let group = library.add_group("Row fixture", "row-group").unwrap();
    library.add_category(&group, "Rows", "row-category");
    library
        .group_mut(&group)
        .unwrap()
        .category_mut("row-category")
        .unwrap()
        .items = (0..count)
        .map(|index| {
            Item::new(
                format!("row-item-{index}"),
                format!("Synthetic row {index}"),
                ItemKind::Uri,
                format!("https://example.invalid/fixture/{index}"),
            )
        })
        .collect();
    library.select_group(&group);
    library.select_category("row-category");
    library
}
fn memory_controller(library: Library) -> LibraryController {
    let mut controller = LibraryController::load(None);
    *controller.library_mut() = library;
    controller
}
fn backup_names(path: &Path) -> Vec<String> {
    let mut names: Vec<_> = std::fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("config.backup."))
        .collect();
    names.sort();
    names
}
fn persistence_measurements(run: &OwnedRun) -> Vec<Value> {
    let mut results = Vec::new();
    for count in [100usize, 1000, 5000] {
        let case = run.0.join(count.to_string());
        std::fs::create_dir(&case).unwrap();
        let config = case.join("config.json");
        let library = persistence_fixture(count);
        let text = serde_json::to_string_pretty(&library).unwrap();
        std::fs::write(&config, &text).unwrap();
        let clone_ms = median_ms(
            || {
                black_box(library.clone());
            },
            15,
        );
        let serialize_ms = median_ms(
            || {
                black_box(serde_json::to_string_pretty(&library).unwrap());
            },
            7,
        );
        let mut controller = LibraryController::load(Some(config.clone()));
        for _ in 0..11 {
            controller.record_launch("fixture-item-0");
            assert!(controller.save_error().is_none());
        }
        let record_ms = median_ms(
            || {
                controller.record_launch("fixture-item-0");
                assert!(controller.save_error().is_none());
            },
            5,
        );
        let prune_ms = median_ms(
            || {
                black_box(prune_backups(&config, 10));
            },
            5,
        );
        let before_text = std::fs::read(&config).unwrap();
        let before_names = backup_names(&case);
        let reselect_ms = median_ms(
            || {
                assert!(controller.select_category("fixture-category"));
            },
            3,
        );
        assert_eq!(std::fs::read(&config).unwrap(), before_text);
        let new_names = backup_names(&case)
            .iter()
            .filter(|name| !before_names.contains(name))
            .count();
        let result = json!({"items":count,"fixture_digest_algorithm":"blake3","fixture_digest":blake3::hash(text.as_bytes()).to_hex().to_string(),
            "json_bytes":text.len(),"clone_ms":clone_ms,"serialize_ms":serialize_ms,"record_and_save_ms":record_ms,
            "prune_only_ms":prune_ms,"reselect_current_ms":reselect_ms,"new_backups_from_three_reselects":new_names});
        println!("persistence {result}");
        results.push(result);
    }
    results
}
fn row_measurements() -> Vec<Value> {
    let mut results = Vec::new();
    for count in [100usize, 1000, 5000, 10000] {
        let library = row_fixture(count);
        let digest = blake3::hash(serde_json::to_string_pretty(&library).unwrap().as_bytes())
            .to_hex()
            .to_string();
        let controller = memory_controller(library);
        let elapsed = median_ms(
            || {
                let rows = black_box(controller.item_rows());
                assert_eq!(rows.len(), count);
                assert_eq!(rows[0].id, "row-item-0");
                assert_eq!(rows[count - 1].id, format!("row-item-{}", count - 1));
            },
            3,
        );
        let result = json!({"items":count,"fixture_digest":digest,"manual_item_rows_ms":elapsed});
        println!("rows {result}");
        results.push(result);
    }
    results
}
fn search_measurements() -> Vec<Value> {
    let mut results = Vec::new();
    for count in [1000usize, 10000] {
        let mut template = row_fixture(count);
        for (index, item) in template.groups[0].categories[0]
            .items
            .iter_mut()
            .enumerate()
        {
            item.name = format!("微信工具{index}");
        }
        let digest = blake3::hash(serde_json::to_string_pretty(&template).unwrap().as_bytes())
            .to_hex()
            .to_string();
        let mut cold_empty = Vec::new();
        let mut cold_pinyin = Vec::new();
        for _ in 0..3 {
            let controller = memory_controller(template.clone());
            let start = Instant::now();
            let rows = black_box(controller.search_items("", false));
            cold_empty.push(start.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(rows.len(), count);
            let controller = memory_controller(template.clone());
            let start = Instant::now();
            let rows = black_box(controller.search_items("wxgj", false));
            cold_pinyin.push(start.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(rows.len(), count);
        }
        let controller = memory_controller(template);
        black_box(controller.search_items("wxgj", false));
        let hot = median_ms(
            || {
                assert_eq!(
                    black_box(controller.search_items("wxgj", false)).len(),
                    count
                );
            },
            7,
        );
        let result = json!({"items":count,"fixture_digest":digest,"cold_empty_ms":median(cold_empty),"cold_pinyin_ms":median(cold_pinyin),"hot_pinyin_ms":hot});
        println!("search {result}");
        results.push(result);
    }
    results
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(debug_assertions) {
        return Err("performance measurements require --release".into());
    }
    let mut args = std::env::args().skip(1);
    let mut output = None;
    let mut label = "working-tree".to_string();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => output = Some(PathBuf::from(args.next().ok_or("--out requires a file")?)),
            "--label" => label = args.next().ok_or("--label requires a value")?,
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    let run = OwnedRun::create()?;
    println!("Isolated warm-cache Release measurements; no GUI, network, real config or launches.");
    let report = json!({"schema_version":1,"package_version":env!("CARGO_PKG_VERSION"),"label":label,
        "started_unix_ms":SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),"profile":"release",
        "persistence_samples":5,"row_samples":3,"cold_search_samples":3,"hot_search_samples":7,
        "persistence":persistence_measurements(&run),"rows":row_measurements(),"search":search_measurements()});
    if let Some(output) = output {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?;
        file.write_all(serde_json::to_string_pretty(&report)?.as_bytes())?;
        file.sync_all()?;
    }
    Ok(())
}
