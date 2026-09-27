//! Self-playing Tetris. The AI scores every rotation/column with the classic
//! height/lines/holes/bumpiness heuristic, then steers the piece there. It
//! gets sloppier as the level rises, so games eventually end and restart.
//! Drawn in half-block pixels: bevelled blocks sized to fill the screen height.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;

const BW: usize = 10;
const BH: usize = 20;

const BG: Rgb = Rgb(8, 8, 16);
const WELL: Rgb = Rgb(16, 16, 28);
const WELL_ALT: Rgb = Rgb(20, 20, 34);
const DOT: Rgb = Rgb(34, 34, 54);
const FRAME: Rgb = Rgb(90, 100, 150);
const TEXT: Rgb = Rgb(170, 180, 220);

const COLORS: [Rgb; 7] = [
    Rgb(60, 220, 230),  // I
    Rgb(240, 210, 60),  // O
    Rgb(170, 90, 230),  // T
    Rgb(90, 220, 100),  // S
    Rgb(235, 70, 80),   // Z
    Rgb(70, 110, 240),  // J
    Rgb(245, 150, 50),  // L
];

const SHAPES: [[(i8, i8); 4]; 7] = [
    [(0, 1), (1, 1), (2, 1), (3, 1)],
    [(1, 0), (2, 0), (1, 1), (2, 1)],
    [(1, 0), (0, 1), (1, 1), (2, 1)],
    [(1, 0), (2, 0), (0, 1), (1, 1)],
    [(0, 0), (1, 0), (1, 1), (2, 1)],
    [(0, 0), (0, 1), (1, 1), (2, 1)],
    [(2, 0), (0, 1), (1, 1), (2, 1)],
];

type Cells = [(i8, i8); 4];

/// Rotation `r` of piece `p`, normalized to start at (0, 0).
fn shape(p: usize, r: usize) -> Cells {
    let mut c = SHAPES[p];
    for _ in 0..r % 4 {
        for b in c.iter_mut() {
            *b = (-b.1, b.0);
        }
    }
    let mx = c.iter().map(|b| b.0).min().unwrap_or(0);
    let my = c.iter().map(|b| b.1).min().unwrap_or(0);
    for b in c.iter_mut() {
        b.0 -= mx;
        b.1 -= my;
    }
    c
}

fn rotations(p: usize) -> usize {
    match p {
        1 => 1,
        0 | 3 | 4 => 2,
        _ => 4,
    }
}

type Board = [[u8; BW]; BH];

fn fits(b: &Board, cells: &Cells, x: i32, y: i32) -> bool {
    cells.iter().all(|&(dx, dy)| {
        let (cx, cy) = (x + dx as i32, y + dy as i32);
        (0..BW as i32).contains(&cx) && cy < BH as i32 && (cy < 0 || b[cy as usize][cx as usize] == 0)
    })
}

fn drop_y(b: &Board, cells: &Cells, x: i32, y: i32) -> i32 {
    let mut y = y;
    while fits(b, cells, x, y + 1) {
        y += 1;
    }
    y
}

/// Board quality after placing a piece (higher is better).
fn evaluate(b: &Board) -> f32 {
    let mut lines = 0;
    let mut heights = [0i32; BW];
    let mut holes = 0;
    for (y, row) in b.iter().enumerate() {
        if row.iter().all(|&c| c != 0) {
            lines += 1;
        }
        for x in 0..BW {
            if row[x] != 0 {
                if heights[x] == 0 {
                    heights[x] = (BH - y) as i32;
                }
            } else if heights[x] != 0 {
                holes += 1;
            }
        }
    }
    let agg: i32 = heights.iter().sum();
    let bump: i32 = heights.windows(2).map(|w| (w[0] - w[1]).abs()).sum();
    -0.51 * agg as f32 + 0.76 * lines as f32 - 0.36 * holes as f32 - 0.18 * bump as f32
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Steer,
    Drop,
    Clear(u32),
    Over(u32),
}

struct Tetris {
    w: usize,
    h: usize,
    board: Board,
    piece: usize,
    next: usize,
    bag: Vec<usize>,
    rot: usize,
    x: i32,
    y: i32,
    target: (usize, i32),
    phase: Phase,
    tick: u32,
    full_rows: Vec<usize>,
    score: u32,
    lines: u32,
    best: u32,
}

