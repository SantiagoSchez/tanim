//! Cell buffer that animations draw into, plus a diffing renderer that turns
//! it into the minimal stream of truecolor escape sequences.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const BLACK: Rgb = Rgb(0, 0, 0);
    pub const WHITE: Rgb = Rgb(255, 255, 255);

    /// Linear interpolation towards `o`; `t` is clamped to `[0, 1]`.
    #[inline]
    pub fn lerp(self, o: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let f = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t + 0.5) as u8;
        Rgb(f(self.0, o.0), f(self.1, o.1), f(self.2, o.2))
    }

    /// Multiply every channel by `k` (saturating).
    #[inline]
    pub fn scale(self, k: f32) -> Rgb {
        let f = |a: u8| (a as f32 * k).clamp(0.0, 255.0) as u8;
        Rgb(f(self.0), f(self.1), f(self.2))
    }

    /// Channel-wise saturating add.
    #[inline]
    pub fn add(self, o: Rgb) -> Rgb {
        Rgb(
            self.0.saturating_add(o.0),
            self.1.saturating_add(o.1),
            self.2.saturating_add(o.2),
        )
    }

    /// HSV to RGB. `h` wraps around `[0, 1)`, `s` and `v` in `[0, 1]`.
    pub fn hsv(h: f32, s: f32, v: f32) -> Rgb {
        let h = (h - h.floor()) * 6.0;
        let s = s.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);
        let i = h as i32;
        let f = h - i as f32;
        let p = v * (1.0 - s);
        let q = v * (1.0 - s * f);
        let t = v * (1.0 - s * (1.0 - f));
        let (r, g, b) = match i {
            0 => (v, t, p),
            1 => (q, v, p),
            2 => (p, v, t),
            3 => (p, q, v),
            4 => (t, p, v),
            _ => (v, p, q),
        };
        Rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
    }

    /// Sample a multi-stop gradient at `t` in `[0, 1]`.
    pub fn gradient(stops: &[Rgb], t: f32) -> Rgb {
        match stops.len() {
            0 => Rgb::BLACK,
            1 => stops[0],
            n => {
                let t = t.clamp(0.0, 1.0) * (n - 1) as f32;
                let i = (t as usize).min(n - 2);
                stops[i].lerp(stops[i + 1], t - i as f32)
            }
        }
    }

    /// Perceived brightness in `[0, 255]`.
    #[inline]
    pub fn luma(self) -> u8 {
        ((self.0 as u32 * 54 + self.1 as u32 * 183 + self.2 as u32 * 19) >> 8) as u8
    }
}

/// One terminal cell. Every char drawn must be single-width.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Rgb,
}

impl Default for Cell {
    fn default() -> Self {
        Cell { ch: ' ', fg: Rgb::WHITE, bg: Rgb::BLACK }
    }
}

/// Upper half block: fg paints the top "pixel", bg the bottom one.
pub const HALF: char = '▀';

/// Drawing surface. Persists between frames; animations clear it if they want.
/// All drawing methods are bounds-checked and silently clip.
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub cells: Vec<Cell>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Self {
        Canvas { w, h, cells: vec![Cell::default(); w * h] }
    }

    #[inline]
    pub fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            None
        } else {
            Some(y as usize * self.w + x as usize)
        }
    }

    /// Fill every cell with a space on `bg`.
    pub fn clear(&mut self, bg: Rgb) {
        self.cells.fill(Cell { ch: ' ', fg: bg, bg });
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Option<&Cell> {
        self.idx(x, y).map(|i| &self.cells[i])
    }


    /// Set char, foreground and background.
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, ch: char, fg: Rgb, bg: Rgb) {
        if let Some(i) = self.idx(x, y) {
            self.cells[i] = Cell { ch, fg, bg };
        }
    }

    /// Set char and foreground, keeping the existing background.
    #[inline]
    pub fn put(&mut self, x: i32, y: i32, ch: char, fg: Rgb) {
        if let Some(i) = self.idx(x, y) {
            let c = &mut self.cells[i];
            c.ch = ch;
            c.fg = fg;
        }
    }

    /// Change only the background of a cell.
    #[inline]
    pub fn paint_bg(&mut self, x: i32, y: i32, bg: Rgb) {
        if let Some(i) = self.idx(x, y) {
            self.cells[i].bg = bg;
        }
    }

    /// Fill a rectangle with `ch` on `bg`.
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, ch: char, fg: Rgb, bg: Rgb) {
        for yy in y.max(0)..(y + h).min(self.h as i32) {
            for xx in x.max(0)..(x + w).min(self.w as i32) {
                self.set(xx, yy, ch, fg, bg);
            }
        }
    }

    /// Draw a string left to right (spaces included), keeping backgrounds.
    pub fn text(&mut self, x: i32, y: i32, s: &str, fg: Rgb) {
        for (i, ch) in s.chars().enumerate() {
            self.put(x + i as i32, y, ch, fg);
        }
    }

    /// Paint one half-block pixel. `py` ranges over `0..2 * h`.
    #[inline]
    pub fn pixel(&mut self, x: i32, py: i32, c: Rgb) {
        if py < 0 {
            return;
        }
        if let Some(i) = self.idx(x, py >> 1) {
            let cell = &mut self.cells[i];
            if cell.ch != HALF {
                cell.ch = HALF;
                cell.fg = cell.bg;
            }
            if py & 1 == 0 {
                cell.fg = c;
            } else {
                cell.bg = c;
            }
        }
    }

    /// Overwrite the whole canvas from a `w x 2h` pixel buffer.
    pub fn blit_pixels(&mut self, px: &[Rgb]) {
        let w = self.w;
        for y in 0..self.h {
            let top = &px[2 * y * w..2 * y * w + w];
            let bot = &px[(2 * y + 1) * w..(2 * y + 1) * w + w];
            let row = &mut self.cells[y * w..y * w + w];
            for x in 0..w {
                row[x] = Cell { ch: HALF, fg: top[x], bg: bot[x] };
            }
        }
    }
}

