//! Classic digital rain: falling heads leave fading trails of mutating glyphs.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;

pub const BG: Rgb = Rgb(0, 5, 2);
const HEAD: Rgb = Rgb(210, 255, 220);
const TRAIL: [Rgb; 4] = [Rgb(0, 40, 12), Rgb(0, 110, 35), Rgb(20, 200, 70), Rgb(120, 255, 150)];
const SYMBOLS: [char; 14] = ['=', '+', '*', ':', '.', '"', '<', '>', '|', 'Z', '¦', '-', '_', '╌'];

struct Drop {
    x: usize,
    y: f32,
    speed: f32,
}

struct Matrix {
    w: usize,
    h: usize,
    glyph: Vec<char>,
    heat: Vec<f32>,
    decay: Vec<f32>,
    drops: Vec<Drop>,
}

fn glyph(rng: &mut Rng) -> char {
    match rng.below(10) {
        0..=6 => char::from_u32(0xFF66 + rng.below(0x38) as u32).unwrap_or('0'),
        7 | 8 => (b'0' + rng.below(10) as u8) as char,
        _ => *rng.pick(&SYMBOLS),
    }
}

impl Matrix {
    fn spawn(&mut self, x: usize, rng: &mut Rng) {
        let speed = rng.rangef(0.25, 1.1);
        // Trail length in cells, converted to a per-frame decay for this speed.
        let len = rng.rangef(5.0, (self.h as f32 * 0.9).max(6.0));
        let frames = (len / speed).max(1.0);
        self.decay[x] = (0.03f32.ln() / frames).exp();
        let y = -rng.rangef(0.0, 4.0);
        self.drops.push(Drop { x, y, speed });
    }
}

impl Animation for Matrix {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        let (w, h) = (self.w, self.h);

        // Random glyph mutations.
        for _ in 0..(w * h / 40 + 1) {
            let i = rng.below(w * h);
            self.glyph[i] = glyph(rng);
        }

        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let v = self.heat[i] * self.decay[x];
                self.heat[i] = if v < 0.02 { 0.0 } else { v };
            }
        }

        let busy = self.drops.len();
        for x in 0..w {
            if busy < w * 2 && rng.chance(0.012) {
                self.spawn(x, rng);
            }
        }

        let mut i = 0;
        while i < self.drops.len() {
            let d = &mut self.drops[i];
            let old = d.y.floor() as i32;
            d.y += d.speed;
            let new = d.y.floor() as i32;
            for row in (old + 1).max(0)..=new.min(h as i32 - 1) {
                let k = row as usize * w + d.x;
                self.heat[k] = 1.0;
                self.glyph[k] = glyph(rng);
            }
            if new >= h as i32 {
                self.drops.swap_remove(i);
            } else {
                i += 1;
            }
        }

        for y in 0..h {
            for x in 0..w {
                let k = y * w + x;
                let v = self.heat[k];
                if v <= 0.0 {
                    c.set(x as i32, y as i32, ' ', BG, BG);
                } else {
                    // Quantized so fading trails don't repaint every frame.
                    let q = (v.powf(1.3) * 12.0).round() / 12.0;
                    let col = Rgb::gradient(&TRAIL, q);
                    c.set(x as i32, y as i32, self.glyph[k], col, BG);
                }
            }
        }
        for d in &self.drops {
            let y = d.y.floor() as i32;
            if y >= 0 {
                let k = y as usize * w + d.x;
                c.set(d.x as i32, y, self.glyph[k], HEAD, BG);
            }
        }
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    let glyph = (0..w * h).map(|_| glyph(rng)).collect();
    Box::new(Matrix {
        w,
        h,
        glyph,
        heat: vec![0.0; w * h],
        decay: vec![0.9; w],
        drops: Vec::new(),
    })
}
