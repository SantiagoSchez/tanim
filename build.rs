//! Compiles the `web/` crate to WebAssembly so `tanim --export-html` can embed
//! it in the pages it writes. Without the wasm32 target installed the build
//! still succeeds; the export just reports that it is unavailable.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::{env, fs};

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("tanim_web.wasm");
    // The web crate depends on this one: nothing to do when building for it.
    if env::var("TARGET").is_ok_and(|t| t.starts_with("wasm32")) {
        return;
    }
    for path in ["src/lib.rs", "src/anims", "src/canvas.rs", "src/rng.rs", "web/src", "web/Cargo.toml"] {
        println!("cargo:rerun-if-changed={path}");
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let target_dir = root.join("target").join("wasm");
    let ok = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["build", "--release", "--target", "wasm32-unknown-unknown", "--manifest-path"])
        .arg(root.join("web").join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&target_dir)
        // Settings meant for the outer (native) build must not leak in.
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_BUILD_TARGET")
        .env_remove("CARGO_TARGET_DIR")
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    let wasm = target_dir.join("wasm32-unknown-unknown/release/tanim_web.wasm");
    if !(ok && fs::copy(&wasm, &out).is_ok()) {
        println!("cargo:warning=HTML export disabled: install the target with `rustup target add wasm32-unknown-unknown`");
        fs::write(&out, []).unwrap();
    }
}
