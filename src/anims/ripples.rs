//! Raindrops on a pond: a damped 2D wave equation on half-block pixels,
//! shaded by slope, with a few lily pads drifting on top.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;

const DAMP: f32 = 0.985;
const C2: f32 = 0.5;
const VISC: f32 = 0.06;
const SHALLOW: Rgb = Rgb(38, 104, 128);
const DEEP_BASE: Rgb = Rgb(8, 38, 62);
const DARK: Rgb = Rgb(2, 14, 28);
const HIGHLIGHT: Rgb = Rgb(200, 240, 250);
const PAD: Rgb = Rgb(46, 118, 58);
const PAD_DARK: Rgb = Rgb(26, 78, 40);

struct Pad {
    x: f32,
    y: f32,
    r: f32,
    vx: f32,
    vy: f32,
    notch: f32,
    flower: Option<Rgb>,
}

struct Ripples {
    w: usize,
    ph: usize,
    cur: Vec<f32>,
    prev: Vec<f32>,
    next: Vec<f32>,
    px: Vec<Rgb>,
    base: Vec<Rgb>,
    pads: Vec<Pad>,
    rain: f32,
    rain_target: f32,
    t: u32,
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    let ph = h * 2;
    let n = w * ph;
    let base = (0..ph)
        .map(|y| Rgb::gradient(&[SHALLOW, DEEP_BASE], y as f32 / ph.max(1) as f32))
        .collect();
    let pad_count = if w * ph > 800 { rng.range(1, 4) } else { 0 };
    let pads = (0..pad_count)
        .map(|_| Pad {
            x: rng.rangef(0.0, w as f32),
            y: rng.rangef(0.0, ph as f32),
            r: rng.rangef(3.0, 6.0),
            vx: rng.rangef(-0.02, 0.02),
            vy: rng.rangef(-0.015, 0.015),
            notch: rng.rangef(0.0, std::f32::consts::TAU),
            flower: if rng.chance(0.5) {
                Some(*rng.pick(&[Rgb(240, 150, 190), Rgb(250, 240, 245), Rgb(250, 200, 90)]))
            } else {
                None
            },
        })
        .collect();
    Box::new(Ripples {
        w,
        ph,
        cur: vec![0.0; n],
        prev: vec![0.0; n],
        next: vec![0.0; n],
        px: vec![Rgb::BLACK; n],
        base,
        pads,
        rain: 0.3,
        rain_target: 0.3,
        t: 0,
    })
}

impl Ripples {
    fn drop(&mut self, cx: i32, cy: i32, r: i32, amp: f32) {
        for dy in -r..=r {
            for dx in -r..=r {
                let d2 = (dx * dx + dy * dy) as f32;
                let rr = (r * r) as f32 + 0.5;
                if d2 > rr {
                    continue;
                }
                let (x, y) = (cx + dx, cy + dy);
                if x <= 0 || y <= 0 || x as usize >= self.w - 1 || y as usize >= self.ph - 1 {
                    continue;
                }
                let i = y as usize * self.w + x as usize;
                let k = (d2 / rr * std::f32::consts::FRAC_PI_2).cos();
                self.cur[i] += amp * k * k;
            }
        }
    }

    /// Leapfrog wave equation with an isotropic 9-point Laplacian and a
    /// touch of viscosity so high-frequency noise dies out first.
    fn simulate(&mut self) {
        let w = self.w;
        if w < 3 || self.ph < 3 {
            return;
        }
        let lap = |b: &[f32], i: usize| {
            (4.0 * (b[i - 1] + b[i + 1] + b[i - w] + b[i + w])
                + (b[i - w - 1] + b[i - w + 1] + b[i + w - 1] + b[i + w + 1])
                - 20.0 * b[i])
                / 6.0
        };
        for y in 1..self.ph - 1 {
            let row = y * w;
            for x in 1..w - 1 {
                let i = row + x;
                let (c, p) = (self.cur[i], self.prev[i]);
                let lc = lap(&self.cur, i);
                let lp = lap(&self.prev, i);
                self.next[i] = (2.0 * c - p + C2 * lc + VISC * (lc - lp)) * DAMP;
            }
        }
        std::mem::swap(&mut self.prev, &mut self.cur);
        std::mem::swap(&mut self.cur, &mut self.next);
    }