impl Tetris {
    fn new(w: usize, h: usize, rng: &mut Rng) -> Tetris {
        let mut t = Tetris {
            w,
            h,
            board: [[0; BW]; BH],
            piece: 0,
            next: 0,
            bag: Vec::new(),
            rot: 0,
            x: 0,
            y: 0,
            target: (0, 0),
            phase: Phase::Steer,
            tick: 0,
            full_rows: Vec::new(),
            score: 0,
            lines: 0,
            best: 0,
        };
        t.restart(rng);
        t
    }

    fn level(&self) -> u32 {
        self.lines / 10
    }

    fn draw_bag(&mut self, rng: &mut Rng) -> usize {
        if self.bag.is_empty() {
            self.bag = (0..7).collect();
            for i in (1..7).rev() {
                self.bag.swap(i, rng.below(i + 1));
            }
        }
        self.bag.pop().unwrap_or(0)
    }

    fn restart(&mut self, rng: &mut Rng) {
        self.board = [[0; BW]; BH];
        self.bag.clear();
        self.score = 0;
        self.lines = 0;
        self.next = self.draw_bag(rng);
        self.spawn(rng);
    }

    fn spawn(&mut self, rng: &mut Rng) {
        self.piece = self.next;
        self.next = self.draw_bag(rng);
        self.rot = 0;
        self.x = 3;
        self.y = 0;
        self.tick = 0;
        if !fits(&self.board, &shape(self.piece, 0), self.x, self.y) {
            self.best = self.best.max(self.score);
            self.phase = Phase::Over(0);
            return;
        }
        self.target = self.plan(rng);
        self.phase = Phase::Steer;
    }

    /// Best (rotation, x) for the current piece; sometimes a random one.
    fn plan(&self, rng: &mut Rng) -> (usize, i32) {
        let mut options = Vec::new();
        for r in 0..rotations(self.piece) {
            let cells = shape(self.piece, r);
            let pw = cells.iter().map(|b| b.0).max().unwrap_or(0) as i32 + 1;
            for x in 0..=(BW as i32 - pw) {
                if !fits(&self.board, &cells, x, 0) {
                    continue;
                }
                let y = drop_y(&self.board, &cells, x, 0);
                let mut b = self.board;
                for &(dx, dy) in &cells {
                    let cy = y + dy as i32;
                    if cy >= 0 {
                        b[cy as usize][(x + dx as i32) as usize] = 1;
                    }
                }
                options.push((evaluate(&b), r, x));
            }
        }
        if options.is_empty() {
            return (0, self.x);
        }
        let sloppiness = (self.level() as f32 * 0.03).min(0.35);
        if rng.chance(sloppiness) {
            let o = options[rng.below(options.len())];
            return (o.1, o.2);
        }
        let best = options.iter().fold(options[0], |a, &o| if o.0 > a.0 { o } else { a });
        (best.1, best.2)
    }

    fn lock(&mut self) {
        for &(dx, dy) in &shape(self.piece, self.rot) {
            let (cx, cy) = (self.x + dx as i32, self.y + dy as i32);
            if cy >= 0 {
                self.board[cy as usize][cx as usize] = self.piece as u8 + 1;
            }
        }
        self.full_rows = (0..BH).filter(|&y| self.board[y].iter().all(|&c| c != 0)).collect();
        self.phase = if self.full_rows.is_empty() { Phase::Drop } else { Phase::Clear(0) };
    }

