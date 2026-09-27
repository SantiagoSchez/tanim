//! Thunderstorm: drifting clouds, wind-slanted rain at several depths over a
//! dark wooded ridge, splashes and ripples in the puddles, and branching
//! lightning that lights up the whole scene.
//!
//! Everything is drawn in half-block pixels. Each drop has a depth: far drops
//! are short, dim and slow and land near the horizon, near ones are long,
//! bright motion-blurred streaks that land at the bottom of the screen.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;

const SKY_TOP: Rgb = Rgb(22, 25, 36);
const SKY_BOT: Rgb = Rgb(10, 12, 20);
const CLOUD: Rgb = Rgb(66, 70, 86);
const CLOUD_DARK: Rgb = Rgb(34, 37, 50);
const CLOUD_LIT: Rgb = Rgb(190, 190, 215);
const FLASH_SKY: Rgb = Rgb(95, 100, 135);
const RIDGE: Rgb = Rgb(7, 8, 13);
const RIDGE_LIT: Rgb = Rgb(28, 30, 42);
const GROUND: Rgb = Rgb(20, 22, 28);
const GROUND_NEAR: Rgb = Rgb(28, 30, 36);
const DROP_FAR: Rgb = Rgb(70, 82, 108);
const DROP_NEAR: Rgb = Rgb(170, 188, 220);
const BOLT: Rgb = Rgb(255, 252, 225);
const BOLT_GLOW: Rgb = Rgb(150, 150, 230);

/// Brightness of a strike over its lifetime: double flicker, then a fade.
const STRIKE: [f32; 16] = [
    1.0, 0.9, 0.2, 0.0, 0.8, 1.0, 0.75, 0.55, 0.4, 0.3, 0.22, 0.15, 0.1, 0.06, 0.03, 0.0,
];

struct Drop {
    x: f32,
    y: f32,
    z: f32,
}

/// A ring spreading on the ground where a drop hit.
struct Ripple {
    x: f32,
    y: f32,
    age: u8,
    z: f32,
}

/// A droplet bouncing up out of a splash.
struct Spray {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    floor: f32,
}

struct Bolt {
    /// Pixels of the channel with their weight (1 for the main stroke).
    px: Vec<(i32, i32, f32)>,
    x: f32,
    age: usize,
    strength: f32,
}

struct Rain {
    w: usize,
    ph: usize,
    cloud_h: usize,
    /// First pixel row of the ground plane.
    ground: usize,
    /// Top pixel row of the ridge silhouette per column.
    land: Vec<usize>,
    /// Puddle strength per ground pixel (0 = dry ground).
    puddle: Vec<f32>,
    drops: Vec<Drop>,
    ripples: Vec<Ripple>,
    spray: Vec<Spray>,
    bolt: Option<Bolt>,
    wind: f32,
    wind_target: f32,
    intensity: f32,
    intensity_target: f32,
    next_strike: u32,
    cloud_off: f32,
    seed: u32,
    t: u32,
    /// Cloud density of the pixel row being drawn.
    row: Vec<f32>,
    px: Vec<Rgb>,
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    let ph = h * 2;
    let ground = ph.saturating_sub((ph / 9).max(1));
    let seed = rng.next_u64() as u32;
    let mut r = Rain {
        w,
        ph,
        cloud_h: (ph / 4).max(1),
        ground,
        land: Vec::new(),
        puddle: vec![0.0; w * (ph - ground)],
        drops: Vec::new(),
        ripples: Vec::new(),
        spray: Vec::new(),
        bolt: None,
        wind: rng.rangef(-0.4, 0.4),
        wind_target: 0.0,
        intensity: 0.8,
        intensity_target: 0.8,
        next_strike: rng.range(60, 200) as u32,
        cloud_off: 0.0,
        seed,
        t: 0,
        row: vec![0.0; w],
        px: vec![Rgb::BLACK; w * ph],
    };
    r.build_land(rng);
    r.wind_target = r.wind;
    let target = r.target();
    for _ in 0..target {
        let mut d = r.spawn(rng);
        d.y = rng.rangef(0.0, ph as f32);
        r.drops.push(d);
    }
    Box::new(r)
}

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x27d4_eb2d) ^ (y as u32).wrapping_mul(0x1656_67b1) ^ seed;
    h = (h ^ (h >> 15)).wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    (h & 0xffff) as f32 / 65535.0
}

