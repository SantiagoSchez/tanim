//! Flocking birds (separation, alignment, cohesion) on a wrapping dusk sky,
//! with a hawk that shows up now and then and scatters them.
//!
//! Drawn in half-block pixels: each bird is a tiny flapping "v" whose wings
//! sweep forward and back, leaving a soft fading trail behind it. World units
//! are pixels: x in columns, y in half-rows.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;
use std::f32::consts::TAU;

const SKY_TOP: Rgb = Rgb(6, 10, 28);
const SKY_LOW: Rgb = Rgb(30, 18, 42);
const STAR: Rgb = Rgb(150, 160, 200);
const HAWK: Rgb = Rgb(255, 60, 40);
const HUES: [f32; 3] = [0.1, 0.52, 0.88];
const VIEW: f32 = 9.0;
/// Birds keep this far apart, so each one reads as a separate chevron.
const PERSONAL: f32 = 4.8;
const FEAR: f32 = 16.0;
const MIN_SPEED: f32 = 0.3;
const MAX_SPEED: f32 = 0.75;
/// Per-frame fade of the motion trails.
const TRAIL_DECAY: f32 = 0.62;

#[derive(Clone, Copy)]
struct Boid {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    flap: f32,
    flock: u8,
}

struct Hawk {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    flap: f32,
    life: i32,
}

struct Boids {
    w: usize,
    ph: usize,
    ww: f32,
    wh: f32,
    boids: Vec<Boid>,
    hawk: Option<Hawk>,
    hawk_timer: i32,
    gw: usize,
    gh: usize,
    head: Vec<i32>,
    next: Vec<i32>,
    /// Per pixel trail strength and color.
    trail: Vec<(f32, Rgb)>,
    wander: Vec<f32>,
    /// Sky gradient with a sprinkling of faint stars; never changes.
    bg: Vec<Rgb>,
    px: Vec<Rgb>,
}

#[inline]
fn wrap_delta(d: f32, size: f32) -> f32 {
    if d > size * 0.5 {
        d - size
    } else if d < -size * 0.5 {
        d + size
    } else {
        d
    }
}

/// Neighbouring grid indices (with wrap), deduplicated for tiny grids.
fn around(g: i32, n: i32) -> ([i32; 3], usize) {
    let mut out = [0; 3];
    let mut k = 0;
    for o in -1..=1 {
        let v = (g + o).rem_euclid(n);
        if !out[..k].contains(&v) {
            out[k] = v;
            k += 1;
        }
    }
    (out, k)
}

fn color(b: &Boid) -> Rgb {
    let a = b.vy.atan2(b.vx) / TAU;
    Rgb::hsv(HUES[b.flock as usize % HUES.len()] + a * 0.08, 0.55, 1.0)
}

impl Boids {
    fn rebuild_grid(&mut self) {
        self.head.fill(-1);
        for (i, b) in self.boids.iter().enumerate() {
            let gx = ((b.x / VIEW) as usize).min(self.gw - 1);
            let gy = ((b.y / VIEW) as usize).min(self.gh - 1);
            let g = gy * self.gw + gx;
            self.next[i] = self.head[g];
            self.head[g] = i as i32;
        }
    }

    /// Blend `col` into the pixel under `(x, y)`, wrapping around the edges
    /// like the world does.
    fn dot(&mut self, x: f32, y: f32, col: Rgb, a: f32) {
        let xx = (x.round() as i32).rem_euclid(self.w as i32) as usize;
        let yy = (y.round() as i32).rem_euclid(self.ph as i32) as usize;
        let p = &mut self.px[yy * self.w + xx];
        *p = p.lerp(col, a);
    }

