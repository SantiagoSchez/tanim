//! Self-playing snake: BFS to the apple, but only when the tail stays
//! reachable afterwards; otherwise it stalls safely by chasing its tail.
//! Board, snake and apples are pixel art; tiles grow with the screen.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;
use std::collections::VecDeque;

const BG: Rgb = Rgb(10, 14, 12);
const TILE_A: Rgb = Rgb(16, 22, 18);
const TILE_B: Rgb = Rgb(19, 26, 21);
const BAR: Rgb = Rgb(6, 8, 7);
const HEAD: Rgb = Rgb(170, 255, 150);
const TAIL: Rgb = Rgb(20, 90, 130);
const APPLE: Rgb = Rgb(255, 70, 80);
const NONE: u32 = u32::MAX;
/// Steps in the head-to-tail gradient.
const BANDS: f32 = 12.0;

enum State {
    Play,
    Over { t: u32, won: bool },
}

struct Snake {
    w: usize,
    /// Tile size in pixels.
    ts: i32,
    gw: usize,
    gh: usize,
    ox: i32,
    oy: i32,
    body: VecDeque<usize>,
    occ: Vec<bool>,
    apple: usize,
    score: u32,
    best: u32,
    starve: usize,
    state: State,
    frame: u32,
    prev: Vec<u32>,
    queue: VecDeque<usize>,
}

impl Snake {
    fn new(w: usize, h: usize, rng: &mut Rng) -> Snake {
        // The top row holds the header; the board fills the pixels below it.
        let ph = 2 * h.saturating_sub(1);
        let ts = (ph / 14).clamp(2, 6);
        let gw = w / ts;
        let gh = ph / ts;
        let mut s = Snake {
            w,
            ts: ts as i32,
            gw,
            gh,
            ox: ((w - gw * ts) / 2) as i32,
            oy: (2 + (ph - gh * ts) / 2) as i32,
            body: VecDeque::new(),
            occ: vec![false; gw * gh],
            apple: 0,
            score: 0,
            best: 0,
            starve: 0,
            state: State::Play,
            frame: 0,
            prev: vec![NONE; gw * gh],
            queue: VecDeque::new(),
        };
        if s.playable() {
            s.restart(rng);
        }
        s
    }

    fn playable(&self) -> bool {
        self.gw >= 4 && self.gh >= 2
    }

    fn restart(&mut self, rng: &mut Rng) {
        self.occ.fill(false);
        self.body.clear();
        let y = self.gh / 2;
        let x0 = self.gw / 2;
        for i in 0..3 {
            let c = y * self.gw + x0 - i;
            self.body.push_back(c);
            self.occ[c] = true;
        }
        self.score = 0;
        self.starve = 0;
        self.state = State::Play;
        self.place_apple(rng);
    }

    /// Returns false if the board is full.
    fn place_apple(&mut self, rng: &mut Rng) -> bool {
        let free = self.occ.len() - self.body.len();
        if free == 0 {
            return false;
        }
        let mut k = rng.below(free);
        for (i, &o) in self.occ.iter().enumerate() {
            if !o {
                if k == 0 {
                    self.apple = i;
                    return true;
                }
                k -= 1;
            }
        }
        false
    }

    fn neighbors(&self, c: usize) -> ([usize; 4], usize) {
        let (x, y) = (c % self.gw, c / self.gw);
        let mut out = [0; 4];
        let mut n = 0;
        if x > 0 {
            out[n] = c - 1;
            n += 1;
        }
        if x + 1 < self.gw {
            out[n] = c + 1;
            n += 1;
        }
        if y > 0 {
            out[n] = c - self.gw;
            n += 1;
        }
        if y + 1 < self.gh {
            out[n] = c + self.gw;
            n += 1;
        }
        (out, n)
    }