    fn shade(&mut self) {
        let (w, ph) = (self.w, self.ph);
        for y in 0..ph {
            let up = y.saturating_sub(1) * w;
            let down = (y + 1).min(ph - 1) * w;
            let row = y * w;
            let base = self.base[y];
            for x in 0..w {
                let l = x.saturating_sub(1);
                let r = (x + 1).min(w - 1);
                let sx = self.cur[row + l] - self.cur[row + r];
                let sy = self.cur[up + x] - self.cur[down + x];
                // Quantize, with a dead zone, so calm water stops repainting.
                let light = ((sx * 0.6 + sy) * 0.4 * 24.0).round() / 24.0;
                self.px[row + x] = if light.abs() < 0.05 {
                    base
                } else if light > 0.0 {
                    base.lerp(HIGHLIGHT, light * light.min(1.0) + light * 0.3)
                } else {
                    base.lerp(DARK, -light * 0.8)
                };
            }
        }
    }

    fn draw_pads(&mut self) {
        let (w, ph) = (self.w as i32, self.ph as i32);
        for p in &self.pads {
            let ri = p.r.ceil() as i32;
            let (cx, cy) = (p.x.round() as i32, p.y.round() as i32);
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let (x, y) = (cx + dx, cy + dy);
                    if x < 0 || y < 0 || x >= w || y >= ph {
                        continue;
                    }
                    let d = ((dx * dx + dy * dy) as f32).sqrt();
                    if d > p.r {
                        continue;
                    }
                    // A wedge-shaped notch makes it read as a lily pad.
                    let a = (dy as f32).atan2(dx as f32);
                    let mut da = (a - p.notch).abs();
                    if da > std::f32::consts::PI {
                        da = std::f32::consts::TAU - da;
                    }
                    if da < 0.2 && d > 0.8 {
                        continue;
                    }
                    let i = (y * w + x) as usize;
                    let bob = (self.cur[i] * 0.15).clamp(-0.3, 0.3);
                    let c = PAD.lerp(PAD_DARK, d / p.r * 0.6 + 0.2 - bob);
                    self.px[i] = c;
                }
            }
            if let Some(f) = p.flower {
                for (dx, dy) in [(0, 0), (1, 0), (0, -1)] {
                    let (x, y) = (cx + dx, cy + dy);
                    if x >= 0 && y >= 0 && x < w && y < ph {
                        self.px[(y * w + x) as usize] = f;
                    }
                }
            }
        }
    }
}

impl Animation for Ripples {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        if self.w == 0 || self.ph == 0 {
            return;
        }
        self.t += 1;
        if self.t % 150 == 0 {
            self.rain_target = if rng.chance(0.2) { rng.rangef(0.6, 1.2) } else { rng.rangef(0.05, 0.45) };
        }
        self.rain += (self.rain_target - self.rain) * 0.01;

        let area = (self.w * self.ph) as f32;
        let mut expected = self.rain * area / 20000.0 + 0.015;
        while expected > 0.0 {
            if rng.chance(expected.min(1.0)) {
                let x = rng.range(1, self.w as i32 - 1);
                let y = rng.range(1, self.ph as i32 - 1);
                let (r, amp) = if rng.chance(0.04) { (4, -6.0) } else { (2, rng.rangef(-3.0, -1.5)) };
                self.drop(x, y, r, amp);
            }
            expected -= 1.0;
        }

        self.simulate();
        self.shade();

        let (w, ph) = (self.w as f32, self.ph as f32);
        for p in &mut self.pads {
            p.x += p.vx;
            p.y += p.vy;
            if p.x < p.r || p.x > w - p.r {
                p.vx = -p.vx;
                p.x = p.x.clamp(p.r, (w - p.r).max(p.r));
            }
            if p.y < p.r || p.y > ph - p.r {
                p.vy = -p.vy;
                p.y = p.y.clamp(p.r, (ph - p.r).max(p.r));
            }
            p.notch += 0.002;
        }
        self.draw_pads();
        c.blit_pixels(&self.px);
    }
}