    /// A bird seen from below: a bright head and body along its heading, and
    /// wings that beat between spread wide and swept back as `flap` turns.
    /// Wings are drawn as lines, so bigger sizes stay solid.
    fn bird(&mut self, b: &Boid, size: f32, col: Rgb, a: f32) {
        let s = (b.vx * b.vx + b.vy * b.vy).sqrt().max(1e-4);
        let (fx, fy) = (b.vx / s, b.vy / s);
        let (qx, qy) = (-fy, fx);
        let up = 0.5 + 0.5 * b.flap.sin();
        let span = size * (0.6 + 0.7 * up);
        let back = size * (1.0 - 0.8 * up);
        let steps = (size * 2.0).ceil() as usize;
        // Big birds get broad wings (a second, leading edge line) and a tail.
        let edges: &[f32] = if size > 1.5 { &[0.0, 0.8] } else { &[0.0] };
        for &lead in edges {
            let (ox, oy) = (b.x + fx * lead, b.y + fy * lead);
            for side in [-1.0, 1.0] {
                let (tx, ty) = (-fx * back + qx * span * side, -fy * back + qy * span * side);
                // Small birds are just head and two wingtips: a crisp chevron.
                let first = if size > 1.5 { 1 } else { steps };
                for k in first..=steps {
                    let t = k as f32 / steps as f32;
                    self.dot(ox + tx * t * (1.0 - lead * 0.3), oy + ty * t * (1.0 - lead * 0.3), col.scale(1.0 - 0.25 * t), a);
                }
            }
        }
        if size > 1.5 {
            for k in 1..=2 {
                let d = size * 0.4 * k as f32;
                self.dot(b.x - fx * d, b.y - fy * d, col.scale(0.75), a);
            }
            self.dot(b.x + fx * size * 0.5, b.y + fy * size * 0.5, col, a);
        }
        self.dot(b.x, b.y, col.lerp(Rgb::WHITE, 0.3), a);
    }
}

impl Animation for Boids {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        let (ww, wh) = (self.ww, self.wh);

        // Hawk lifecycle.
        self.hawk_timer -= 1;
        if self.hawk.is_none() && self.hawk_timer <= 0 {
            let (x, y) = if rng.chance(0.5) { (0.0, rng.rangef(0.0, wh)) } else { (rng.rangef(0.0, ww), 0.0) };
            self.hawk = Some(Hawk { x, y, vx: 0.0, vy: 0.0, flap: 0.0, life: rng.range(300, 480) });
        }
        if let Some(hk) = &mut self.hawk {
            // Chase the nearest bird.
            let mut best = (f32::MAX, 0.0, 0.0);
            for b in &self.boids {
                let dx = wrap_delta(b.x - hk.x, ww);
                let dy = wrap_delta(b.y - hk.y, wh);
                let d = dx * dx + dy * dy;
                if d < best.0 {
                    best = (d, dx, dy);
                }
            }
            let d = best.0.sqrt().max(0.01);
            hk.vx += best.1 / d * 0.05;
            hk.vy += best.2 / d * 0.05;
            let s = (hk.vx * hk.vx + hk.vy * hk.vy).sqrt();
            if s > 0.85 {
                hk.vx *= 0.85 / s;
                hk.vy *= 0.85 / s;
            }
            hk.x = (hk.x + hk.vx).rem_euclid(ww);
            hk.y = (hk.y + hk.vy).rem_euclid(wh);
            // Slow, heavy wingbeats.
            hk.flap += 0.12;
            hk.life -= 1;
            if hk.life <= 0 {
                self.hawk = None;
                self.hawk_timer = rng.range(600, 1300);
            }
        }

        for a in &mut self.wander {
            *a += rng.rangef(-0.04, 0.04);
        }
        self.rebuild_grid();
        let (gw, gh) = (self.gw as i32, self.gh as i32);
        for i in 0..self.boids.len() {
            let me = self.boids[i];
            let (mut ax, mut ay, mut cx, mut cy, mut sx, mut sy) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
            let mut n = 0.0;
            let gx = ((me.x / VIEW) as i32).min(gw - 1);
            let gy = ((me.y / VIEW) as i32).min(gh - 1);
            let (cols, nc) = around(gx, gw);
            let (rows, nr) = around(gy, gh);
            for &cyg in &rows[..nr] {
                for &cxg in &cols[..nc] {
                    let mut j = self.head[(cyg * gw + cxg) as usize];
                    while j >= 0 {
                        let o = self.boids[j as usize];
                        j = self.next[j as usize];
                        let dx = wrap_delta(o.x - me.x, ww);
                        let dy = wrap_delta(o.y - me.y, wh);
                        let d2 = dx * dx + dy * dy;
                        if d2 == 0.0 || d2 > VIEW * VIEW {
                            continue;
                        }
                        if d2 < PERSONAL * PERSONAL {
                            sx -= dx / d2;
                            sy -= dy / d2;
                        }
                        // Only a handful of mates: lets big flocks split and merge.
                        if o.flock == me.flock && n < 10.0 {
                            ax += o.vx;
                            ay += o.vy;
                            cx += dx;
                            cy += dy;
                            n += 1.0;
                        }
                    }
                }
            }
            let b = &mut self.boids[i];
            if n > 0.0 {
                b.vx += (ax / n - b.vx) * 0.05 + cx / n * 0.004;
                b.vy += (ay / n - b.vy) * 0.05 + cy / n * 0.004;
            }
            let (dy, dx) = self.wander[b.flock as usize].sin_cos();
            b.vx += sx * 0.12 + dx * 0.01 + rng.rangef(-0.03, 0.03);
            b.vy += sy * 0.12 + dy * 0.01 + rng.rangef(-0.03, 0.03);
            if let Some(hk) = &self.hawk {
                let dx = wrap_delta(b.x - hk.x, ww);
                let dy = wrap_delta(b.y - hk.y, wh);
                let d2 = dx * dx + dy * dy;
                if d2 < FEAR * FEAR && d2 > 0.0 {
                    let d = d2.sqrt();
                    let k = 0.12 * (1.0 - d / FEAR) / d;
                    b.vx += dx * k * 2.0;
                    b.vy += dy * k * 2.0;
                }
            }
            let s = (b.vx * b.vx + b.vy * b.vy).sqrt().max(1e-4);
            let limit = if self.hawk.is_some() { MAX_SPEED * 1.3 } else { MAX_SPEED };
            let t = s.clamp(MIN_SPEED, limit);
            b.vx *= t / s;
            b.vy *= t / s;
            // Beat faster when flying fast (or fleeing).
            b.flap += 0.25 + t * 0.5;
        }
        for b in &mut self.boids {
            b.x = (b.x + b.vx).rem_euclid(ww);
            b.y = (b.y + b.vy).rem_euclid(wh);
        }

