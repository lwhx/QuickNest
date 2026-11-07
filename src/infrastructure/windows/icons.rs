//! P1-10 icon extraction (baseline 12.1).
//!
//! `IShellItemImageFactory::GetImage` (64x64, icon-only) runs on a dedicated
//! background worker thread; `SHGetFileInfoW` is the compatibility fallback.
//! Results are straight-alpha RGBA buffers that the UI thread turns into
//! `slint::Image`s, so extraction and encoding never block the UI (baseline
//! 5.4/12.1). On-disk hash caching is P1-11.

use std::sync::mpsc::{Receiver, Sender, channel};

use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits,
    GetObjectW, HGDIOBJ, ReleaseDC,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
};
use windows::core::PCWSTR;

/// Baseline 12.1 requests 64x64 or 256x256; the grid's largest preset is 56px.
pub const ICON_SIZE: u32 = 64;

const CACHE_MAGIC: &[u8; 4] = b"QNI1";

/// Normalizes an icon source for the cache key: case-insensitive drive paths
/// and forward slashes collapse to the same identity (baseline 12.2).
fn normalize_source(source: &str) -> String {
    source.trim().replace('/', "\\").to_lowercase()
}

/// BLAKE3 cache key (baseline 12.2): normalized icon source, file modification
/// time and size (or `missing`), plus the requested size. Custom icons use
/// their own file identity through the same function. Program updates change
/// the key, so stale entries stop being referenced and are pruned.
pub fn cache_key(source: &str, size: u32) -> String {
    let normalized = normalize_source(source);
    let mut hasher = blake3::Hasher::new();
    hasher.update(normalized.as_bytes());
    hasher.update(&[0]);
    match std::fs::metadata(&normalized) {
        Ok(metadata) => {
            hasher.update(b"file");
            if let Ok(modified) = metadata.modified()
                && let Ok(elapsed) = modified.duration_since(std::time::UNIX_EPOCH)
            {
                hasher.update(&elapsed.as_nanos().to_le_bytes());
            }
            hasher.update(&metadata.len().to_le_bytes());
        }
        Err(_) => {
            hasher.update(b"missing");
        }
    }
    hasher.update(&size.to_le_bytes());
    hasher.finalize().to_hex().to_string()
}

fn cache_file(cache_dir: &std::path::Path, key: &str) -> std::path::PathBuf {
    cache_dir.join(format!("{key}.qni"))
}

/// Reads a cached icon snapshot (`QNI1`, width, height, RGBA). Corrupt or
/// truncated files are ignored so the icon is simply re-extracted.
pub fn read_cache(cache_dir: &std::path::Path, key: &str) -> Option<IconPixels> {
    let bytes = std::fs::read(cache_file(cache_dir, key)).ok()?;
    if bytes.len() < 12 || &bytes[0..4] != CACHE_MAGIC {
        return None;
    }
    let width = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    let height = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    let expected = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    let rgba = bytes.get(12..12 + expected)?;
    Some(IconPixels {
        width,
        height,
        rgba: rgba.to_vec(),
    })
}

/// Writes an icon snapshot; failures are ignored by callers (baseline 12.3:
/// cache problems must not affect display).
pub fn write_cache(
    cache_dir: &std::path::Path,
    key: &str,
    pixels: &IconPixels,
) -> std::io::Result<()> {
    std::fs::create_dir_all(cache_dir)?;
    let mut bytes = Vec::with_capacity(12 + pixels.rgba.len());
    bytes.extend_from_slice(CACHE_MAGIC);
    bytes.extend_from_slice(&pixels.width.to_le_bytes());
    bytes.extend_from_slice(&pixels.height.to_le_bytes());
    bytes.extend_from_slice(&pixels.rgba);
    std::fs::write(cache_file(cache_dir, key), bytes)
}