/// Keeps what the terminal currently shows and emits only the differences.
pub struct Screen {
    prev: Vec<Cell>,
    w: usize,
    h: usize,
    /// Background color drawn as the terminal's own background instead.
    clear: Option<Rgb>,
    out: Vec<u8>,
}

impl Screen {
    pub fn new() -> Self {
        Screen { prev: Vec::new(), w: 0, h: 0, clear: None, out: Vec::with_capacity(1 << 16) }
    }

    /// Forget what is on screen so the next render repaints everything.
    pub fn invalidate(&mut self) {
        self.w = 0;
        self.h = 0;
    }

    /// Show `bg` as the terminal's default background (and so through any
    /// transparency or blur it has) rather than painting it. Repaints fully.
    pub fn set_clear(&mut self, bg: Option<Rgb>) {
        self.clear = bg;
        self.invalidate();
    }

    /// Build the byte stream that brings the terminal up to date with `c`.
    pub fn render(&mut self, c: &Canvas) -> &[u8] {
        self.out.clear();
        self.out.extend_from_slice(b"\x1b[?2026h");
        if self.w != c.w || self.h != c.h {
            self.w = c.w;
            self.h = c.h;
            // A sentinel that never matches forces a full repaint.
            self.prev = vec![Cell { ch: '\0', fg: Rgb::BLACK, bg: Rgb::BLACK }; c.w * c.h];
            self.out.extend_from_slice(b"\x1b[0m\x1b[2J");
        }
        let mut cur: Option<(usize, usize)> = None;
        let mut fg: Option<Rgb> = None;
        let mut bg: Option<Rgb> = None;
        let mut utf = [0u8; 4];
        for y in 0..c.h {
            for x in 0..c.w {
                let i = y * c.w + x;
                let cell = normalize(c.cells[i], self.clear);
                if looks_same(cell, self.prev[i], self.clear) {
                    continue;
                }
                self.prev[i] = cell;
                match cur {
                    Some((cx, cy)) if cy == y && cx == x => {}
                    Some((cx, cy)) if cy == y && cx < x => {
                        self.out.extend_from_slice(b"\x1b[");
                        push_num(&mut self.out, x - cx);
                        self.out.push(b'C');
                    }
                    _ => {
                        self.out.extend_from_slice(b"\x1b[");
                        push_num(&mut self.out, y + 1);
                        self.out.push(b';');
                        push_num(&mut self.out, x + 1);
                        self.out.push(b'H');
                    }
                }
                let need_bg = bg != Some(cell.bg);
                let need_fg = cell.ch != ' ' && fg != Some(cell.fg);
                if need_bg || need_fg {
                    self.out.extend_from_slice(b"\x1b[");
                    if need_bg {
                        if self.clear == Some(cell.bg) {
                            self.out.extend_from_slice(b"49");
                        } else {
                            push_rgb(&mut self.out, b'4', cell.bg);
                        }
                        bg = Some(cell.bg);
                    }
                    if need_fg {
                        if need_bg {
                            self.out.push(b';');
                        }
                        push_rgb(&mut self.out, b'3', cell.fg);
                        fg = Some(cell.fg);
                    }
                    self.out.push(b'm');
                }
                self.out.extend_from_slice(cell.ch.encode_utf8(&mut utf).as_bytes());
                cur = if x + 1 < c.w { Some((x + 1, y)) } else { None };
            }
        }
        self.out.extend_from_slice(b"\x1b[?2026l");
        &self.out
    }
}

/// Largest per-channel difference not worth repainting a cell for. Compared
/// against what the terminal already shows, so slow fades still arrive: they
/// just update in steps of this size instead of every frame.
const DEADBAND: u8 = 2;

