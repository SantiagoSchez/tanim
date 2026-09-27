//! Northern lights over mountains, mirrored in a still lake.
//!
//! An aurora is a thin sheet of light seen almost edge on, so instead of
//! painting bands each curtain is a wavy line (its track across the sky) that
//! is walked in small steps, every sample dropping light into the screen
//! column it lands on. Where the line folds back on itself many samples pile
//! into the same columns, which is exactly where real curtains look brightest:
//! the folds and bright creases come out of the geometry for free. Each column
//! then glows upwards from the curtain's sharp lower edge, green low down and
//! fading through teal into violet and red at the top.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;
use std::f32::consts::PI;

const SKY_TOP: Rgb = Rgb(2, 3, 12);
const SKY_LOW: Rgb = Rgb(10, 20, 36);
const FAR_MOUNT: Rgb = Rgb(18, 24, 42);
const NEAR_MOUNT: Rgb = Rgb(8, 11, 20);
const SNOW: Rgb = Rgb(58, 70, 92);
const WATER: Rgb = Rgb(3, 7, 16);
/// Aurora colors from the lower edge upwards.
const GLOW: [Rgb; 4] = [Rgb(90, 255, 150), Rgb(40, 220, 170), Rgb(120, 90, 230), Rgb(210, 60, 120)];

struct Curtain {
    /// Screen x where the track starts and its length.
    x0: f32,
    len: f32,
    /// Lower edge height (pixel row) and how far the light reaches upwards.
    base: f32,
    tall: f32,
    /// Ripples along the track: amplitude, spatial frequency, speed.
    ripples: [(f32, f32, f32); 3],
    drift: f32,
    bright: f32,
    age: u32,
    life: u32,
    phase: f32,
}

struct Star {
    x: i32,
    y: i32,
    b: f32,
    phase: f32,
}

struct Meteor {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: u32,
}

struct Aurora {
    w: usize,
    ph: usize,
    horizon: usize,
    far: Vec<i32>,
    near: Vec<i32>,
    /// Pine silhouettes on the near shore: (x, height).
    pines: Vec<(i32, i32)>,
    stars: Vec<Star>,
    curtains: Vec<Curtain>,
    meteor: Option<Meteor>,
    /// Per-column light and weighted lower edge for the curtain being drawn.
    light: Vec<f32>,
    edge: Vec<f32>,
    /// Additive aurora light per sky pixel, as floats.
    glow: Vec<[f32; 3]>,
    px: Vec<Rgb>,
    t: f32,
    /// Occasional substorm: brightens and quickens everything, then decays.
    surge: f32,
}

/// 1D midpoint displacement: a jagged ridge line of `n` heights in `[0, 1]`.
fn ridge(n: usize, rough: f32, rng: &mut Rng) -> Vec<f32> {
    let mut size = 1;
    while size + 1 < n {
        size *= 2;
    }
    let mut v = vec![0.0f32; size + 1];
    v[0] = rng.rangef(0.2, 0.8);
    v[size] = rng.rangef(0.2, 0.8);
    let mut step = size;
    let mut amp = 0.6;
    while step > 1 {
        let half = step / 2;
        let mut i = half;
        while i < size {
            v[i] = (v[i - half] + v[i + half]) / 2.0 + rng.rangef(-amp, amp);
            i += step;
        }
        amp *= rough;
        step = half;
    }
    let (lo, hi) = v.iter().fold((f32::MAX, f32::MIN), |(a, b), &x| (a.min(x), b.max(x)));
    let span = (hi - lo).max(1e-3);
    (0..n).map(|i| (v[i * size / n.max(1)] - lo) / span).collect()
}