/// Removes cache files not referenced by the current library (baseline 12.3:
/// deleted items stop referencing icons; shared icons survive while any item
/// still references them). Returns the number of removed files.
pub fn prune_cache(cache_dir: &std::path::Path, keep: &std::collections::HashSet<String>) -> usize {
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("qni") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if !keep.contains(stem) && std::fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Reads the cached icon or extracts and caches it (background worker only).
pub fn load_or_extract(cache_dir: &std::path::Path, source: &str, size: u32) -> Option<IconPixels> {
    if source.trim().is_empty() {
        return None;
    }
    let key = cache_key(source, size);
    if let Some(pixels) = read_cache(cache_dir, &key) {
        return Some(pixels);
    }
    let pixels = extract_icon(source, size)?;
    let _ = write_cache(cache_dir, &key, &pixels);
    Some(pixels)
}

/// Straight-alpha RGBA pixels of one extracted icon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconPixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug)]
pub struct IconResult {
    pub key: String,
    /// None means extraction failed; the UI keeps its default icon.
    pub pixels: Option<IconPixels>,
}

/// Background icon extraction worker. Requests are (cache key, icon source);
/// the worker owns one STA for the lifetime of the thread.
pub struct IconService {
    requests: Sender<(String, String)>,
    results: Receiver<IconResult>,
    _worker: std::thread::JoinHandle<()>,
}

impl IconService {
    pub fn start(cache_dir: std::path::PathBuf) -> Self {
        let (request_tx, request_rx) = channel::<(String, String)>();
        let (result_tx, result_rx) = channel::<IconResult>();
        let worker = std::thread::Builder::new()
            .name("quicknest-icons".into())
            .spawn(move || {
                // Shell COM calls need an apartment on this worker thread.
                let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok() };
                while let Ok((key, source)) = request_rx.recv() {
                    // Baseline 12.2/12.3: reuse the on-disk snapshot when the
                    // key still matches, otherwise extract and cache it.
                    let pixels = load_or_extract(&cache_dir, &source, ICON_SIZE);
                    if result_tx.send(IconResult { key, pixels }).is_err() {
                        break;
                    }
                }
                if initialized {
                    unsafe { CoUninitialize() };
                }
            })
            .expect("icon worker thread");
        Self {
            requests: request_tx,
            results: result_rx,
            _worker: worker,
        }
    }

    /// Queues an extraction request; duplicate in-flight keys are the caller's
    /// responsibility (the UI keeps a pending set).
    pub fn request(&self, key: &str, source: &str) {
        let _ = self.requests.send((key.to_string(), source.to_string()));
    }

    /// Non-blocking result poll for the UI timer.
    pub fn try_recv(&self) -> Option<IconResult> {
        self.results.try_recv().ok()
    }
}

/// Extracts an icon with `GetImage`, falling back to `SHGetFileInfoW`.
pub fn extract_icon(source: &str, size: u32) -> Option<IconPixels> {
    if source.trim().is_empty() {
        return None;
    }
    // 便携版（基线 23 章）：相对图标路径按便携根解析。
    let resolved = crate::portable::resolve_path(source);
    let resolved_str = resolved.to_string_lossy().into_owned();
    let source: &str = resolved_str.as_str();
    extract_with_image_factory(source, size).or_else(|| extract_with_file_info(source))
}

fn extract_with_image_factory(source: &str, size: u32) -> Option<IconPixels> {
    let wide = wide(source);
    // SAFETY: the wide buffer outlives the call; the bitmap is owned and
    // deleted here.
    unsafe {
        let factory: IShellItemImageFactory =
            SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).ok()?;
        let bitmap = factory
            .GetImage(
                SIZE {
                    cx: size as i32,
                    cy: size as i32,
                },
                SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK,
            )
            .ok()?;
        let pixels = hbitmap_to_pixels(bitmap);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        pixels
    }
}

fn extract_with_file_info(source: &str) -> Option<IconPixels> {
    use windows::Win32::UI::Shell::{SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGetFileInfoW};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    let wide = wide(source);
    let mut info = SHFILEINFOW::default();
    // SAFETY: the wide buffer outlives the call; the icon is destroyed below.
    let result = unsafe {
        SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            Default::default(),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };
    if result == 0 || info.hIcon.is_invalid() {
        return None;
    }
    let pixels = unsafe {
        let mut icon_info = ICONINFO::default();
        if GetIconInfo(info.hIcon, &mut icon_info).is_ok() {
            let pixels = hbitmap_to_pixels(icon_info.hbmColor);
            let _ = DeleteObject(HGDIOBJ(icon_info.hbmColor.0));
            let _ = DeleteObject(HGDIOBJ(icon_info.hbmMask.0));
            pixels
        } else {
            None
        }
    };
    // SAFETY: the icon came from SHGetFileInfoW and is no longer used.
    unsafe {
        let _ = DestroyIcon(info.hIcon);
    }
    pixels
}