    /// BFS from `from` to `to` over free cells of `occ` (`to` may be occupied).
    /// Returns the path excluding `from`, including `to`.
    fn path(&mut self, occ: &[bool], from: usize, to: usize) -> Option<Vec<usize>> {
        self.prev.fill(NONE);
        self.queue.clear();
        self.queue.push_back(from);
        self.prev[from] = from as u32;
        while let Some(c) = self.queue.pop_front() {
            if c == to {
                let mut p = Vec::new();
                let mut k = to;
                while k != from {
                    p.push(k);
                    k = self.prev[k] as usize;
                }
                p.reverse();
                return Some(p);
            }
            let (nb, n) = self.neighbors(c);
            for &d in &nb[..n] {
                if self.prev[d] == NONE && (!occ[d] || d == to) {
                    self.prev[d] = c as u32;
                    self.queue.push_back(d);
                }
            }
        }
        None
    }

    /// Number of free cells reachable from `from`.
    fn area(&mut self, occ: &[bool], from: usize) -> usize {
        self.prev.fill(NONE);
        self.queue.clear();
        self.queue.push_back(from);
        self.prev[from] = 0;
        let mut n = 0;
        while let Some(c) = self.queue.pop_front() {
            n += 1;
            let (nb, k) = self.neighbors(c);
            for &d in &nb[..k] {
                if self.prev[d] == NONE && !occ[d] {
                    self.prev[d] = 0;
                    self.queue.push_back(d);
                }
            }
        }
        n
    }

    /// Would the head still reach the tail after following `steps`?
    fn safe_after(&mut self, steps: &[usize]) -> bool {
        let mut body = self.body.clone();
        let mut occ = self.occ.clone();
        for &s in steps {
            if s != self.apple {
                if let Some(t) = body.pop_back() {
                    occ[t] = false;
                }
            }
            body.push_front(s);
            occ[s] = true;
        }
        if body.len() >= occ.len() {
            return true;
        }
        let (head, tail) = (body[0], body[body.len() - 1]);
        occ[head] = false;
        let ok = self.path(&occ, head, tail).is_some();
        ok
    }

    fn choose(&mut self) -> Option<usize> {
        let head = self.body[0];
        let tail = self.body[self.body.len() - 1];
        let mut occ = std::mem::take(&mut self.occ);
        occ[tail] = false;
        let to_apple = self.path(&occ, head, self.apple);
        occ[tail] = true;
        self.occ = occ;
        if let Some(p) = to_apple {
            if self.safe_after(&p) {
                return Some(p[0]);
            }
        }
        // Stall: pick the move that keeps the tail reachable and wanders far
        // from the apple; failing that, the one with the most room.
        let (nb, n) = self.neighbors(head);
        let (ax, ay) = ((self.apple % self.gw) as i32, (self.apple / self.gw) as i32);
        let mut best: Option<(i64, usize)> = None;
        for &d in &nb[..n] {
            if self.occ[d] && d != tail {
                continue;
            }
            let dist = ((d % self.gw) as i32 - ax).abs() + ((d / self.gw) as i32 - ay).abs();
            let score = if self.safe_after(&[d]) {
                1_000_000 + dist as i64
            } else {
                let mut occ = self.occ.clone();
                occ[tail] = false;
                occ[d] = false;
                self.area(&occ, d) as i64
            };
            if best.map_or(true, |(b, _)| score > b) {
                best = Some((score, d));
            }
        }
        best.map(|(_, d)| d)
    }

    fn tick(&mut self, rng: &mut Rng) {
        match self.state {
            State::Play => {
                let Some(next) = self.choose() else {
                    self.state = State::Over { t: 0, won: false };
                    return;
                };
                if next == self.apple {
                    self.body.push_front(next);
                    self.occ[next] = true;
                    self.score += 1;
                    self.best = self.best.max(self.score);
                    self.starve = 0;
                    if !self.place_apple(rng) {
                        self.state = State::Over { t: 0, won: true };
                    }
                } else {
                    let t = self.body.pop_back().unwrap_or(next);
                    self.occ[t] = false;
                    self.body.push_front(next);
                    self.occ[next] = true;
                    self.starve += 1;
                    if self.starve > self.occ.len() * 3 {
                        self.state = State::Over { t: 0, won: false };
                    }
                }
            }
            State::Over { ref mut t, .. } => {
                *t += 1;
                if *t > 60 {
                    self.restart(rng);
                }
            }
        }
    }