    fn update(&mut self, rng: &mut Rng) {
        self.tick += 1;
        match self.phase {
            Phase::Steer => {
                let every = 4u32.saturating_sub(self.level() / 3).max(1);
                if self.tick % every != 0 {
                    return;
                }
                let cells = shape(self.piece, self.target.0);
                if self.rot != self.target.0 {
                    let r = (self.rot + 1) % 4;
                    if fits(&self.board, &shape(self.piece, r), self.x, self.y) {
                        self.rot = r;
                    } else {
                        self.target.0 = self.rot;
                    }
                } else if self.x != self.target.1 {
                    let nx = self.x + (self.target.1 - self.x).signum();
                    if fits(&self.board, &cells, nx, self.y) {
                        self.x = nx;
                    } else {
                        self.target.1 = self.x;
                    }
                } else {
                    self.phase = Phase::Drop;
                    self.tick = 0;
                }
            }
            Phase::Drop => {
                let cells = shape(self.piece, self.rot);
                if fits(&self.board, &cells, self.x, self.y + 1) {
                    self.y += 1;
                } else {
                    self.lock();
                    if self.phase == Phase::Drop {
                        self.spawn(rng);
                    }
                }
            }
            Phase::Clear(t) => {
                if t >= 14 {
                    let n = self.full_rows.len();
                    let mut kept: Vec<[u8; BW]> =
                        (0..BH).filter(|y| !self.full_rows.contains(y)).map(|y| self.board[y]).collect();
                    let mut b = [[0u8; BW]; BH];
                    let off = BH - kept.len();
                    for (i, row) in kept.drain(..).enumerate() {
                        b[off + i] = row;
                    }
                    self.board = b;
                    self.score += [0, 100, 300, 500, 800][n.min(4)] * (self.level() + 1);
                    self.lines += n as u32;
                    self.full_rows.clear();
                    self.spawn(rng);
                } else {
                    self.phase = Phase::Clear(t + 1);
                }
            }
            Phase::Over(t) => {
                if t > (BH as u32) * 2 + 40 {
                    self.restart(rng);
                } else {
                    self.phase = Phase::Over(t + 1);
                }
            }
        }
    }

    /// Pixel size of a board cell, with the well's frame thickness.
    fn cell_size(&self) -> (i32, i32) {
        let (w, ph) = (self.w as i32, 2 * self.h as i32);
        let mut s = (ph / BH as i32).max(1);
        // Narrow screens: shrink until the well and the side panel fit.
        while s > 1 && 10 * s + 4 + 3 + (4 * s).max(9) > w {
            s -= 1;
        }
        (s, if s >= 4 { 2 } else { 1 })
    }

    fn rect(c: &mut Canvas, x: i32, y: i32, w: i32, h: i32, col: Rgb) {
        for py in y..y + h {
            for px in x..x + w {
                c.pixel(px, py, col);
            }
        }
    }

    /// A bevelled block: light top-left edge, dark bottom-right edge.
    fn block(c: &mut Canvas, x: i32, y: i32, s: i32, col: Rgb) {
        // Tiny blocks get a gentler bevel so they still read as solid.
        let (l, d) = if s == 2 { (0.3, 0.7) } else { (0.45, 0.55) };
        let light = col.lerp(Rgb::WHITE, l);
        let dark = col.scale(d);
        for j in 0..s {
            for i in 0..s {
                let p = if s == 1 {
                    col
                } else if (j == 0 || i == 0) && i < s - 1 && j < s - 1 {
                    light
                } else if j == s - 1 || i == s - 1 {
                    dark
                } else if s >= 4 && i == 1 && j == 1 {
                    col.lerp(Rgb::WHITE, 0.25)
                } else {
                    col
                };
                c.pixel(x + i, y + j, p);
            }
        }
    }

    /// Empty well cell: a faint checker at small sizes, a dot grid otherwise.
    fn empty(c: &mut Canvas, x: i32, y: i32, s: i32, odd: bool) {
        if s < 3 {
            Self::rect(c, x, y, s, s, if odd { WELL_ALT } else { WELL });
        } else {
            Self::rect(c, x, y, s, s, WELL);
            c.pixel(x + s / 2, y + s / 2, DOT);
        }
    }