fn noise(x: f32, y: f32, seed: u32) -> f32 {
    let (xf, yf) = (x.floor(), y.floor());
    let (xi, yi) = (xf as i32, yf as i32);
    let (fx, fy) = (x - xf, y - yf);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let a = hash(xi, yi, seed);
    let b = hash(xi + 1, yi, seed);
    let c = hash(xi, yi + 1, seed);
    let d = hash(xi + 1, yi + 1, seed);
    let top = a + (b - a) * sx;
    let bot = c + (d - c) * sx;
    top + (bot - top) * sy
}

/// Add `k * noise(x0 + i * dx, y)` to every `out[i]`, hashing each lattice
/// cell once instead of once per pixel.
fn add_noise_row(out: &mut [f32], x0: f32, dx: f32, y: f32, seed: u32, k: f32) {
    let yf = y.floor();
    let yi = yf as i32;
    let fy = y - yf;
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let (mut cell, mut left, mut right) = (i32::MIN, 0.0, 0.0);
    for (i, o) in out.iter_mut().enumerate() {
        let x = x0 + i as f32 * dx;
        let xf = x.floor();
        if xf as i32 != cell {
            cell = xf as i32;
            let col = |xi: i32| {
                let a = hash(xi, yi, seed);
                a + (hash(xi, yi + 1, seed) - a) * sy
            };
            left = col(cell);
            right = col(cell + 1);
        }
        let fx = x - xf;
        *o += k * (left + (right - left) * fx * fx * (3.0 - 2.0 * fx));
    }
}

impl Rain {
    fn target(&self) -> usize {
        // Drops are pixel streaks now, so fewer of them fill the same air.
        (self.w as f32 * self.ph as f32 * 0.012 * self.intensity) as usize + 1
    }

    fn spawn(&self, rng: &mut Rng) -> Drop {
        let spread = self.wind.abs() * self.ph as f32 * 0.75 + 2.0;
        let x = rng.rangef(-spread, self.w as f32 + spread);
        Drop { x, y: rng.rangef(-6.0, self.cloud_h as f32), z: rng.f32() }
    }

    /// Row where a drop at depth `z` meets the ground: far ones near the
    /// horizon, near ones at the bottom edge.
    fn floor(&self, z: f32) -> f32 {
        self.ground as f32 + z * (self.ph - self.ground) as f32
    }

    /// A low rolling ridge crowned with pines, as a height map.
    fn build_land(&mut self, rng: &mut Rng) {
        let (w, ph) = (self.w, self.ph);
        let g = self.ground as f32;
        let mut top: Vec<f32> = (0..w)
            .map(|x| {
                let n = noise(x as f32 * 0.03, 0.5, self.seed ^ 5) * 0.7 + noise(x as f32 * 0.11, 1.5, self.seed ^ 9) * 0.3;
                g - ph as f32 * (0.02 + 0.07 * n)
            })
            .collect();
        // Pines grow in clumps: dense where the forest noise is high, with
        // open gaps between.
        let hills = top.clone();
        let mut x = rng.range(0, 4);
        while (x as usize) < w {
            let forest = noise(x as f32 * 0.06, 3.5, self.seed ^ 13);
            if forest > 0.35 {
                let base = hills[x as usize];
                let tall = rng.rangef(3.0, (ph as f32 * 0.1).max(3.5)) * (0.5 + forest);
                let slope = rng.rangef(1.8, 2.8);
                let half = (tall / slope) as i32;
                for dx in -half..=half {
                    let xx = x + dx;
                    if xx >= 0 && (xx as usize) < w {
                        let y = base - tall + dx.abs() as f32 * slope;
                        top[xx as usize] = top[xx as usize].min(y);
                    }
                }
            }
            x += rng.range(1, 5);
        }
        self.land = top.iter().map(|&y| (y.max(self.cloud_h as f32).round() as usize).min(self.ground)).collect();

        // Puddles: flat horizontal blobs, squashed more towards the horizon.
        let gh = ph - self.ground;
        for _ in 0..(w * gh / 60).max(1) {
            let (cx, cy) = (rng.rangef(0.0, w as f32), rng.rangef(0.0, gh as f32));
            let rx = rng.rangef(2.0, 8.0) * (0.4 + cy / gh as f32);
            let ry = rx * 0.18 + 0.4;
            for yy in 0..gh {
                for xx in 0..w {
                    let (dx, dy) = ((xx as f32 - cx) / rx, (yy as f32 - cy) / ry);
                    let d = dx * dx + dy * dy;
                    if d < 1.0 {
                        let p = &mut self.puddle[yy * w + xx];
                        *p = p.max((1.0 - d).sqrt());
                    }
                }
            }
        }
    }

