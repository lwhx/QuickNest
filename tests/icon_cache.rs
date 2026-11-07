//! Icon cache end-to-end test in its own process (Shell COM extraction plus
//! the P1-11 disk cache). A second service must reuse the snapshot without
//! rewriting it.

use quicknest::infrastructure::windows::icons::{
    ICON_SIZE, IconResult, IconService, cache_key, read_cache,
};
use std::time::Duration;

fn wait_result(service: &IconService) -> IconResult {
    for _ in 0..100 {
        if let Some(result) = service.try_recv() {
            return result;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("icon result timed out");
}

#[test]
fn extraction_writes_the_cache_and_a_second_service_reuses_it() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("quicknest-icon-cache-e2e-{stamp}"));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let source = "C:\\Windows\\System32\\cmd.exe";
    let key = cache_key(source, ICON_SIZE);
    let cache_path = dir.join(format!("{key}.qni"));

    {
        let service = IconService::start(dir.clone());
        service.request("probe", source);
        let result = wait_result(&service);
        assert!(result.pixels.is_some(), "icon extraction failed");
    }
    let cached = read_cache(&dir, &key).expect("cache file written");
    let first_mtime = std::fs::metadata(&cache_path)
        .expect("metadata")
        .modified()
        .expect("mtime");

    std::thread::sleep(Duration::from_millis(60));
    {
        let service = IconService::start(dir.clone());
        service.request("probe", source);
        let result = wait_result(&service);
        let pixels = result.pixels.expect("cached icon missing");
        assert_eq!(pixels, cached, "cached snapshot reused");
    }
    let second_mtime = std::fs::metadata(&cache_path)
        .expect("metadata")
        .modified()
        .expect("mtime");
    assert_eq!(
        first_mtime, second_mtime,
        "cache hit must not rewrite the snapshot"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