impl Aurora {
    fn new(w: usize, h: usize, rng: &mut Rng) -> Aurora {
        let ph = 2 * h;
        let horizon = ph * 62 / 100;
        let hz = horizon as f32;
        let far: Vec<i32> = ridge(w, 0.55, rng)
            .iter()
            .map(|v| (hz - ph as f32 * (0.06 + 0.22 * v)) as i32)
            .collect();
        let near: Vec<i32> = ridge(w, 0.45, rng)
            .iter()
            .map(|v| (hz - ph as f32 * (0.02 + 0.05 * v)) as i32)
            .collect();
        // A pine forest along the top of the near hills, dark against the
        // mountains behind.
        let mut pines = Vec::new();
        let mut x = rng.range(0, 3);
        while (x as usize) < w {
            if rng.chance(0.7) {
                pines.push((x, rng.range(2, (ph as i32 / 14).max(3))));
            }
            x += rng.range(1, 4);
        }
        let stars = (0..(w * horizon / 22).max(1))
            .map(|_| Star {
                x: rng.range(0, w as i32),
                y: rng.range(0, (horizon as i32 - 2).max(1)),
                b: rng.rangef(0.25, 1.0),
                phase: rng.rangef(0.0, 6.3),
            })
            .collect();
        let mut a = Aurora {
            w,
            ph,
            horizon,
            far,
            near,
            pines,
            stars,
            curtains: Vec::new(),
            meteor: None,
            light: vec![0.0; w],
            edge: vec![0.0; w],
            glow: vec![[0.0; 3]; w * horizon],
            px: vec![Rgb::BLACK; w * ph],
            t: rng.rangef(0.0, 1000.0),
            surge: 0.0,
        };
        for i in 0..4 {
            let mut c = a.curtain(rng);
            // Start mid-life so the sky is not empty at first.
            c.age = c.life / 3 + i * 40;
            a.curtains.push(c);
        }
        a
    }

    fn curtain(&self, rng: &mut Rng) -> Curtain {
        let (w, ph) = (self.w as f32, self.ph as f32);
        let len = w * rng.rangef(0.5, 1.1);
        // The first ripple is strong enough (amplitude x frequency > 1) to
        // make the track double back on itself: those are the folds.
        let k0 = rng.rangef(0.02, 0.05) * 80.0 / w.max(20.0);
        let a0 = rng.rangef(0.9, 1.6) / k0;
        Curtain {
            x0: rng.rangef(-0.2 * w, 0.7 * w),
            len,
            base: ph * rng.rangef(0.2, 0.38),
            tall: ph * rng.rangef(0.1, 0.2),
            ripples: [
                (a0.min(w * 0.3), k0, rng.rangef(0.004, 0.012)),
                (rng.rangef(2.0, 6.0), rng.rangef(0.08, 0.2), rng.rangef(-0.03, 0.03)),
                (rng.rangef(0.5, 2.0), rng.rangef(0.3, 0.6), rng.rangef(0.02, 0.06)),
            ],
            drift: rng.rangef(-0.03, 0.03),
            bright: rng.rangef(0.6, 1.0),
            age: 0,
            life: rng.range(900, 2200) as u32,
            phase: rng.rangef(0.0, 6.3),
        }
    }

    fn update(&mut self, rng: &mut Rng) {
        let quick = 1.0 + 1.5 * self.surge;
        self.t += quick;
        self.surge *= 0.994;
        if rng.chance(0.0008) {
            self.surge = 1.0;
        }
        for i in 0..self.curtains.len() {
            let c = &mut self.curtains[i];
            c.age += 1;
            c.x0 += c.drift * quick;
            if c.age >= c.life {
                self.curtains[i] = self.curtain(rng);
            }
        }
        match &mut self.meteor {
            Some(m) => {
                m.x += m.vx;
                m.y += m.vy;
                m.age += 1;
                if m.age > 26 {
                    self.meteor = None;
                }
            }
            None => {
                if rng.chance(0.003) {
                    let right = rng.chance(0.5);
                    self.meteor = Some(Meteor {
                        x: rng.rangef(0.1, 0.9) * self.w as f32,
                        y: rng.rangef(0.0, 0.3) * self.horizon as f32,
                        vx: if right { 2.2 } else { -2.2 },
                        vy: 0.9,
                        age: 0,
                    });
                }
            }
        }
    }