    fn strike(&mut self, rng: &mut Rng) {
        let (w, cloud) = (self.w as i32, self.cloud_h as i32);
        let x0 = rng.range(w / 8, w - w / 8);
        let mut px = Vec::new();
        // (x, y, overall lean, remaining length, weight)
        let mut stack = vec![(x0 as f32, cloud - 1, rng.rangef(-0.3, 0.3), i32::MAX, 1.0f32)];
        while let Some((mut x, mut y, lean, mut len, weight)) = stack.pop() {
            let (mut dir, mut seg) = (lean, 0);
            loop {
                let xi = x.round() as i32;
                let floor = self.land.get(xi as usize).map_or(self.ground, |&l| l) as i32;
                if len <= 0 || y >= floor || y >= self.ph as i32 - 1 {
                    break;
                }
                // Zigzag: short straight segments, each veering the other way.
                if seg == 0 {
                    let side = if dir > lean { -1.0 } else { 1.0 };
                    dir = lean + side * rng.rangef(0.2, 1.3);
                    seg = rng.range(1, 5);
                }
                seg -= 1;
                let nx = x + dir;
                y += 1;
                len -= 1;
                // Fill sideways steps so the channel stays connected.
                let (a, b) = (xi, nx.round() as i32);
                let step = if b >= a { 1 } else { -1 };
                let mut k = a;
                loop {
                    if k != a || b == a {
                        px.push((k, y, weight));
                    }
                    if k == b {
                        break;
                    }
                    k += step;
                }
                x = nx;
                if weight > 0.3 && stack.len() < 8 && rng.chance(0.05) {
                    let side = if rng.chance(0.5) { 1.0 } else { -1.0 };
                    let len = rng.range(4, (self.ph as i32 / 3).max(5));
                    stack.push((x, y, lean + side * rng.rangef(0.4, 0.9), len, weight * 0.55));
                }
            }
        }
        self.bolt = Some(Bolt { px, x: x0 as f32, age: 0, strength: 1.0 });
    }

    /// Blend `col` into pixel `(x, y)` if it is on screen.
    #[inline]
    fn blend(&mut self, x: i32, y: i32, col: Rgb, a: f32) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.ph {
            let p = &mut self.px[y as usize * self.w + x as usize];
            *p = p.lerp(col, a);
        }
    }

    /// Anti-aliased streak from `(x, y)` (the head) back up to `y - len`,
    /// leaning `slope` pixels sideways per row and fading towards the tail.
    /// Only `smooth` streaks are anti-aliased sideways: it doubles the pixels
    /// that change, so it is kept for the few big drops up close.
    fn streak(&mut self, x: f32, y: f32, len: f32, slope: f32, col: Rgb, a: f32, smooth: bool) {
        // A crisp streak leaning less than a pixel is drawn straight: a lone
        // kink halfway down looks broken.
        let (x, slope) = if !smooth && (slope * len).abs() < 1.0 { (x - slope * len * 0.5, 0.0) } else { (x, slope) };
        let (top, bot) = (y - len, y);
        let mut row = top.floor();
        while row <= bot {
            // Vertical coverage of this pixel row by the streak.
            let cover = (bot.min(row + 1.0) - top.max(row)).clamp(0.0, 1.0);
            if cover > 0.0 {
                let t = (row + 0.5 - top) / len.max(1e-3);
                let cx = x - (bot - row - 0.5) * slope - if smooth { 0.5 } else { 0.0 };
                let xf = cx.floor();
                let fr = cx - xf;
                let k = a * cover * (0.35 + 0.65 * t.clamp(0.0, 1.0));
                if smooth {
                    self.blend(xf as i32, row as i32, col, k * (1.0 - fr));
                    self.blend(xf as i32 + 1, row as i32, col, k * fr);
                } else {
                    self.blend(cx.round() as i32, row as i32, col, k);
                }
            }
            row += 1.0;
        }
    }
}

