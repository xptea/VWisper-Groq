fn main() {
    #[cfg(target_os = "macos")]
    {
        use std::env;
        use std::path::PathBuf;
        let mut c = cc::Build::new();
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let c_file = manifest_dir.join("src/platform/macos_fn_monitor.c");
        c.file(c_file);
        c.compile("vwisper_macos_fn_monitor");

        let out_dir = std::env::var("OUT_DIR").unwrap();
        println!("cargo:rustc-link-search=native={}", out_dir);
        println!("cargo:rustc-link-lib=static=vwisper_macos_fn_monitor");
        println!("cargo:rustc-link-lib=framework=CoreGraphics");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rerun-if-changed=src/platform/macos_fn_monitor.c");
    }

    tauri_build::build()
}
