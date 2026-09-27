//! Lava lamp: metaball blobs heat up in the pool at the bottom, rise, cool
//! off near the top and sink back, merging and splitting on the way.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;

#[derive(Clone, Copy)]
struct Scheme {
    bg_top: Rgb,
    bg_bot: Rgb,
    edge: Rgb,
    core: Rgb,
    glow: Rgb,
}

const fn sc(bg_top: Rgb, bg_bot: Rgb, edge: Rgb, core: Rgb, glow: Rgb) -> Scheme {
    Scheme { bg_top, bg_bot, edge, core, glow }
}

const SCHEMES: &[Scheme] = &[
    sc(Rgb(18, 4, 28), Rgb(90, 20, 40), Rgb(235, 60, 20), Rgb(255, 200, 70), Rgb(190, 40, 40)),
    sc(Rgb(0, 8, 28), Rgb(0, 45, 70), Rgb(30, 200, 130), Rgb(200, 255, 160), Rgb(20, 120, 120)),
    sc(Rgb(14, 0, 24), Rgb(60, 10, 70), Rgb(230, 50, 170), Rgb(255, 185, 235), Rgb(140, 30, 140)),
    sc(Rgb(4, 8, 36), Rgb(20, 40, 100), Rgb(255, 110, 0), Rgb(255, 230, 120), Rgb(120, 60, 70)),
    sc(Rgb(8, 18, 4), Rgb(40, 60, 10), Rgb(190, 225, 20), Rgb(255, 255, 170), Rgb(90, 110, 10)),
    sc(Rgb(25, 0, 0), Rgb(70, 10, 0), Rgb(250, 30, 60), Rgb(255, 160, 150), Rgb(160, 20, 30)),
];

fn mix(a: &Scheme, b: &Scheme, t: f32) -> Scheme {
    Scheme {
        bg_top: a.bg_top.lerp(b.bg_top, t),
        bg_bot: a.bg_bot.lerp(b.bg_bot, t),
        edge: a.edge.lerp(b.edge, t),
        core: a.core.lerp(b.core, t),
        glow: a.glow.lerp(b.glow, t),
    }
}

struct Blob {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    r2: f32,
    temp: f32,
    heat_rate: f32,
    cool_rate: f32,
    lane: f32,
}

struct Lava {
    w: usize,
    ph: usize,
    blobs: Vec<Blob>,
    px: Vec<Rgb>,
    field: Vec<f32>,
    wave: Vec<f32>,
    cur: usize,
    next: usize,
    blend: f32,
    hold: u32,
    t: f32,
    vmax: f32,
    pool: f32,
}

impl Animation for Lava {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        let (w, ph) = (self.w, self.ph);
        let (fw, fh) = (w as f32, ph as f32);
        self.t = (self.t + 1.0) % 4_000_000.0;
        let t = self.t;

        // Scheme schedule: hold ~40s, crossfade over ~5s.
        if self.blend > 0.0 {
            self.blend += 1.0 / 150.0;
            if self.blend >= 1.0 {
                self.cur = self.next;
                self.blend = 0.0;
                self.hold = 0;
            }
        } else {
            self.hold += 1;
            if self.hold > 1200 {
                self.next = (self.cur + 1 + rng.below(SCHEMES.len() - 1)) % SCHEMES.len();
                self.blend = 1.0 / 150.0;
            }
        }
        let s = mix(&SCHEMES[self.cur], &SCHEMES[self.next], self.blend);