#[inline]
fn close(a: Rgb, b: Rgb, clear: Option<Rgb>) -> bool {
    // Never blur the line between painted and see-through background.
    if clear.is_some() && ((Some(a) == clear) != (Some(b) == clear)) {
        return false;
    }
    a.0.abs_diff(b.0) <= DEADBAND && a.1.abs_diff(b.1) <= DEADBAND && a.2.abs_diff(b.2) <= DEADBAND
}

#[inline]
fn looks_same(a: Cell, b: Cell, clear: Option<Rgb>) -> bool {
    a.ch == b.ch && close(a.bg, b.bg, clear) && (a.ch == ' ' || close(a.fg, b.fg, clear))
}

/// Collapse cells that look identical on screen (a space, a solid block and a
/// half block with equal halves are all one flat color) so they diff equal and
/// need no foreground escape.
#[inline]
fn normalize(c: Cell, clear: Option<Rgb>) -> Cell {
    match c.ch {
        ' ' => Cell { ch: ' ', fg: Rgb::BLACK, bg: c.bg },
        HALF if c.fg == c.bg => Cell { ch: ' ', fg: Rgb::BLACK, bg: c.bg },
        // Only the lower pixel is lit: flip to the lower half block so the
        // transparent half is the background, which is the only part of a
        // cell the terminal can leave unpainted.
        HALF if clear == Some(c.fg) => Cell { ch: '▄', fg: c.bg, bg: c.fg },
        '█' => Cell { ch: ' ', fg: Rgb::BLACK, bg: c.fg },
        _ => c,
    }
}

/// `38;2;r;g;b` (kind `3`) or `48;2;r;g;b` (kind `4`), without CSI or `m`.
#[inline]
fn push_rgb(out: &mut Vec<u8>, kind: u8, c: Rgb) {
    out.extend_from_slice(&[kind, b'8', b';', b'2', b';']);
    push_num(out, c.0 as usize);
    out.push(b';');
    push_num(out, c.1 as usize);
    out.push(b';');
    push_num(out, c.2 as usize);
}

#[inline]
fn push_num(out: &mut Vec<u8>, mut n: usize) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    out.extend_from_slice(&buf[i..]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(ch: char, fg: Rgb, bg: Rgb) -> Canvas {
        let mut c = Canvas::new(1, 1);
        c.set(0, 0, ch, fg, bg);
        c
    }

    fn text(s: &mut Screen, c: &Canvas) -> String {
        String::from_utf8_lossy(s.render(c)).into_owned()
    }

    const CLEAR: Rgb = Rgb(1, 1, 8);
    const RED: Rgb = Rgb(200, 30, 30);

    #[test]
    fn clear_background_uses_the_terminal_default() {
        let mut s = Screen::new();
        s.set_clear(Some(CLEAR));
        // Lit top pixel over a clear bottom: upper block on the default bg.
        let out = text(&mut s, &one(HALF, RED, CLEAR));
        assert!(out.contains("49") && out.contains('▀') && !out.contains("48;2;1;1;8"), "{out:?}");
        // Clear top over a lit bottom: flipped to the lower block.
        let mut s = Screen::new();
        s.set_clear(Some(CLEAR));
        let out = text(&mut s, &one(HALF, CLEAR, RED));
        assert!(out.contains("49") && out.contains('▄') && out.contains("38;2;200;30;30"), "{out:?}");
    }

    #[test]
    fn opaque_screens_paint_every_background() {
        let mut s = Screen::new();
        let out = text(&mut s, &one(HALF, CLEAR, RED));
        assert!(out.contains("48;2;200;30;30") && out.contains('▀') && !out.contains("49"), "{out:?}");
    }

    #[test]
    fn invisible_changes_are_not_sent() {
        let mut s = Screen::new();
        text(&mut s, &one(HALF, RED, Rgb(10, 10, 10)));
        let out = text(&mut s, &one(HALF, Rgb(201, 31, 29), Rgb(12, 10, 9)));
        assert!(!out.contains('▀'), "tiny change was repainted: {out:?}");
        // Drift accumulates against what is on screen, so it arrives eventually.
        let out = text(&mut s, &one(HALF, Rgb(203, 30, 30), Rgb(10, 10, 10)));
        assert!(out.contains('▀'), "accumulated change was never sent: {out:?}");
    }

    #[test]
    fn near_clear_colors_still_replace_the_clear_background() {
        let mut s = Screen::new();
        s.set_clear(Some(CLEAR));
        text(&mut s, &one(' ', CLEAR, CLEAR));
        let out = text(&mut s, &one(' ', Rgb(2, 2, 9), Rgb(2, 2, 9)));
        assert!(out.contains("48;2;2;2;9"), "painted cell stayed see-through: {out:?}");
    }
}