        // Draw: sky, fading trails, birds, hawk.
        let (w, ph) = (self.w, self.ph);
        for i in 0..w * ph {
            let (v, col) = self.trail[i];
            if v > 0.04 {
                self.px[i] = self.bg[i].lerp(col, v * 0.2);
                self.trail[i].0 = v * TRAIL_DECAY;
            } else {
                self.px[i] = self.bg[i];
            }
        }
        for i in 0..self.boids.len() {
            let b = self.boids[i];
            let col = color(&b);
            let (x, y) = (b.x as usize, b.y as usize);
            if x < w && y < ph {
                self.trail[y * w + x] = (1.0, col);
            }
            self.bird(&b, 1.0, col, 1.0);
        }
        if let Some(hk) = &self.hawk {
            let fade = (hk.life as f32 / 30.0).min(1.0);
            // Heading defaults to the right until the hawk picks up speed.
            let (vx, vy) = if hk.vx * hk.vx + hk.vy * hk.vy < 1e-4 { (1.0, 0.0) } else { (hk.vx, hk.vy) };
            let b = Boid { x: hk.x, y: hk.y, vx, vy, flap: hk.flap, flock: 0 };
            self.bird(&b, 5.0, HAWK, fade);
        }
        c.blit_pixels(&self.px);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    let ph = h * 2;
    let (ww, wh) = (w as f32, ph as f32);
    let gw = ((ww / VIEW).ceil() as usize).max(1);
    let gh = ((wh / VIEW).ceil() as usize).max(1);
    let n = (w * h / 40).clamp(1, 180);
    let flocks = rng.range(2, 4) as u8;
    let boids = (0..n)
        .map(|i| {
            let a = rng.f32() * TAU;
            Boid {
                x: rng.rangef(0.0, ww),
                y: rng.rangef(0.0, wh),
                vx: a.cos() * 0.5,
                vy: a.sin() * 0.5,
                flap: rng.f32() * TAU,
                flock: (i % flocks as usize) as u8,
            }
        })
        .collect();
    let mut bg: Vec<Rgb> = (0..w * ph).map(|i| SKY_TOP.lerp(SKY_LOW, (i / w.max(1)) as f32 / ph.max(2) as f32)).collect();
    // Stars thin out towards the hazy horizon.
    for _ in 0..w * ph / 90 {
        let y = (rng.f32() * rng.f32() * ph as f32) as usize;
        let i = y * w + rng.below(w);
        if i < bg.len() {
            bg[i] = bg[i].lerp(STAR, rng.rangef(0.15, 0.5) * (1.0 - y as f32 / ph as f32));
        }
    }
    Box::new(Boids {
        w,
        ph,
        ww,
        wh,
        boids,
        hawk: None,
        hawk_timer: rng.range(300, 700),
        gw,
        gh,
        head: vec![-1; gw * gh],
        next: vec![-1; n],
        trail: vec![(0.0, Rgb::BLACK); w * ph],
        wander: (0..flocks).map(|_| rng.f32() * TAU).collect(),
        px: bg.clone(),
        bg,
    })
}
