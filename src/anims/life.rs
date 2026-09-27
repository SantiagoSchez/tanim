//! Conway's Game of Life on a toroidal half-block grid. Cells change color
//! as they age, dead cells leave fading trails, and the board is refreshed
//! with soups and gliders when it stagnates.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;

const HISTORY: usize = 64;
const POP_WINDOW: usize = 240;
const MAX_EPOCH: u32 = 3000;
const FADE_FRAMES: u32 = 24;
const AGE_STEPS: usize = 120;

struct Life {
    w: usize,
    h: usize,
    cells: Vec<u8>,
    next: Vec<u8>,
    age: Vec<u16>,
    px: Vec<Rgb>,
    hue: f32,
    hashes: [u64; HISTORY],
    hash_pos: usize,
    repeats: u32,
    pops: Vec<u32>,
    pop_pos: usize,
    gen: u32,
    since_inject: u32,
    fade: u32,
}

impl Life {
    fn reseed(&mut self, rng: &mut Rng) {
        let density = rng.rangef(0.2, 0.4);
        for (c, a) in self.cells.iter_mut().zip(self.age.iter_mut()) {
            *c = rng.chance(density) as u8;
            *a = 0;
        }
        self.hue = rng.f32();
        self.reset_detectors();
        self.gen = 0;
    }

    fn reset_detectors(&mut self) {
        self.hashes = [0; HISTORY];
        self.repeats = 0;
        self.pops.fill(u32::MAX);
        self.since_inject = 0;
    }

    fn set(&mut self, x: i32, y: i32) {
        let x = x.rem_euclid(self.w as i32) as usize;
        let y = y.rem_euclid(self.h as i32) as usize;
        let i = y * self.w + x;
        if self.cells[i] == 0 {
            self.cells[i] = 1;
            self.age[i] = 0;
        }
    }

    /// Drop a random soup patch or a glider somewhere on the board.
    fn inject(&mut self, rng: &mut Rng) {
        let cx = rng.below(self.w) as i32;
        let cy = rng.below(self.h) as i32;
        if rng.chance(0.4) {
            const GLIDER: [(i32, i32); 5] = [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)];
            let sx = if rng.chance(0.5) { 1 } else { -1 };
            let sy = if rng.chance(0.5) { 1 } else { -1 };
            for _ in 0..rng.range(1, 4) {
                let ox = cx + rng.range(-12, 12);
                let oy = cy + rng.range(-12, 12);
                for &(dx, dy) in &GLIDER {
                    self.set(ox + dx * sx, oy + dy * sy);
                }
            }
        } else {
            let r = rng.range(4, 10);
            for dy in -r..=r {
                for dx in -r..=r {
                    if rng.chance(0.45) {
                        self.set(cx + dx, cy + dy);
                    }
                }
            }
        }
        self.reset_detectors();
    }

    fn evolve(&mut self) -> u32 {
        let (w, h) = (self.w, self.h);
        let mut pop = 0;
        for y in 0..h {
            let up = (y + h - 1) % h * w;
            let mid = y * w;
            let dn = (y + 1) % h * w;
            for x in 0..w {
                let l = if x == 0 { w - 1 } else { x - 1 };
                let r = if x + 1 == w { 0 } else { x + 1 };
                let c = &self.cells;
                let n = c[up + l] + c[up + x] + c[up + r] + c[mid + l] + c[mid + r]
                    + c[dn + l] + c[dn + x] + c[dn + r];
                let alive = c[mid + x];
                let live = (n == 3 || (n == 2 && alive == 1)) as u8;
                self.next[mid + x] = live;
                pop += live as u32;
            }
        }
        std::mem::swap(&mut self.cells, &mut self.next);
        for (a, &c) in self.age.iter_mut().zip(&self.cells) {
            *a = if c == 1 { a.saturating_add(1) } else { 0 };
        }
        pop
    }

    fn stagnant(&mut self, pop: u32) -> bool {
        // Exact repetition (still lifes, short oscillators).
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for chunk in self.cells.chunks(8) {
            let mut word = 0u64;
            for &b in chunk {
                word = word << 1 | b as u64;
            }
            hash = (hash ^ word).wrapping_mul(0x0100_0000_01b3).rotate_left(7);
        }
        if self.hashes.contains(&hash) {
            self.repeats += 1;
        } else {
            self.repeats = 0;
        }
        self.hashes[self.hash_pos] = hash;
        self.hash_pos = (self.hash_pos + 1) % HISTORY;

        // Flat population (lone gliders, sparse debris).
        self.pops[self.pop_pos] = pop;
        self.pop_pos = (self.pop_pos + 1) % POP_WINDOW;
        let flat = if self.pops.contains(&u32::MAX) {
            false
        } else {
            let lo = *self.pops.iter().min().unwrap_or(&0);
            let hi = *self.pops.iter().max().unwrap_or(&0);
            hi - lo <= (hi / 50).max(3)
        };
        self.repeats > 40 || flat
    }

    fn paint(&mut self) {
        // Color by age, computed once per frame.
        let mut lut = [Rgb::BLACK; AGE_STEPS + 1];
        for (a, c) in lut.iter_mut().enumerate() {
            let a = a as f32;
            *c = if a <= 1.0 {
                Rgb::hsv(self.hue, 0.15, 1.0)
            } else {
                let k = a / AGE_STEPS as f32;
                let sat = 0.55 + 0.4 * (a / 8.0).min(1.0);
                Rgb::hsv(self.hue + 0.35 * k, sat, 1.0 - 0.35 * k)
            };
        }
        for i in 0..self.cells.len() {
            if self.cells[i] == 1 {
                self.px[i] = lut[(self.age[i] as usize).min(AGE_STEPS)];
            } else {
                let p = &mut self.px[i];
                *p = Rgb(
                    (p.0 as u16 * 200 >> 8) as u8,
                    (p.1 as u16 * 200 >> 8) as u8,
                    (p.2 as u16 * 215 >> 8) as u8,
                );
            }
        }
    }
}

impl Animation for Life {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        if self.fade > 0 {
            // Dim everything, then start a new epoch.
            for p in &mut self.px {
                *p = p.scale(0.82);
            }
            self.fade -= 1;
            if self.fade == 0 {
                self.reseed(rng);
                self.paint();
            }
            c.blit_pixels(&self.px);
            return;
        }

        let pop = self.evolve();
        self.gen += 1;
        self.since_inject += 1;
        let area = (self.w * self.h) as u32;

        if pop == 0 || self.gen > MAX_EPOCH {
            self.fade = FADE_FRAMES;
        } else if self.stagnant(pop) {
            if self.gen > 900 || pop < area / 100 + 3 {
                self.fade = FADE_FRAMES;
            } else {
                self.inject(rng);
            }
        } else if self.since_inject > 150 && pop < area / 25 && rng.chance(0.02) {
            self.inject(rng);
        }

        // Slow hue drift within an epoch.
        self.hue = (self.hue + 0.0004) % 1.0;
        self.paint();
        c.blit_pixels(&self.px);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    let ph = h * 2;
    let n = w * ph;
    let mut life = Life {
        w,
        h: ph,
        cells: vec![0; n],
        next: vec![0; n],
        age: vec![0; n],
        px: vec![Rgb::BLACK; n],
        hue: 0.0,
        hashes: [0; HISTORY],
        hash_pos: 0,
        repeats: 0,
        pops: vec![u32::MAX; POP_WINDOW],
        pop_pos: 0,
        gen: 0,
        since_inject: 0,
        fade: 0,
    };
    life.reseed(rng);
    Box::new(life)
}
