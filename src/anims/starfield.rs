//! 3D starfield: stars fly out of a drifting vanishing point, stretching into
//! streaks whenever the ship eases into warp.
//!
//! Drawn in half-block pixels: distant stars are faint single pixels that
//! twinkle, near ones grow brighter and wider, and every star is motion
//! blurred along its path as an anti-aliased line that becomes a long streak
//! at warp speed.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;
use std::f32::consts::TAU;

pub const BG: Rgb = Rgb(1, 1, 8);
const FAR: Rgb = Rgb(35, 40, 75);
const TINTS: [Rgb; 5] = [
    Rgb(255, 255, 255),
    Rgb(200, 220, 255),
    Rgb(170, 200, 255),
    Rgb(255, 240, 210),
    Rgb(255, 220, 190),
];
/// Streak tails shift towards this blue at warp.
const WARP_TINT: [f32; 3] = [0.55, 0.7, 1.0];
const PERIOD: f32 = 1100.0;
/// Longest streak in pixels.
const MAX_STREAK: f32 = 90.0;

struct Star {
    x: f32,
    y: f32,
    z: f32,
    tint: [f32; 3],
    /// Intrinsic brightness, so stars at the same depth still differ.
    mag: f32,
    phase: f32,
    twinkle: f32,
}

struct Starfield {
    w: usize,
    ph: usize,
    stars: Vec<Star>,
    /// Additive light per pixel, as floats.
    acc: Vec<[f32; 3]>,
    px: Vec<Rgb>,
    t: f32,
    spread_y: f32,
}

fn rgb(c: Rgb) -> [f32; 3] {
    [c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0]
}

impl Starfield {
    fn spawn(&self, rng: &mut Rng, z: f32) -> Star {
        Star {
            x: rng.rangef(-1.0, 1.0),
            y: rng.rangef(-self.spread_y, self.spread_y),
            z,
            tint: rgb(*rng.pick(&TINTS)),
            mag: rng.rangef(0.45, 1.0),
            phase: rng.f32() * TAU,
            twinkle: rng.rangef(0.03, 0.12),
        }
    }

    /// Add light to a single pixel.
    #[inline]
    fn plot(&mut self, x: i32, y: i32, c: [f32; 3], a: f32) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.ph {
            let p = &mut self.acc[y as usize * self.w + x as usize];
            for k in 0..3 {
                p[k] += c[k] * a;
            }
        }
    }

    /// Spread light at a sub-pixel position over the four nearest pixels.
    #[inline]
    fn splat(&mut self, x: f32, y: f32, c: [f32; 3], a: f32) {
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (x0, y0) = (x0 as i32, y0 as i32);
        self.plot(x0, y0, c, a * (1.0 - tx) * (1.0 - ty));
        self.plot(x0 + 1, y0, c, a * tx * (1.0 - ty));
        self.plot(x0, y0 + 1, c, a * (1.0 - tx) * ty);
        self.plot(x0 + 1, y0 + 1, c, a * tx * ty);
    }
}

impl Animation for Starfield {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        self.t += 1.0;
        let t = self.t;
        let (w, ph) = (self.w as f32, self.ph as f32);
        // Mostly cruising, with a warp burst once per period.
        let phase = 0.5 - 0.5 * (t * TAU / PERIOD).cos();
        let warp = phase * phase * phase;
        let speed = 0.004 + 0.035 * warp;
        let cx = w * 0.5 + (t * 0.0021).sin() * w * 0.12;
        let cy = ph * 0.5 + (t * 0.0033).sin() * ph * 0.12;
        let f = w * 0.5;
        // Streaks span this many frames of motion: a blur while cruising,
        // long light trails at warp.
        let blur = 1.0 + 5.0 * warp;
        let trail = 0.3 + 1.2 * warp.min(1.0);

        let far = rgb(FAR);
        self.acc.fill([0.0; 3]);
        for i in 0..self.stars.len() {
            let s = &mut self.stars[i];
            s.z -= speed;
            let sx = cx + s.x / s.z * f;
            let sy = cy + s.y / s.z * f;
            if s.z < 0.02 || sx < -1.0 || sy < -1.0 || sx > w + 1.0 || sy > ph + 1.0 {
                let z = rng.rangef(0.85, 1.0);
                let fresh = self.spawn(rng, z);
                self.stars[i] = fresh;
                continue;
            }
            let s = &self.stars[i];
            let near = 1.0 - s.z;
            let k = near.powf(1.3);
            // Faint and bluish far away, full color and brightness up close.
            let b = s.mag * (0.35 + 0.65 * k);
            let mut col = [0.0; 3];
            for j in 0..3 {
                col[j] = (far[j] + (s.tint[j] - far[j]) * k) * b;
            }
            // Distant stars twinkle; close ones burn steadily.
            let tw = 1.0 - 0.45 * s.z * (0.5 + 0.5 * (t * s.twinkle + s.phase).sin());
            let (x, y, z) = (s.x, s.y, s.z);

            let tz = (z + speed * blur).min(1.2);
            let tx = cx + x / tz * f;
            let ty = cy + y / tz * f;
            let (mut dx, mut dy) = (sx - tx, sy - ty);
            let len = dx.abs().max(dy.abs());
            if len > 1.5 {
                if len > MAX_STREAK {
                    dx *= MAX_STREAK / len;
                    dy *= MAX_STREAK / len;
                }
                // One sample per pixel along the major axis, fading in from
                // the tail, gives an anti-aliased line.
                let n = len.min(MAX_STREAK).ceil() as i32;
                let a = trail * (0.15 + 0.85 * k);
                for j in 0..n {
                    let u = j as f32 / n as f32;
                    let mut tc = col;
                    for q in 0..3 {
                        tc[q] += (WARP_TINT[q] * col[0].max(col[2]) - col[q]) * (1.0 - u) * warp;
                    }
                    self.splat(sx - dx * (1.0 - u), sy - dy * (1.0 - u), tc, a * u * u.sqrt());
                }
            }

            // Heads grow with proximity: a pixel, an anti-aliased dot, then a
            // dot with a soft cross.
            if z > 0.35 {
                self.plot(sx.floor() as i32, sy.floor() as i32, col, tw);
            } else if z > 0.15 {
                self.splat(sx, sy, col, 1.6);
            } else {
                self.splat(sx, sy, col, 1.8);
                for (ox, oy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                    self.splat(sx + ox, sy + oy, col, 0.35);
                }
            }
        }

        for (p, a) in self.px.iter_mut().zip(&self.acc) {
            let m = a[0].max(a[1]).max(a[2]);
            // Snap faint light to the exact background so it stays see-through.
            *p = if m * 255.0 < 3.0 {
                BG
            } else {
                let v = |x: f32, b: u8| (b as f32 + x.min(1.0) * 255.0) as u8;
                Rgb(v(a[0], BG.0), v(a[1], BG.1), v(a[2], BG.2))
            };
        }
        c.blit_pixels(&self.px);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    let ph = 2 * h;
    let mut s = Starfield {
        w,
        ph,
        stars: Vec::new(),
        acc: vec![[0.0; 3]; w * ph],
        px: vec![BG; w * ph],
        t: 0.0,
        spread_y: (ph as f32 / w.max(1) as f32).max(0.2),
    };
    let n = (w * h / 8).clamp(8, 1400);
    for _ in 0..n {
        let z = rng.rangef(0.05, 1.0);
        let star = s.spawn(rng, z);
        s.stars.push(star);
    }
    Box::new(s)
}