    fn draw(&self, c: &mut Canvas) {
        c.clear(BG);
        let (s, fb) = self.cell_size();
        let (bw, bh) = (BW as i32 * s, BH as i32 * s);
        let panel = (4 * s).max(9);
        let total_w = bw + 2 * fb + 3 + panel;
        // Top-left corner of the well's interior, in pixels.
        let bx = (self.w as i32 - total_w) / 2 + fb;
        // A well that fills the whole height drops its top and bottom frame.
        let fv = fb.min((2 * self.h as i32 - bh) / 2).max(0);
        let by = (2 * self.h as i32 - (bh + 2 * fv)) / 2 + fv;
        // Frame, bevelled like a raised border.
        Self::rect(c, bx - fb, by - fv, bw + 2 * fb, fv, FRAME.lerp(Rgb::WHITE, 0.25));
        Self::rect(c, bx - fb, by, fb, bh + fv, FRAME.lerp(Rgb::WHITE, 0.25));
        Self::rect(c, bx + bw, by, fb, bh + fv, FRAME.scale(0.6));
        Self::rect(c, bx, by + bh, bw, fv, FRAME.scale(0.6));
        let over = if let Phase::Over(t) = self.phase { Some(t) } else { None };
        let clear_t = if let Phase::Clear(t) = self.phase { Some(t) } else { None };
        let grey = Rgb(70, 70, 85);
        for y in 0..BH {
            let full = clear_t.is_some() && self.full_rows.contains(&y);
            for x in 0..BW {
                let (sx, sy) = (bx + s * x as i32, by + s * y as i32);
                let v = self.board[y][x];
                let odd = (x + y) % 2 == 1;
                // Game over: rows turn grey from the bottom, then wipe from the top.
                if let Some(t) = over {
                    let fill = (t / 2) as usize;
                    if fill > BH && y < fill - BH {
                        Self::empty(c, sx, sy, s, odd);
                        continue;
                    }
                    if fill > BH || y + fill >= BH {
                        Self::block(c, sx, sy, s, grey);
                        continue;
                    }
                }
                if full {
                    // Line clear: flash white, then vanish from the middle out.
                    let t = clear_t.unwrap_or(0);
                    let gone = t >= 6 && (2 * x as i32 - 9).abs() < (t as i32 - 5) * 11 / 8;
                    if gone {
                        Self::empty(c, sx, sy, s, odd);
                    } else if t < 6 && t % 4 < 2 || t >= 6 {
                        Self::block(c, sx, sy, s, Rgb(235, 240, 255));
                    } else {
                        Self::block(c, sx, sy, s, COLORS[v as usize - 1]);
                    }
                } else if v != 0 {
                    Self::block(c, sx, sy, s, COLORS[v as usize - 1]);
                } else {
                    Self::empty(c, sx, sy, s, odd);
                }
            }
        }
        if matches!(self.phase, Phase::Steer | Phase::Drop) {
            let cells = shape(self.piece, self.rot);
            let col = COLORS[self.piece];
            let gy = drop_y(&self.board, &cells, self.x, self.y);
            // Ghost: hollow outlines where the piece will land.
            for &(dx, dy) in &cells {
                let sx = bx + s * (self.x + dx as i32);
                let sy = by + s * (gy + dy as i32);
                if s < 3 {
                    Self::rect(c, sx, sy, s, s, col.scale(0.3));
                } else {
                    Self::rect(c, sx, sy, s, s, WELL);
                    let edge = col.scale(0.45);
                    Self::rect(c, sx, sy, s, 1, edge);
                    Self::rect(c, sx, sy + s - 1, s, 1, edge);
                    Self::rect(c, sx, sy, 1, s, edge);
                    Self::rect(c, sx + s - 1, sy, 1, s, edge);
                }
            }
            for &(dx, dy) in &cells {
                Self::block(c, bx + s * (self.x + dx as i32), by + s * (self.y + dy as i32), s, col);
            }
        }
        // Side panel. Text sits on whole cells of plain background.
        let px = bx + bw + fb + 3;
        let ty = by / 2;
        c.text(px, ty, "NEXT", TEXT);
        for &(dx, dy) in &shape(self.next, 0) {
            Self::block(c, px + s * dx as i32, 2 * (ty + 2) + s * dy as i32, s, COLORS[self.next]);
        }
        let stats = [
            ("SCORE", self.score),
            ("LINES", self.lines),
            ("LEVEL", self.level()),
            ("BEST", self.best.max(self.score)),
        ];
        for (i, (k, v)) in stats.iter().enumerate() {
            let y = ty + s + 4 + 3 * i as i32;
            c.text(px, y, k, TEXT.scale(0.7));
            c.text(px, y + 1, &v.to_string(), Rgb::WHITE);
        }
        if over.is_some() {
            let y = (by + bh / 2) / 2;
            Self::rect(c, bx, 2 * y - 1, bw, 4, BG);
            c.text(bx + bw / 2 - 4, y, "GAME OVER", Rgb(255, 90, 90));
        }
    }
}

impl Animation for Tetris {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        self.update(rng);
        self.draw(c);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    Box::new(Tetris::new(w, h, rng))
}
