//! HTTPS transport and BCrypt verification. No certificate bypass or redirects.
#![cfg(windows)]
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Networking::WinInet::*;
use windows::Win32::Security::Cryptography::*;
use windows::core::PCWSTR;
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

struct Hash {
    alg: BCRYPT_ALG_HANDLE,
    hash: BCRYPT_HASH_HANDLE,
}
impl Hash {
    fn new() -> Result<Self, String> {
        unsafe {
            let mut alg = BCRYPT_ALG_HANDLE::default();
            BCryptOpenAlgorithmProvider(
                &mut alg,
                PCWSTR(wide("SHA256").as_ptr()),
                PCWSTR::null(),
                BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS(0),
            )
            .ok()
            .map_err(|e| e.to_string())?;
            let mut hash = BCRYPT_HASH_HANDLE::default();
            if let Err(e) = BCryptCreateHash(alg, &mut hash, None, None, 0).ok() {
                let _ = BCryptCloseAlgorithmProvider(alg, 0);
                return Err(e.to_string());
            }
            Ok(Self { alg, hash })
        }
    }
    fn add(&self, bytes: &[u8]) -> Result<(), String> {
        unsafe {
            BCryptHashData(self.hash, bytes, 0)
                .ok()
                .map_err(|e| e.to_string())
        }
    }
    fn finish(&self) -> Result<String, String> {
        let mut bytes = [0u8; 32];
        unsafe {
            BCryptFinishHash(self.hash, &mut bytes, 0)
                .ok()
                .map_err(|e| e.to_string())?;
        }
        Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
    }
}
impl Drop for Hash {
    fn drop(&mut self) {
        unsafe {
            let _ = BCryptDestroyHash(self.hash);
            let _ = BCryptCloseAlgorithmProvider(self.alg, 0);
        }
    }
}
pub fn sha256_hex(bytes: &[u8]) -> Result<String, String> {
    let hash = Hash::new()?;
    hash.add(bytes)?;
    hash.finish()
}
pub fn sha256_file(path: &Path, max: u64) -> Result<String, String> {
    reject_reparse_path(path)?;
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let hash = Hash::new()?;
    let mut total = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > max {
            return Err("文件超过大小上限".into());
        }
        hash.add(&buffer[..n])?;
    }
    hash.finish()
}
fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, String> {
    if !hex.len().is_multiple_of(2) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("hex 格式无效".into());
    }
    hex.as_bytes()
        .chunks_exact(2)
        .map(|p| {
            let h = (p[0] as char).to_digit(16).unwrap();
            let l = (p[1] as char).to_digit(16).unwrap();
            Ok(((h << 4) | l) as u8)
        })
        .collect()
}
pub fn ecdsa_p256_verify(public_blob_hex: &str, data: &[u8], signature_hex: &str) -> bool {
    let Ok(public) = hex_to_bytes(public_blob_hex) else {
        return false;
    };
    let Ok(signature) = hex_to_bytes(signature_hex) else {
        return false;
    };
    if public.len() != 72
        || public[..8] != [0x45, 0x43, 0x53, 0x31, 0x20, 0, 0, 0]
        || signature.len() != 64
    {
        return false;
    }
    let Ok(digest) = sha256_hex(data).and_then(|h| hex_to_bytes(&h)) else {
        return false;
    };
    unsafe {
        let mut alg = BCRYPT_ALG_HANDLE::default();
        if BCryptOpenAlgorithmProvider(
            &mut alg,
            PCWSTR(wide("ECDSA_P256").as_ptr()),
            PCWSTR::null(),
            BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS(0),
        )
        .is_err()
        {
            return false;
        }
        let mut key = BCRYPT_KEY_HANDLE::default();
        if BCryptImportKeyPair(alg, None, BCRYPT_PUBLIC_KEY_BLOB, &mut key, &public, 0).is_err() {
            let _ = BCryptCloseAlgorithmProvider(alg, 0);
            return false;
        }
        let valid = BCryptVerifySignature(key, None, &digest, &signature, BCRYPT_FLAGS(0)).is_ok();
        let _ = BCryptDestroyKey(key);
        let _ = BCryptCloseAlgorithmProvider(alg, 0);
        valid
    }
}

