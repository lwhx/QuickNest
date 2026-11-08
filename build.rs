fn main() {
    write_build_info();
    let config = slint_build::CompilerConfiguration::new()
        .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles)
        .with_debug_info(std::env::var("PROFILE").as_deref() != Ok("release"));
    slint_build::compile_with_config("ui/main_window.slint", config)
        .expect("Failed to compile the P0-01 Slint window");

    if std::env::var("TARGET").as_deref() == Ok("x86_64-pc-windows-msvc") {
        let manifest = std::path::Path::new("assets/QuickNest.manifest")
            .canonicalize()
            .expect("Missing Windows manifest");
        println!("cargo:rustc-link-arg-bin=quicknest=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin=quicknest=/MANIFESTINPUT:{}",
            manifest.display()
        );
        println!("cargo:rerun-if-changed=assets/QuickNest.manifest");
    }
}

// Kept dependency-free so recording provenance never changes Cargo.lock.
fn sha256(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut padded = bytes.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&((bytes.len() as u64) * 8).to_be_bytes());
    for block in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (word, chunk) in w.iter_mut().zip(block.chunks_exact(4)) {
            *word = u32::from_be_bytes(chunk.try_into().unwrap());
        }
        for i in 16..64 {
            let a = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let b = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(a)
                .wrapping_add(w[i - 7])
                .wrapping_add(b);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (word, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *word = word.wrapping_add(value);
        }
    }
    state.iter().map(|word| format!("{word:08x}")).collect()
}

fn tool_version(variable: &str) -> String {
    println!("cargo:rerun-if-env-changed={variable}");
    let program = std::env::var_os(variable).expect("Cargo tool path is missing");
    let result = std::process::Command::new(program)
        .arg("-Vv")
        .output()
        .expect("Cannot query build tool version");
    assert!(result.status.success(), "Cannot query build tool version");
    String::from_utf8(result.stdout)
        .expect("Invalid tool version encoding")
        .replace("\r\n", "\n")
        .trim()
        .to_owned()
}

fn write_build_info() {
    use std::fmt::Write;
    let names = [
        "QUICKNEST_BUILD_SOURCE_FINGERPRINT",
        "QUICKNEST_BUILD_BASE_HEAD",
        "QUICKNEST_BUILD_DIRTY",
        "QUICKNEST_BUILD_RUSTC_SHA256",
        "QUICKNEST_BUILD_CARGO_SHA256",
        "QUICKNEST_BUILD_FINGERPRINT",
    ];
    for name in names {
        println!("cargo:rerun-if-env-changed={name}");
    }
    println!("cargo:rerun-if-env-changed=QUICKNEST_UPDATE_PUBLIC_KEY_HEX");
    println!("cargo:rerun-if-changed=src/update_key.pub");
    let key = std::env::var("QUICKNEST_UPDATE_PUBLIC_KEY_HEX").unwrap_or_default();
    let key = key.trim();
    let key_bytes: Vec<u8> = if key.is_empty() {
        Vec::new()
    } else {
        assert!(
            key.len() == 144 && key.bytes().all(|b| b.is_ascii_hexdigit()),
            "Release public key must be a 72-byte BCrypt ECDSA P-256 PUBLIC blob"
        );
        (0..key.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&key[i..i + 2], 16).unwrap())
            .collect()
    };
    let key_status = if key_bytes.is_empty() {
        "disabled"
    } else {
        assert!(
            &key_bytes[..8] == b"ECS1\x20\0\0\0",
            "Only an ECDSA P-256 public blob is accepted"
        );
        assert!(
            !key.eq_ignore_ascii_case(include_str!("src/update_key.pub").trim()),
            "The disclosed fixture key cannot be used for a release"
        );
        "configured"
    };
    let key_hash = if key_bytes.is_empty() {
        None
    } else {
        Some(sha256(&key_bytes))
    };
    let rustc = tool_version("RUSTC");
    let cargo = tool_version("CARGO");
    let target = std::env::var("TARGET").unwrap();
    let profile = std::env::var("PROFILE").unwrap();
    let values: Vec<_> = names
        .iter()
        .map(|name| std::env::var(name).ok().filter(|s| !s.is_empty()))
        .collect();
    let recorded = values.iter().any(Option::is_some);
    let mut generated = String::new();
    for (name, value) in [
        ("BUILD_TARGET", &target),
        ("BUILD_PROFILE", &profile),
        ("BUILD_RUSTC", &rustc),
        ("BUILD_CARGO", &cargo),
    ] {
        writeln!(generated, "const {name}: &str = {value:?};").unwrap();
    }
    writeln!(
        generated,
        "const BUILD_UPDATE_KEY_STATUS: &str = {key_status:?};"
    )
    .unwrap();
    writeln!(
        generated,
        "const BUILD_UPDATE_KEY_SHA256: Option<&str> = {:?};",
        key_hash.as_deref()
    )
    .unwrap();
    if recorded {
        assert!(
            values.iter().all(Option::is_some),
            "Incomplete release provenance; use scripts/build-release.ps1"
        );
        let v: Vec<&str> = values.iter().map(|v| v.as_deref().unwrap()).collect();
        let hex = |s: &str, len| {
            s.len() == len
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        assert!(
            hex(v[0], 64)
                && (hex(v[1], 40) || hex(v[1], 64))
                && hex(v[3], 64)
                && hex(v[4], 64)
                && hex(v[5], 64),
            "Invalid release provenance fingerprints"
        );
        assert!(
            matches!(v[2], "true" | "false"),
            "Invalid release dirty declaration"
        );
        assert_eq!(
            v[3],
            sha256(rustc.as_bytes()),
            "Rust compiler changed after source freeze"
        );
        assert_eq!(
            v[4],
            sha256(cargo.as_bytes()),
            "Cargo changed after source freeze"
        );
        let version = std::env::var("CARGO_PKG_VERSION").unwrap();
        let recipe = format!(
            "quicknest-build-v1\nversion\t{version}\nsource_fingerprint\t{}\nbase_head\t{}\ndirty\t{}\ntarget\t{target}\nprofile\t{profile}\nrustc\t{}\ncargo\t{}\nupdate_key_status\t{key_status}\nupdate_key_blob_sha256\t{}\n",
            v[0],
            v[1],
            v[2],
            v[3],
            v[4],
            key_hash.as_deref().unwrap_or("")
        );
        assert_eq!(
            v[5],
            sha256(recipe.as_bytes()),
            "Release build fingerprint differs from the frozen recipe"
        );
        generated.push_str("const BUILD_PROVENANCE: &str = \"recorded\";\n");
        for (name, value) in [
            ("BUILD_SOURCE_FINGERPRINT", v[0]),
            ("BUILD_BASE_HEAD", v[1]),
            ("BUILD_FINGERPRINT", v[5]),
        ] {
            writeln!(generated, "const {name}: Option<&str> = Some({value:?});").unwrap();
        }
        writeln!(
            generated,
            "const BUILD_DIRTY: Option<bool> = Some({});",
            v[2]
        )
        .unwrap();
    } else {
        generated.push_str("const BUILD_PROVENANCE: &str = \"unrecorded\";\nconst BUILD_SOURCE_FINGERPRINT: Option<&str> = None;\nconst BUILD_BASE_HEAD: Option<&str> = None;\nconst BUILD_FINGERPRINT: Option<&str> = None;\nconst BUILD_DIRTY: Option<bool> = None;\n");
    }
    let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap())
        .join("quicknest_build_info.rs");
    std::fs::write(path, generated).expect("Cannot write compiled build provenance");
}