/// Converts a 32bpp HBITMAP (premultiplied BGRA, as returned by GetImage) to
/// straight-alpha RGBA. Top-down rows via a negative bitmap height.
fn hbitmap_to_pixels(bitmap: windows::Win32::Graphics::Gdi::HBITMAP) -> Option<IconPixels> {
    if bitmap.is_invalid() {
        return None;
    }
    // SAFETY: GetObjectW fills a BITMAP for this bitmap handle; the DC is
    // released and the buffer is sized from the bitmap dimensions.
    unsafe {
        let mut bitmap_data = BITMAP::default();
        let copied = GetObjectW(
            HGDIOBJ(bitmap.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bitmap_data as *mut _ as *mut core::ffi::c_void),
        );
        if copied == 0 {
            return None;
        }
        let width = bitmap_data.bmWidth.max(0) as u32;
        let height = bitmap_data.bmHeight.max(0) as u32;
        if width == 0 || height == 0 {
            return None;
        }
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut buffer = vec![0_u8; (width as usize) * (height as usize) * 4];
        let dc = GetDC(None);
        let lines = GetDIBits(
            dc,
            bitmap,
            0,
            height,
            Some(buffer.as_mut_ptr() as *mut core::ffi::c_void),
            &mut info,
            DIB_RGB_COLORS,
        );
        let _ = ReleaseDC(None, dc);
        if lines == 0 {
            return None;
        }
        for pixel in buffer.chunks_exact_mut(4) {
            let blue = pixel[0];
            let green = pixel[1];
            let red = pixel[2];
            let alpha = pixel[3];
            let (straight_red, straight_green, straight_blue) = if alpha == 0 {
                (0, 0, 0)
            } else if alpha == 255 {
                (red, green, blue)
            } else {
                (
                    ((red as u32 * 255) / alpha as u32).min(255) as u8,
                    ((green as u32 * 255) / alpha as u32).min(255) as u8,
                    ((blue as u32 * 255) / alpha as u32).min(255) as u8,
                )
            };
            pixel[0] = straight_red;
            pixel[1] = straight_green;
            pixel[2] = straight_blue;
            pixel[3] = alpha;
        }
        Some(IconPixels {
            width,
            height,
            rgba: buffer,
        })
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("quicknest-icon-cache-{stamp}"));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn cache_key_normalizes_case_and_separators() {
        let lower = cache_key("c:/windows/system32/cmd.exe", ICON_SIZE);
        let upper = cache_key("C:\\Windows\\System32\\cmd.exe", ICON_SIZE);
        assert_eq!(lower, upper);
        assert_ne!(lower, cache_key("C:\\Windows\\System32\\cmd.exe", 32));
    }

    #[test]
    fn cache_key_tracks_file_identity() {
        let dir = temp_dir();
        let file = dir.join("icon.ico");
        std::fs::write(&file, b"a").expect("write");
        let first = cache_key(&file.to_string_lossy(), ICON_SIZE);
        std::fs::write(&file, b"bb").expect("write");
        let second = cache_key(&file.to_string_lossy(), ICON_SIZE);
        assert_ne!(first, second, "size change invalidates the key");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_round_trip_and_corruption() {
        let dir = temp_dir();
        let pixels = IconPixels {
            width: 2,
            height: 2,
            rgba: (1..=16).collect(),
        };
        write_cache(&dir, "abc", &pixels).expect("write");
        assert_eq!(read_cache(&dir, "abc"), Some(pixels));
        std::fs::write(cache_file(&dir, "bad"), b"nope").expect("write bad");
        assert_eq!(read_cache(&dir, "bad"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prune_removes_unreferenced_entries() {
        let dir = temp_dir();
        let pixels = IconPixels {
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 255],
        };
        write_cache(&dir, "keep", &pixels).expect("keep");
        write_cache(&dir, "drop", &pixels).expect("drop");
        let mut keep = std::collections::HashSet::new();
        keep.insert("keep".to_string());
        assert_eq!(prune_cache(&dir, &keep), 1);
        assert!(read_cache(&dir, "keep").is_some());
        assert!(!cache_file(&dir, "drop").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
