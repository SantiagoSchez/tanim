//! The animations and the cell canvas they draw on. The `tanim` binary plays
//! them in a terminal; the `web/` crate compiles them to WebAssembly so the
//! same code runs in a browser page (see `tanim --export-html`).

pub mod anims;
pub mod canvas;
pub mod rng;
pub mod zoom;

/// Keys an animation or the player reacts to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    None,
    Enter,
    Esc,
    /// Zoom in and out.
    Up,
    Down,
    /// Pan the zoomed view (W, A, S, D).
    PanUp,
    PanLeft,
    PanDown,
    PanRight,
    /// The space bar: an animation-specific action (a new dungeon, say).
    Space,
}
