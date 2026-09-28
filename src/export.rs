//! `--export-html`: standalone web pages that run the animations in a
//! browser. build.rs compiles them to WebAssembly; each page embeds that
//! module (base64), the favicon and the player in `web.html`, so it opens from
//! disk with no server.

use std::fs;
use std::path::Path;
use tanim::anims::Entry;

const WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tanim_web.wasm"));
const PAGE: &str = include_str!("web.html");
const ICON: &[u8] = include_bytes!("favicon.png");

/// Write `index.html` (every animation, with a picker) and one page per entry
/// in `pages` into `dir`, returning the paths written.
pub fn write(dir: &Path, pages: &[&Entry]) -> Result<Vec<String>, String> {
    if WASM.is_empty() {
        return Err("this build has no web support; run `rustup target add wasm32-unknown-unknown` and rebuild".into());
    }
    fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let wasm = base64(WASM);
    let icon = base64(ICON);
    let page = |title: &str, start: &str, single: bool| {
        PAGE.replace("__TITLE__", title)
            .replace("__START__", start)
            .replace("__SINGLE__", if single { "true" } else { "false" })
            .replace("__ICON__", &icon)
            .replace("__WASM__", &wasm)
    };
    let mut written = Vec::new();
    let mut save = |name: &str, html: String| {
        let path = dir.join(format!("{name}.html"));
        fs::write(&path, html).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        written.push(path.display().to_string());
        Ok::<_, String>(())
    };
    save("index", page("tanim", "", false))?;
    for e in pages {
        save(e.name, page(&format!("tanim · {}", e.name), e.name, true))?;
    }
    Ok(written)
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() { ABC[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_matches_the_standard_alphabet() {
        assert_eq!(super::base64(b""), "");
        assert_eq!(super::base64(b"f"), "Zg==");
        assert_eq!(super::base64(b"fo"), "Zm8=");
        assert_eq!(super::base64(b"foo"), "Zm9v");
        assert_eq!(super::base64(b"tanim!"), "dGFuaW0h");
    }
}