    /// Accumulate every curtain's light into `glow`.
    fn draw_curtains(&mut self) {
        for g in self.glow.iter_mut() {
            *g = [0.0; 3];
        }
        let (w, hz) = (self.w, self.horizon);
        let t = self.t;
        let surge = 1.0 + 1.2 * self.surge;
        for ci in 0..self.curtains.len() {
            self.light.fill(0.0);
            self.edge.fill(0.0);
            let c = &self.curtains[ci];
            let fade = {
                let a = c.age as f32 / c.life as f32;
                (a * 6.0).min(1.0).min((1.0 - a) * 6.0).max(0.0)
            };
            let bright = c.bright * fade * surge * (0.75 + 0.25 * (t * 0.013 + c.phase).sin());
            if bright <= 0.01 {
                continue;
            }
            let steps = (c.len * 3.0) as usize;
            let (base, tall) = (c.base, c.tall);
            for i in 0..steps {
                let s = i as f32 / 3.0;
                let mut x = c.x0 + s;
                for &(amp, k, speed) in &c.ripples {
                    x += amp * (k * s + speed * t + c.phase).sin();
                }
                // Taper both ends of the curtain.
                let u = s / c.len;
                let wgt = (PI * u).sin().max(0.0).sqrt() / 3.0;
                let e = base + 0.03 * self.ph as f32 * (0.05 * s + 0.01 * t + c.phase).sin();
                let xi = x.floor();
                let fr = x - xi;
                for (xx, part) in [(xi as i32, 1.0 - fr), (xi as i32 + 1, fr)] {
                    if xx >= 0 && (xx as usize) < w {
                        self.light[xx as usize] += wgt * part;
                        self.edge[xx as usize] += e * wgt * part;
                    }
                }
            }
            for x in 0..w {
                let l = self.light[x];
                if l < 1e-3 {
                    continue;
                }
                let edge = self.edge[x] / l;
                // Vertical rays: the light shimmers column by column.
                let xf = x as f32;
                let rays = 0.7 + 0.3 * (xf * 0.9 + t * 0.07).sin() * (xf * 0.23 - t * 0.031).sin();
                let power = l.min(3.0) * bright * rays;
                let top = (edge - tall * 3.5).max(0.0) as usize;
                let bottom = ((edge + 3.0) as usize).min(hz);
                for y in top..bottom {
                    let d = edge - y as f32;
                    // Sharp lower edge, long exponential fade upwards.
                    let prof = if d < 0.0 { (d * 1.2).exp() } else { (-d / tall).exp() * (1.0 - (-(d + 1.0) / 1.5).exp()) };
                    let col = Rgb::gradient(&GLOW, (d / (tall * 3.0)).clamp(0.0, 1.0));
                    let a = prof * power * 0.9;
                    let g = &mut self.glow[y * w + x];
                    g[0] += col.0 as f32 * a;
                    g[1] += col.1 as f32 * a;
                    g[2] += col.2 as f32 * a;
                }
            }
        }
    }

