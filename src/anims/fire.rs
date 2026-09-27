//! Doom-style fire propagation with slowly shifting wind, a breathing base
//! and rising embers.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;

struct Ember {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
}

struct Fire {
    w: usize,
    ph: usize,
    heat: Vec<u8>,
    px: Vec<Rgb>,
    palette: [Rgb; 256],
    /// Maximum heat lost per pixel climbed; tuned so flames reach ~2/3 height.
    decay: f32,
    wind: f32,
    wind_target: f32,
    t: f32,
    embers: Vec<Ember>,
}

fn palette() -> [Rgb; 256] {
    let stops = [
        Rgb(0, 0, 0),
        Rgb(31, 7, 7),
        Rgb(87, 15, 7),
        Rgb(159, 31, 7),
        Rgb(207, 71, 7),
        Rgb(223, 111, 15),
        Rgb(223, 151, 31),
        Rgb(207, 183, 55),
        Rgb(239, 223, 127),
        Rgb(255, 255, 255),
    ];
    let mut p = [Rgb::BLACK; 256];
    for (i, c) in p.iter_mut().enumerate() {
        *c = Rgb::gradient(&stops, i as f32 / 255.0);
    }
    p
}

impl Fire {
    fn new(w: usize, h: usize, rng: &mut Rng) -> Self {
        let ph = h * 2;
        // Average loss per row is decay/2: flames reach ~3/4 of the height.
        let decay = (2.0 * 200.0 / (0.75 * ph as f32)).clamp(1.5, 60.0);
        Fire {
            w,
            ph,
            heat: vec![0; w * ph],
            px: vec![Rgb::BLACK; w * ph],
            palette: palette(),
            decay,
            wind: 0.0,
            wind_target: 0.0,
            t: rng.rangef(0.0, 100.0),
            embers: Vec::new(),
        }
    }
}

impl Animation for Fire {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        let (w, ph) = (self.w, self.ph);
        // Wrap long before f32 loses integer precision.
        self.t = (self.t + 1.0) % 4_000_000.0;
        let t = self.t;

        // Slowly wandering wind in [-0.6, 0.6].
        if rng.chance(0.01) {
            self.wind_target = rng.rangef(-0.6, 0.6);
        }
        self.wind += (self.wind_target - self.wind) * 0.01;

        // Breathing base: two slow sines plus per-column flicker.
        // Breathing: flames grow taller and shorter by modulating the decay.
        let breath = 0.85 + 0.15 * (t * 0.017).sin() + 0.08 * (t * 0.049).sin();
        let dmax = self.decay / breath;
        let base = ph - 1;
        for x in 0..w {
            let local = 0.92 + 0.08 * ((x as f32 * 0.07 + t * 0.03).sin());
            let v = 255.0 * local - rng.below(30) as f32;
            self.heat[base * w + x] = v.max(0.0) as u8;
        }

        // Propagate upwards: each pixel pulls from the one below it.
        let wind = self.wind;
        for y in 1..ph {
            // Pixels nobody writes into this frame (wind shifts) cool down
            // instead of freezing.
            let decay = dmax as u8;
            for v in &mut self.heat[(y - 1) * w..y * w] {
                *v = v.saturating_sub(decay);
            }
            for x in 0..w {
                let src = y * w + x;
                let v = self.heat[src];
                let dst_y = y - 1;
                if v == 0 {
                    self.heat[dst_y * w + x] = 0;
                    continue;
                }
                let r = rng.next_u64();
                let mut dx = (r & 3) as i32 - 1; // -1..=2, slight jitter
                if dx == 2 {
                    dx = 0;
                }
                let wr = ((r >> 8) & 0xff) as f32 / 255.0;
                if wr < wind.abs() {
                    dx += if wind > 0.0 { 1 } else { -1 };
                }
                // Wrap horizontally so wind never leaves a cold edge.
                let dx_pos = (x as i32 + dx).rem_euclid(w as i32);
                let loss = (((r >> 16) & 0xffff) as f32 * (dmax + 1.0) / 65536.0) as u8;
                self.heat[dst_y * w + dx_pos as usize] = v.saturating_sub(loss);
            }
        }

        // Embers: spawn in hot zones, float up and fade.
        if self.embers.len() < 40 + w / 4 && rng.chance(0.4) {
            let x = rng.below(w);
            let y = ph as f32 * rng.rangef(0.55, 0.95);
            self.embers.push(Ember {
                x: x as f32,
                y,
                vx: rng.rangef(-0.3, 0.3),
                vy: -rng.rangef(0.4, 1.1),
                life: 1.0,
            });
        }
        for e in &mut self.embers {
            e.vx += (wind * 0.08) + rng.rangef(-0.08, 0.08);
            e.vx *= 0.96;
            e.x += e.vx;
            e.y += e.vy;
            e.life -= 0.012 + rng.f32() * 0.01;
        }
        self.embers.retain(|e| e.life > 0.0 && e.y >= 0.0 && e.x >= 0.0 && e.x < w as f32);

        for (p, &h) in self.px.iter_mut().zip(&self.heat) {
            *p = self.palette[h as usize];
        }
        for e in &self.embers {
            let i = e.y as usize * w + e.x as usize;
            if i < self.px.len() {
                let glow = Rgb(255, 170, 60).scale(e.life);
                self.px[i] = self.px[i].add(glow);
            }
        }
        c.blit_pixels(&self.px);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    Box::new(Fire::new(w, h, rng))
}