    /// Top-left pixel of tile `i`.
    fn origin(&self, i: usize) -> (i32, i32) {
        (self.ox + self.ts * (i % self.gw) as i32, self.oy + self.ts * (i / self.gw) as i32)
    }

    /// Side of tile `a` that touches tile `b`: 0 up, 1 right, 2 down, 3 left.
    fn side(&self, a: usize, b: usize) -> usize {
        if b + self.gw == a {
            0
        } else if b == a + 1 {
            1
        } else if b == a + self.gw {
            2
        } else {
            3
        }
    }

    /// One body segment. Sides not joined to a neighbour get a darker rim so
    /// runs lying side by side stay apart; big tiles also round free corners.
    /// The head skips the rim so it looks a little wider than the body.
    fn segment(&self, c: &mut Canvas, i: usize, joined: [bool; 4], col: Rgb, head: bool) {
        let (x0, y0) = self.origin(i);
        let ts = self.ts;
        let rim = if head { col } else { col.scale(0.68) };
        for j in 0..ts {
            for k in 0..ts {
                let edge = [j == 0, k == ts - 1, j == ts - 1, k == 0];
                let free = |s: usize| edge[s] && !joined[s];
                if ts >= 4 && (free(0) || free(2)) && (free(1) || free(3)) {
                    continue;
                }
                let p = if ts > 2 && (0..4).any(free) { rim } else { col };
                c.pixel(x0 + k, y0 + j, p);
            }
        }
    }

    /// Eyes on the head looking along `dir`, and a flickering tongue.
    fn face(&self, c: &mut Canvas, i: usize, dir: usize) {
        let (x0, y0) = self.origin(i);
        let ts = self.ts;
        if ts < 3 {
            return;
        }
        // Eyes of `e` pixels, placed as if the head pointed right: `along`
        // from the back of the tile and `across` from its top.
        let e = if ts >= 6 { 2 } else { 1 };
        let along = (ts - e - 1).max(1);
        let map = |a: i32, b: i32| match dir {
            0 => (b, ts - 1 - a),
            1 => (a, b),
            2 => (ts - 1 - b, a),
            _ => (ts - 1 - a, ts - 1 - b),
        };
        let inset = if e == 1 && ts >= 4 { 1 } else { 0 };
        for b0 in [inset, ts - inset - e] {
            // Pupils sit at the front inner corner of each eye, looking ahead.
            let inner = if b0 == inset { b0 + e - 1 } else { b0 };
            for a in along..along + e {
                for b in b0..b0 + e {
                    // The smallest head has its eyes on the outline, where
                    // dark would read as a notch, so they are just white.
                    let pupil = if e == 1 { ts >= 4 } else { a == along + e - 1 && b == inner };
                    let col = if pupil { Rgb(15, 25, 20) } else { Rgb(235, 250, 230) };
                    let (x, y) = map(a, b);
                    c.pixel(x0 + x, y0 + y, col);
                }
            }
        }
        let (hx, hy) = match dir {
            0 => (x0 + ts / 2, y0 - 1),
            1 => (x0 + ts, y0 + ts / 2),
            2 => (x0 + ts / 2, y0 + ts),
            _ => (x0 - 1, y0 + ts / 2),
        };
        let inside = hx >= self.ox
            && hy >= self.oy
            && hx < self.ox + self.ts * self.gw as i32
            && hy < self.oy + self.ts * self.gh as i32;
        if self.frame % 24 < 4 && inside {
            c.pixel(hx, hy, Rgb(240, 60, 90));
        }
    }

