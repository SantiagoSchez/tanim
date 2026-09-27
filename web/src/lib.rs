//! WebAssembly exports for the HTML player (`src/web.html`). No bindings
//! generator: the page calls these plain functions and reads each frame
//! straight out of linear memory.
//!
//! A frame is `w * h` cells of three `u32`s: the char, then the foreground and
//! background as `0xRRGGBB`.

use std::cell::RefCell;
use tanim::anims::{Animation, CATALOG};
use tanim::canvas::Canvas;
use tanim::rng::Rng;
use tanim::Key;

struct Player {
    anim: Box<dyn Animation>,
    canvas: Canvas,
    rng: Rng,
    frame: Vec<u32>,
}

thread_local! {
    static PLAYER: RefCell<Option<Player>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn count() -> u32 {
    CATALOG.len() as u32
}

#[no_mangle]
pub extern "C" fn name_ptr(i: u32) -> *const u8 {
    CATALOG.get(i as usize).map_or(std::ptr::null(), |e| e.name.as_ptr())
}

#[no_mangle]
pub extern "C" fn name_len(i: u32) -> u32 {
    CATALOG.get(i as usize).map_or(0, |e| e.name.len() as u32)
}

#[no_mangle]
pub extern "C" fn desc_ptr(i: u32) -> *const u8 {
    CATALOG.get(i as usize).map_or(std::ptr::null(), |e| e.desc.as_ptr())
}

#[no_mangle]
pub extern "C" fn desc_len(i: u32) -> u32 {
    CATALOG.get(i as usize).map_or(0, |e| e.desc.len() as u32)
}

#[no_mangle]
pub extern "C" fn fps(i: u32) -> u32 {
    CATALOG.get(i as usize).map_or(30, |e| e.fps)
}

/// Start animation `i` on a `w` x `h` cell screen.
#[no_mangle]
pub extern "C" fn start(i: u32, w: u32, h: u32, seed: f64) {
    let Some(entry) = CATALOG.get(i as usize) else { return };
    let (w, h) = (w.max(1) as usize, h.max(1) as usize);
    let mut rng = Rng::new(seed as u64);
    let anim = tanim::zoom::wrap((entry.make)(w, h, &mut rng), w, h);
    PLAYER.with_borrow_mut(|p| {
        *p = Some(Player { anim, canvas: Canvas::new(w, h), rng, frame: vec![0; w * h * 3] });
    });
}

/// Advance one tick and return a pointer to the packed frame.
#[no_mangle]
pub extern "C" fn step() -> *const u32 {
    PLAYER.with_borrow_mut(|p| {
        let Some(p) = p else { return std::ptr::null() };
        p.anim.step(&mut p.canvas, &mut p.rng);
        let rgb = |c: tanim::canvas::Rgb| (c.0 as u32) << 16 | (c.1 as u32) << 8 | c.2 as u32;
        for (out, cell) in p.frame.chunks_exact_mut(3).zip(&p.canvas.cells) {
            out[0] = cell.ch as u32;
            out[1] = rgb(cell.fg);
            out[2] = rgb(cell.bg);
        }
        p.frame.as_ptr()
    })
}

/// Forward a key: 1 zoom in, 2 zoom out, 3-6 pan left, right, up, down,
/// 7 space.
#[no_mangle]
pub extern "C" fn key(k: u32) {
    let key = match k {
        1 => Key::Up,
        2 => Key::Down,
        3 => Key::PanLeft,
        4 => Key::PanRight,
        5 => Key::PanUp,
        6 => Key::PanDown,
        7 => Key::Space,
        _ => return,
    };
    PLAYER.with_borrow_mut(|p| {
        if let Some(p) = p {
            p.anim.key(key);
        }
    });
}