impl Animation for Rain {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        let (w, ph) = (self.w, self.ph);
        if w == 0 || ph == 0 {
            return;
        }
        self.t += 1;

        // Weather drifts slowly.
        if self.t % 120 == 0 && rng.chance(0.5) {
            self.wind_target = if rng.chance(0.2) { rng.rangef(-0.9, 0.9) } else { rng.rangef(-0.35, 0.35) };
        }
        if self.t % 200 == 0 {
            self.intensity_target = rng.rangef(0.35, 1.2);
        }
        self.wind += (self.wind_target - self.wind) * 0.01;
        self.intensity += (self.intensity_target - self.intensity) * 0.01;
        self.cloud_off += 0.01 + self.wind * 0.03;

        // Lightning.
        if self.next_strike == 0 {
            if rng.chance(0.7) && ph >= 8 {
                self.strike(rng);
            } else {
                let x = rng.rangef(0.0, w as f32);
                self.bolt = Some(Bolt { px: Vec::new(), x, age: 0, strength: rng.rangef(0.25, 0.5) });
            }
            self.next_strike = rng.range(90, 360) as u32;
        } else {
            self.next_strike -= 1;
        }
        let (mut flash, mut flash_x) = (0.0, 0.0);
        if let Some(b) = &mut self.bolt {
            flash = STRIKE.get(b.age).copied().unwrap_or(0.0) * b.strength;
            flash_x = b.x;
            b.age += 1;
            if b.age >= STRIKE.len() {
                self.bolt = None;
            }
        }

        // Sky and clouds, lit most strongly around the strike.
        let cloud_h = self.cloud_h as f32;
        let reach = (w as f32 * 0.3).max(1.0);
        for y in 0..self.ground {
            let sky = Rgb::gradient(&[SKY_TOP, SKY_BOT], y as f32 / ph as f32).lerp(FLASH_SKY, flash * 0.7);
            let yf = y as f32 * 0.5;
            let in_cloud = (y as f32) < cloud_h + 2.0;
            let fall = 1.0 - (y as f32 / (cloud_h + 2.0)).powi(2);
            if in_cloud {
                self.row.fill(0.0);
                add_noise_row(&mut self.row, self.cloud_off, 0.07, yf * 0.45, self.seed, 0.65);
                add_noise_row(&mut self.row, self.cloud_off * 1.7, 0.19, yf * 0.9, self.seed ^ 77, 0.35);
            }
            for x in 0..w {
                let mut p = sky;
                if in_cloud {
                    let xf = x as f32;
                    let n = self.row[x];
                    let d = ((n * 1.4 - 0.25) * fall).clamp(0.0, 1.0);
                    // Denser cloud is darker underneath and brighter on top.
                    let body = CLOUD_DARK.lerp(CLOUD, (n * 1.6 - 0.2) * (1.0 - 0.4 * y as f32 / cloud_h));
                    let near = ((xf - flash_x) / reach).powi(2);
                    let lit = flash * (0.45 + 0.55 / (1.0 + near));
                    p = p.lerp(body.lerp(CLOUD_LIT, lit), d);
                }
                self.px[y * w + x] = p;
            }
        }

        // Ridge silhouette, revealed against the sky by each flash.
        let ridge = RIDGE.lerp(RIDGE_LIT, flash);
        for x in 0..w {
            for y in self.land[x]..self.ground {
                self.px[y * w + x] = ridge;
            }
        }

        // Ground plane with puddles mirroring the sky.
        let gh = ph - self.ground;
        for yy in 0..gh {
            let y = self.ground + yy;
            let d = (yy + 1) as f32 / gh as f32;
            let g = GROUND.lerp(GROUND_NEAR, d).lerp(FLASH_SKY, flash * 0.4);
            let mirror = Rgb::gradient(&[SKY_TOP, SKY_BOT], 1.0 - d).lerp(FLASH_SKY.scale(1.4), flash);
            for x in 0..w {
                let p = self.puddle[yy * w + x];
                self.px[y * w + x] = if p > 0.0 { g.lerp(mirror, 0.3 + p * 0.5) } else { g };
            }
        }