    fn apple(&self, c: &mut Canvas, i: usize) {
        const SMALL: [&str; 2] = ["hR", "RD"];
        const MID: [&str; 3] = [" sl", "hRR", "RRD"];
        const BIG: [&str; 4] = ["  sl", "RhRR", "RRRD", " RD "];
        const HUGE: [&str; 5] = ["  sl ", " RsR ", "RhRRR", "RRRRD", " RDD "];
        const GIANT: [&str; 6] = ["   sl ", " RRsR ", "RhRRRR", "RhRRRD", "RRRRDD", " RRDD "];
        let rows: &[&str] = match self.ts {
            2 => &SMALL,
            3 => &MID,
            4 => &BIG,
            5 => &HUGE,
            _ => &GIANT,
        };
        let (x0, y0) = self.origin(i);
        for (j, row) in rows.iter().enumerate() {
            for (k, ch) in row.chars().enumerate() {
                let col = match ch {
                    'R' => APPLE,
                    'h' => Rgb(255, 170, 160),
                    'D' => Rgb(170, 30, 45),
                    's' => Rgb(110, 70, 40),
                    'l' => Rgb(90, 200, 90),
                    _ => continue,
                };
                c.pixel(x0 + k as i32, y0 + j as i32, col);
            }
        }
    }

    fn draw(&self, c: &mut Canvas) {
        c.clear(BG);
        for i in 0..self.gw * self.gh {
            let (x0, y0) = self.origin(i);
            let tile = tile_of(self, i);
            for y in y0..y0 + self.ts {
                for x in x0..x0 + self.ts {
                    c.pixel(x, y, tile);
                }
            }
        }
        let (over_t, won) = match self.state {
            State::Over { t, won } => (Some(t), won),
            State::Play => (None, false),
        };
        if over_t.is_none() {
            self.apple(c, self.apple);
        }
        let len = self.body.len().max(2);
        for (k, &b) in self.body.iter().enumerate().rev() {
            let t = k as f32 / (len - 1) as f32;
            // The gradient comes in bands, so a move only repaints the few
            // segments that slide into the next band instead of all of them.
            let mut col = HEAD.lerp(TAIL, (t * BANDS).round() / BANDS);
            if let Some(ot) = over_t {
                col = if won {
                    Rgb::hsv(t + ot as f32 * 0.03, 0.7, 1.0)
                } else if (ot / 6) % 2 == 0 {
                    Rgb(90, 90, 90).lerp(BG, ot as f32 / 60.0)
                } else {
                    Rgb(200, 60, 60).lerp(BG, ot as f32 / 60.0)
                };
            }
            if k == 0 && over_t.is_none() {
                col = col.lerp(Rgb::WHITE, 0.35);
            }
            let mut joined = [false; 4];
            for n in [k.wrapping_sub(1), k + 1] {
                if let Some(&o) = self.body.get(n) {
                    joined[self.side(b, o)] = true;
                }
            }
            self.segment(c, b, joined, col, k == 0 && over_t.is_none());
        }
        if over_t.is_none() && self.body.len() >= 2 {
            let (head, neck) = (self.body[0], self.body[1]);
            self.face(c, head, self.side(neck, head));
        }
        c.fill_rect(0, 0, self.w as i32, 1, ' ', BAR, BAR);
        let msg = match (over_t, won) {
            (Some(_), true) => "  BOARD CLEARED!".to_string(),
            (Some(_), false) => "  GAME OVER".to_string(),
            _ => format!("  SNAKE   length {}   score {}   best {}", self.body.len(), self.score, self.best),
        };
        c.text(0, 0, &msg, Rgb(150, 200, 160));
    }
}

fn tile_of(s: &Snake, i: usize) -> Rgb {
    if (i % s.gw + i / s.gw) % 2 == 0 {
        TILE_A
    } else {
        TILE_B
    }
}

impl Animation for Snake {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        if !self.playable() {
            c.clear(BG);
            return;
        }
        // Big boards get two moves per frame so games don't drag on.
        self.frame = self.frame.wrapping_add(1);
        let moves = if self.gw * self.gh > 2500 { 2 } else { 1 };
        for _ in 0..moves {
            self.tick(rng);
        }
        self.draw(c);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    Box::new(Snake::new(w, h, rng))
}
