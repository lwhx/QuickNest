fn main() {
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