        // Rain.
        let target = self.target();
        let mut spawn = target.saturating_sub(self.drops.len()).min(w / 2 + 4);
        let wind = self.wind;
        let far = w as f32 + wind.abs() * ph as f32 * 0.75 + 4.0;
        let mut i = 0;
        while i < self.drops.len() {
            let d = &mut self.drops[i];
            let vy = 1.8 + d.z * 2.2;
            d.x += wind * vy * 0.5;
            d.y += vy;
            let (dx, dz) = (d.x, d.z);
            let floor = self.floor(dz);
            let landed = self.drops[i].y >= floor;
            if landed || dx < -far || dx > far + w as f32 {
                if landed && dz > 0.35 && ph >= 6 {
                    self.ripples.push(Ripple { x: dx, y: floor, age: 0, z: dz });
                    if dz > 0.6 {
                        for _ in 0..rng.range(1, 4) {
                            self.spray.push(Spray {
                                x: dx,
                                y: floor - 0.5,
                                vx: rng.rangef(-0.5, 0.5) + wind * 0.3,
                                vy: rng.rangef(-1.3, -0.5) * dz,
                                floor,
                            });
                        }
                    }
                }
                if spawn > 0 || self.drops.len() <= target {
                    spawn = spawn.saturating_sub(1);
                    self.drops[i] = self.spawn(rng);
                    i += 1;
                } else {
                    self.drops.swap_remove(i);
                }
                continue;
            }
            i += 1;
        }
        for _ in 0..spawn {
            let d = self.spawn(rng);
            self.drops.push(d);
        }

        // Ripples and droplets go under the rain.
        let mut ripples = std::mem::take(&mut self.ripples);
        ripples.retain_mut(|r| {
            let rad = 0.6 + r.age as f32 * (0.35 + r.z * 0.35);
            let a = (1.0 - r.age as f32 / 9.0) * (0.2 + r.z * 0.35);
            let col = DROP_FAR.lerp(DROP_NEAR, r.z);
            // A flat ellipse: perspective squashes it into a dash.
            let n = (rad * 3.0) as i32 + 3;
            for k in 0..n {
                let ang = k as f32 / n as f32 * std::f32::consts::TAU;
                let (x, y) = (r.x + ang.cos() * rad, r.y + ang.sin() * rad * 0.3);
                self.blend(x.round() as i32, y.round() as i32, col, a);
            }
            r.age += 1;
            r.age < 9
        });
        self.ripples = ripples;
        let mut spray = std::mem::take(&mut self.spray);
        spray.retain_mut(|s| {
            s.x += s.vx;
            s.y += s.vy;
            s.vy += 0.25;
            self.blend(s.x.round() as i32, s.y.round() as i32, DROP_NEAR, 0.8);
            s.y < s.floor
        });
        self.spray = spray;

        for i in 0..self.drops.len() {
            let Drop { x, y, z } = self.drops[i];
            let col = DROP_FAR.lerp(DROP_NEAR, z).lerp(Rgb::WHITE, flash * 0.3);
            let len = 1.5 + z * 4.5;
            // Clip the head at the ground it will land on.
            let y = y.min(self.floor(z));
            self.streak(x, y, len, wind * 0.5, col, 0.3 + z * 0.6, z > 0.75);
        }

        // Bolt on top of everything: a soft halo, then the white-hot channel.
        if let Some(b) = self.bolt.take() {
            let k = STRIKE.get(b.age.saturating_sub(1)).copied().unwrap_or(0.0);
            if k > 0.0 {
                for &(x, y, weight) in &b.px {
                    for dy in -2..=2 {
                        for dx in -2..=2 {
                            let d2 = (dx * dx + dy * dy) as f32;
                            if d2 > 0.0 && d2 <= 5.0 {
                                self.blend(x + dx, y + dy, BOLT_GLOW, k * weight * 0.3 / d2);
                            }
                        }
                    }
                }
                for &(x, y, weight) in &b.px {
                    self.blend(x, y, BOLT, k * (0.45 + weight * 0.55));
                }
            }
            self.bolt = Some(b);
        }
        c.blit_pixels(&self.px);
    }
}