        // Physics: heat in the bottom zone, cool everywhere, buoyancy from temp.
        for b in &mut self.blobs {
            if b.y > fh * 0.82 {
                b.temp += b.heat_rate;
            } else {
                b.temp -= b.cool_rate;
            }
            b.temp = b.temp.clamp(0.0, 1.0);
            b.vy += -(b.temp - 0.5) * 0.03 * self.vmax;
            b.vy *= 0.985;
            b.vy = b.vy.clamp(-self.vmax, self.vmax);
            // Each blob drifts towards its own lane, which changes now and then.
            if rng.chance(0.002) {
                b.lane = fw * rng.rangef(0.12, 0.88);
            }
            b.vx += rng.rangef(-0.004, 0.004) + (b.lane - b.x) * 0.000_1 * self.vmax;
            b.vx *= 0.99;
            b.x += b.vx;
            b.y += b.vy;
            let r = b.r2.sqrt();
            let (lo, hi) = (r * 0.4, fh - r * 0.2);
            if b.y < lo {
                b.y = lo;
                b.vy = 0.0;
            } else if b.y > hi {
                b.y = hi;
                b.vy = 0.0;
            }
            let (l, rr) = (fw * 0.08 + r * 0.5, fw * 0.92 - r * 0.5);
            if l < rr {
                if b.x < l {
                    b.x = l;
                    b.vx = b.vx.abs();
                } else if b.x > rr {
                    b.x = rr;
                    b.vx = -b.vx.abs();
                }
            }
        }

        // Field evaluation with a compact-support kernel (1 - d²/R²)², R = 2r,
        // scaled so the surface (field = 1) sits at distance r.
        const K: f32 = 1.0 / 0.5625;
        for x in 0..w {
            let wv = self.pool * (1.0 + 0.25 * (x as f32 * 0.09 + t * 0.02).sin());
            self.wave[x] = wv * wv;
        }
        for y in 0..ph {
            let yf = y as f32;
            self.field.fill(0.0);
            for b in &self.blobs {
                let big_r2 = 4.0 * b.r2;
                let dy = yf - b.y;
                let dy2 = dy * dy;
                if dy2 >= big_r2 {
                    continue;
                }
                let half = (big_r2 - dy2).sqrt();
                let x0 = ((b.x - half).floor().max(0.0) as usize).min(w);
                let x1 = ((b.x + half).ceil().max(0.0) as usize + 1).min(w);
                let inv = 1.0 / big_r2;
                for x in x0..x1 {
                    let dx = x as f32 - b.x;
                    let q = 1.0 - (dx * dx + dy2) * inv;
                    if q > 0.0 {
                        self.field[x] += q * q * K;
                    }
                }
            }
            let depth = (fh - yf).max(0.5);
            let inv_d2 = 1.0 / (depth * depth);
            let vy = yf / fh;
            let bg = s.bg_top.lerp(s.bg_bot, vy * vy);
            let light = 0.75 + 0.35 * vy;
            let row = &mut self.px[y * w..(y + 1) * w];
            for x in 0..w {
                let v = self.field[x] + self.wave[x] * inv_d2;
                row[x] = if v < 0.35 {
                    bg
                } else if v >= 1.0 {
                    let k = ((v - 1.0) / 1.5).min(1.0);
                    s.edge.lerp(s.core, k).scale(light)
                } else {
                    let g = ((v - 0.35) / 0.65).clamp(0.0, 1.0);
                    let base = bg.lerp(s.glow, g * g * 0.55);
                    if v > 0.88 {
                        base.lerp(s.edge.scale(light), (v - 0.88) / 0.12)
                    } else {
                        base
                    }
                };
            }
        }
        c.blit_pixels(&self.px);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    let ph = h * 2;
    let (fw, fh) = (w as f32, ph as f32);
    let n = ((w * ph) / 3000).clamp(4, 12);
    let size = fh.max(fw * 0.5);
    let blobs = (0..n)
        .map(|_| {
            let r = size * rng.rangef(0.065, 0.13);
            Blob {
                x: fw * rng.rangef(0.15, 0.85),
                y: fh * rng.rangef(0.1, 0.95),
                vx: 0.0,
                vy: 0.0,
                r2: r * r,
                temp: rng.f32(),
                heat_rate: rng.rangef(0.004, 0.01),
                cool_rate: rng.rangef(0.0012, 0.0025),
                lane: fw * rng.rangef(0.12, 0.88),
            }
        })
        .collect();
    let cur = rng.below(SCHEMES.len());
    Box::new(Lava {
        w,
        ph,
        blobs,
        px: vec![Rgb::BLACK; w * ph],
        field: vec![0.0; w],
        wave: vec![0.0; w],
        cur,
        next: cur,
        blend: 0.0,
        hold: 0,
        t: 0.0,
        vmax: (fh / 250.0).max(0.08),
        pool: (fh * 0.07).max(1.0),
    })
}