/// Conservative canonical HTTPS grammar: ASCII DNS/IPv4 host, optional port;
/// no credentials, fragment, backslash, whitespace, opaque URLs or IPv6 ambiguity.
pub fn validate_https_url(url: &str) -> Result<(), String> {
    if url.len() > 8192
        || !url.is_ascii()
        || url
            .bytes()
            .any(|b| b <= 0x20 || b == 0x7f || b == b'\\' || b == b'#')
    {
        return Err("更新 URL 含非法字符".into());
    }
    let rest = url
        .strip_prefix("https://")
        .ok_or("更新源必须是规范 HTTPS URL")?;
    let authority = rest.split(['/', '?']).next().unwrap_or("");
    let mut parts = authority.split(':');
    let host = parts.next().unwrap_or("");
    if host.is_empty()
        || host.len() > 253
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
    {
        return Err("更新 URL 主机名无效".into());
    }
    if let Some(port) = parts.next()
        && (port.is_empty()
            || !port.bytes().all(|b| b.is_ascii_digit())
            || !matches!(port.parse::<u16>(), Ok(1..=65535)))
    {
        return Err("更新 URL 端口无效".into());
    }
    if parts.next().is_some() {
        return Err("更新 URL authority 无效".into());
    }
    Ok(())
}

pub(crate) fn random_token() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    unsafe {
        BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG)
            .ok()
            .map_err(|e| e.to_string())?;
    }
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
/// Reject reparse points in every existing ancestor, including junctions.
pub(crate) fn reject_reparse_path(path: &Path) -> Result<(), String> {
    use std::os::windows::fs::MetadataExt;
    for ancestor in path.ancestors() {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        match std::fs::symlink_metadata(ancestor) {
            Ok(m) if m.file_attributes() & 0x400 != 0 => {
                return Err(format!("拒绝 reparse 路径：{}", ancestor.display()));
            }
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
pub(crate) fn private_temp_dir() -> Result<PathBuf, String> {
    let base = std::env::temp_dir();
    reject_reparse_path(&base)?;
    for _ in 0..8 {
        let dir = base.join(format!("quicknest-update-{}", random_token()?));
        match std::fs::create_dir(&dir) {
            Ok(()) => {
                reject_reparse_path(&dir)?;
                return Ok(dir);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("无法创建独占临时目录".into())
}
struct InternetHandle(*mut core::ffi::c_void);
impl Drop for InternetHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = InternetCloseHandle(self.0.cast_const());
        }
    }
}
fn guard(cancel: &AtomicBool, started: Instant) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("更新已取消".into());
    }
    if started.elapsed() > Duration::from_secs(180) {
        return Err("更新下载超过总时限".into());
    }
    Ok(())
}
fn http_stream(
    url: &str,
    max: u64,
    cancel: &AtomicBool,
    output: &mut impl Write,
) -> Result<u64, String> {
    validate_https_url(url)?;
    let started = Instant::now();
    guard(cancel, started)?;
    unsafe {
        let open = InternetHandle(InternetOpenW(
            PCWSTR(wide("QuickNest-Updater").as_ptr()),
            INTERNET_OPEN_TYPE_PRECONFIG.0,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ));
        if open.0.is_null() {
            return Err("InternetOpen 失败".into());
        }
        let timeout = 5000u32;
        for option in [
            INTERNET_OPTION_CONNECT_TIMEOUT,
            INTERNET_OPTION_RECEIVE_TIMEOUT,
            INTERNET_OPTION_SEND_TIMEOUT,
        ] {
            InternetSetOptionW(
                Some(open.0.cast_const()),
                option,
                Some((&timeout as *const u32).cast()),
                4,
            )
            .map_err(|e| format!("设置网络超时失败：{e}"))?;
        }
        let retries = 0u32;
        InternetSetOptionW(
            Some(open.0.cast_const()),
            INTERNET_OPTION_CONNECT_RETRIES,
            Some((&retries as *const u32).cast()),
            4,
        )
        .map_err(|e| e.to_string())?;
        // Refuse ALL redirects, including HTTPS -> HTTP. A new signed URL is required.
        let flags = INTERNET_FLAG_NO_AUTO_REDIRECT
            | INTERNET_FLAG_NO_CACHE_WRITE
            | INTERNET_FLAG_RELOAD
            | INTERNET_FLAG_NO_UI
            | INTERNET_FLAG_NO_COOKIES;
        let request = InternetHandle(InternetOpenUrlW(
            open.0.cast_const(),
            PCWSTR(wide(url).as_ptr()),
            None,
            flags,
            None,
        ));
        if request.0.is_null() {
            return Err("HTTPS 连接失败（网络或证书不可信）".into());
        }
        guard(cancel, started)?;
        let mut status = 0u32;
        let mut length = 4u32;
        HttpQueryInfoW(
            request.0.cast_const(),
            HTTP_QUERY_STATUS_CODE | HTTP_QUERY_FLAG_NUMBER,
            Some((&mut status as *mut u32).cast()),
            &mut length,
            None,
        )
        .map_err(|e| e.to_string())?;
        if status != 200 {
            return Err(format!(
                "更新响应必须是 HTTP 200，实际 {status}（重定向被拒绝）"
            ));
        }
        let mut declared = [0u16; 32];
        let mut declared_size = 64u32;
        let expected = if HttpQueryInfoW(
            request.0.cast_const(),
            HTTP_QUERY_CONTENT_LENGTH,
            Some(declared.as_mut_ptr().cast()),
            &mut declared_size,
            None,
        )
        .is_ok()
        {
            let text = String::from_utf16_lossy(
                &declared[..declared.iter().position(|&c| c == 0).unwrap_or(32)],
            );
            let n = text.parse::<u64>().map_err(|_| "Content-Length 无效")?;
            if n > max {
                return Err("响应超过大小上限".into());
            }
            Some(n)
        } else {
            None
        };
        let mut total = 0u64;
        let mut buffer = [0u8; 16384];
        loop {
            guard(cancel, started)?;
            let mut n = 0u32;
            InternetReadFile(
                request.0.cast_const(),
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut n,
            )
            .map_err(|e| format!("读取响应失败：{e}"))?;
            guard(cancel, started)?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > max {
                return Err("响应超过大小上限".into());
            }
            output
                .write_all(&buffer[..n as usize])
                .map_err(|e| e.to_string())?;
            // Abort slow trickles after a grace period, in addition to per-I/O timeout.
            if started.elapsed() > Duration::from_secs(15)
                && total / started.elapsed().as_secs().max(1) < 1024
            {
                return Err("更新下载速率过低".into());
            }
        }
        if expected.is_some_and(|n| n != total) {
            return Err("响应被截断：Content-Length 不匹配".into());
        }
        if total == 0 {
            return Err("更新响应为空".into());
        }
        Ok(total)
    }
}
pub fn https_get_text(url: &str) -> Result<String, String> {
    https_get_text_cancel(url, &AtomicBool::new(false))
}
pub fn https_get_text_cancel(url: &str, cancel: &AtomicBool) -> Result<String, String> {
    let mut bytes = Vec::new();
    http_stream(url, 65_536, cancel, &mut bytes)?;
    String::from_utf8(bytes).map_err(|_| "响应不是 UTF-8".into())
}
/// Exclusive creation; never overwrite an existing file (including symlinks).
pub fn https_get_file(url: &str, destination: &Path, max: u64) -> Result<u64, String> {
    https_get_file_cancel(url, destination, max, &AtomicBool::new(false))
}
pub fn https_get_file_cancel(
    url: &str,
    destination: &Path,
    max: u64,
    cancel: &AtomicBool,
) -> Result<u64, String> {
    validate_https_url(url)?;
    reject_reparse_path(destination)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|e| format!("独占创建下载文件失败：{e}"))?;
    let result = http_stream(url, max, cancel, &mut file).and_then(|n| {
        file.sync_all().map_err(|e| e.to_string())?;
        Ok(n)
    });
    drop(file);
    if result.is_err() {
        let _ = std::fs::remove_file(destination);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hostile_urls_are_rejected() {
        for url in [
            "http://host/x",
            "https://",
            "https://user:password@host/x",
            "https://host\\evil/x",
            "https://host/x#y",
            "https://host:0/x",
            "https://host:65536",
            "https://host/%00\n",
            "https://host./x",
            "https://-host/x",
            "HTTPS://host/x",
        ] {
            assert!(validate_https_url(url).is_err(), "{url}");
        }
        assert!(validate_https_url("https://localhost:19443/package?x=1").is_ok());
    }
    #[test]
    fn existing_download_file_is_never_overwritten() {
        let dir = private_temp_dir().unwrap();
        let file = dir.join("package.bin");
        std::fs::write(&file, b"original").unwrap();
        assert!(https_get_file("https://localhost:1/package", &file, 20).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), b"original");
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn cancellation_precedes_network_and_cleans_file() {
        let dir = private_temp_dir().unwrap();
        let file = dir.join("package.bin");
        assert!(
            https_get_file_cancel(
                "https://localhost:1/package",
                &file,
                20,
                &AtomicBool::new(true)
            )
            .unwrap_err()
            .contains("取消")
        );
        assert!(!file.exists());
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn strict_hex_and_stream_hash() {
        for hex in ["é", "0", "aa bb", " aa", "gg"] {
            assert!(hex_to_bytes(hex).is_err());
        }
        assert_eq!(
            sha256_hex(b"abc").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