    fn draw(&mut self) {
        let (w, ph, hz) = (self.w, self.ph, self.horizon);
        let t = self.t;
        self.draw_curtains();
        // Sky, stars and aurora light.
        for y in 0..hz {
            let base = SKY_TOP.lerp(SKY_LOW, y as f32 / hz.max(1) as f32);
            for x in 0..w {
                self.px[y * w + x] = base;
            }
        }
        for s in &self.stars {
            let tw = 0.7 + 0.3 * (t * 0.05 + s.phase).sin();
            let i = s.y as usize * w + s.x as usize;
            self.px[i] = self.px[i].lerp(Rgb(230, 235, 255), s.b * tw);
        }
        if let Some(m) = &self.meteor {
            for k in 0..8 {
                let (x, y) = ((m.x - m.vx * k as f32 * 0.8) as i32, (m.y - m.vy * k as f32 * 0.8) as i32);
                if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < hz {
                    let i = y as usize * w + x as usize;
                    let a = (1.0 - k as f32 / 8.0) * (1.0 - m.age as f32 / 27.0);
                    self.px[i] = self.px[i].lerp(Rgb(255, 250, 230), a);
                }
            }
        }
        let mut ambient = [0.0f32; 3];
        for (p, g) in self.px.iter_mut().zip(&self.glow) {
            *p = Rgb(
                (p.0 as f32 + g[0]).min(255.0) as u8,
                (p.1 as f32 + g[1]).min(255.0) as u8,
                (p.2 as f32 + g[2]).min(255.0) as u8,
            );
            for k in 0..3 {
                ambient[k] += g[k];
            }
        }
        // The aurora faintly lights the snow and the mountainsides.
        let n = (w * hz).max(1) as f32;
        let amb = Rgb(
            (ambient[0] / n * 1.5).min(60.0) as u8,
            (ambient[1] / n * 1.5).min(60.0) as u8,
            (ambient[2] / n * 1.5).min(60.0) as u8,
        );
        // Snow caps only the peaks that rise above the snow line.
        let snowline = hz as f32 - ph as f32 * 0.17;
        for x in 0..w {
            let (f, nr) = (self.far[x].max(0) as usize, self.near[x].max(0) as usize);
            let cap = ((snowline - f as f32) * 0.7).max(0.0) as usize;
            for y in f..hz {
                let base = if y < f + cap { SNOW } else { FAR_MOUNT };
                self.px[y * w + x] = base.add(amb);
            }
            for y in nr..hz {
                self.px[y * w + x] = NEAR_MOUNT.add(Rgb(amb.0 / 3, amb.1 / 3, amb.2 / 3));
            }
        }
        for &(x, hgt) in &self.pines {
            let foot = self.near[(x.max(0) as usize).min(w - 1)] + 1;
            for k in 0..hgt {
                let half = (hgt - k) / 3;
                for dx in -half..=half {
                    let (xx, yy) = (x + dx, foot - k);
                    if xx >= 0 && (xx as usize) < w && yy >= 0 {
                        self.px[yy as usize * w + xx as usize] = Rgb(4, 6, 10);
                    }
                }
            }
        }
        // The lake: the scene above, flipped, rippled and darkened.
        let depth = (ph - hz).max(1) as f32;
        for y in hz..ph {
            let d = (y - hz) as f32 / depth;
            let src_y = hz as i32 - 1 - (y - hz) as i32;
            let dim = 0.6 - 0.3 * d;
            // Rows ripple sideways, more as the water comes closer.
            let wobble = (y as f32 * 0.7 + t * 0.09).sin() * (0.4 + 2.2 * d);
            for x in 0..w {
                let sx = (x as f32 + wobble).round().clamp(0.0, w as f32 - 1.0) as usize;
                let sy = src_y.clamp(0, hz as i32 - 1) as usize;
                let sy2 = sy.saturating_sub(1);
                // Average two rows so stars soften instead of sparkling noise.
                let src = if hz > 0 { self.px[sy * w + sx].lerp(self.px[sy2 * w + sx], 0.5) } else { WATER };
                let mut c = WATER.lerp(src, dim);
                // Glints on the ripples.
                if (x as f32 * 0.11 + y as f32 * 1.7 + t * 0.05).sin() > 0.985 {
                    c = c.lerp(Rgb(150, 190, 220), 0.25);
                }
                self.px[y * w + x] = c;
            }
        }
    }
}

impl Animation for Aurora {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        self.update(rng);
        if self.w == 0 || self.ph == 0 {
            return;
        }
        self.draw();
        c.blit_pixels(&self.px);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    Box::new(Aurora::new(w, h, rng))
}
