//! Fireworks over a sleeping city: rockets climb, burst into several shell
//! patterns and their sparks fall under gravity and drag.
//!
//! Drawn in half-block pixels, so physics runs directly in square pixel units.
//! Sparks are hot pixels trailing short anti-aliased streaks, fresh ones with
//! a soft halo, and every burst lights up the sky (and the rooftops) around it
//! for a moment.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;
use std::f32::consts::TAU;

const SKY_TOP: Rgb = Rgb(1, 1, 8);
const SKY_LOW: Rgb = Rgb(12, 8, 28);
const BUILDING: Rgb = Rgb(5, 5, 11);
const WINDOWS: [Rgb; 3] = [Rgb(70, 58, 26), Rgb(95, 78, 34), Rgb(40, 52, 72)];
const BEACON: Rgb = Rgb(200, 30, 20);
const EMBER: Rgb = Rgb(110, 25, 10);
const GOLD: Rgb = Rgb(255, 190, 90);
const ROCKET_G: f32 = 0.03;
const SPARK_G: f32 = 0.011;
const MAX_PARTICLES: usize = 7000;

const TRAIL: u8 = 1;
const CRACKLE: u8 = 2;
const STROBE: u8 = 4;
const ROCKET: u8 = 8;

#[derive(Clone, Copy)]
enum Shell {
    Sphere,
    Ring,
    Willow,
    Crackle,
    Multi,
    Palm,
}

#[derive(Clone, Copy)]
struct P {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max: f32,
    color: Rgb,
    drag: f32,
    grav: f32,
    flags: u8,
    shell: Shell,
}

/// The fading glow a burst casts on the sky around it.
struct Flash {
    x: f32,
    y: f32,
    radius: f32,
    color: [f32; 3],
    power: f32,
}

struct Fireworks {
    w: usize,
    ph: usize,
    ps: Vec<P>,
    flashes: Vec<Flash>,
    /// Sky gradient with its faint stars; never changes.
    sky: Vec<Rgb>,
    /// Top pixel row of the skyline in each column.
    skyline: Vec<i32>,
    shade: Vec<Rgb>,
    /// Window pixels: index, color and whether the light is on.
    windows: Vec<(usize, Rgb, bool)>,
    /// Blinking lights on top of antennas.
    beacons: Vec<(usize, u32)>,
    light: Vec<[f32; 3]>,
    glow: Vec<[f32; 3]>,
    px: Vec<Rgb>,
    frame: u32,
    timer: i32,
    finale: i32,
    until_finale: i32,
}

fn shell_color(rng: &mut Rng) -> Rgb {
    match rng.below(8) {
        0 => GOLD,
        1 => Rgb(235, 235, 255),
        _ => Rgb::hsv(rng.f32(), rng.rangef(0.55, 0.9), 1.0),
    }
}

fn rgb(c: Rgb) -> [f32; 3] {
    [c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0]
}

impl Fireworks {
    fn launch(&mut self, rng: &mut Rng) {
        let (w, hh) = (self.w as f32, self.ph as f32);
        let x = rng.rangef(w * 0.1, w * 0.9);
        let apex = rng.rangef(0.12, 0.5) * hh;
        let vy = -(2.0 * ROCKET_G * (hh - apex).max(1.0)).sqrt();
        let shell = match rng.below(6) {
            0 => Shell::Sphere,
            1 => Shell::Ring,
            2 => Shell::Willow,
            3 => Shell::Crackle,
            4 => Shell::Multi,
            _ => Shell::Palm,
        };
        self.ps.push(P {
            x,
            y: hh,
            vx: rng.rangef(-0.15, 0.15),
            vy,
            life: 1000.0,
            max: 1000.0,
            color: shell_color(rng),
            drag: 1.0,
            grav: ROCKET_G,
            flags: ROCKET,
            shell,
        });
    }

    fn burst(&mut self, r: &P, rng: &mut Rng) {
        let size = (self.w as f32).min(self.ph as f32);
        let radius = size * rng.rangef(0.18, 0.33);
        let base = (radius * 4.0).clamp(16.0, 220.0) as usize;
        self.flashes.push(Flash { x: r.x, y: r.y, radius: radius * 1.1 + 2.0, color: rgb(r.color), power: 0.2 });
        let (x, y) = (r.x, r.y);
        let spark = |ps: &mut Vec<P>, vx: f32, vy: f32, life: f32, color: Rgb, drag: f32, flags: u8| {
            if ps.len() < MAX_PARTICLES {
                ps.push(P { x, y, vx, vy, life, max: life, color, drag, grav: SPARK_G, flags, shell: Shell::Sphere });
            }
        };
        match r.shell {
            Shell::Sphere | Shell::Multi | Shell::Crackle => {
                let drag = 0.95;
                let v0 = radius * (1.0 - drag);
                let flags = if matches!(r.shell, Shell::Crackle) { CRACKLE | STROBE } else { 0 };
                for _ in 0..base {
                    // Project a 3D sphere: denser towards the rim.
                    let a = rng.f32() * TAU;
                    let u = rng.rangef(-1.0, 1.0);
                    let s = v0 * (1.0 - u * u).sqrt() * rng.rangef(0.9, 1.0);
                    let col = if matches!(r.shell, Shell::Multi) { shell_color(rng) } else { r.color };
                    spark(&mut self.ps, a.cos() * s, a.sin() * s, rng.rangef(40.0, 65.0), col, drag, flags);
                }
            }
            Shell::Ring => {
                let drag = 0.95;
                let v0 = radius * (1.0 - drag);
                let tilt = rng.rangef(0.25, 1.0);
                let rot = rng.f32() * TAU;
                let (sr, cr) = rot.sin_cos();
                let second = shell_color(rng);
                let n = base.max(16);
                for k in 0..n {
                    let a = k as f32 / n as f32 * TAU;
                    let (ex, ey) = (a.cos() * v0, a.sin() * v0 * tilt);
                    let (vx, vy) = (ex * cr - ey * sr, ex * sr + ey * cr);
                    spark(&mut self.ps, vx, vy, rng.rangef(45.0, 60.0), r.color, drag, 0);
                    if k % 3 == 0 {
                        spark(&mut self.ps, vx * 0.45, vy * 0.45, rng.rangef(35.0, 50.0), second, drag, 0);
                    }
                }
            }
            Shell::Willow => {
                let drag = 0.965;
                let v0 = radius * (1.0 - drag) * 0.9;
                for _ in 0..base {
                    let a = rng.f32() * TAU;
                    let s = v0 * rng.rangef(0.3, 1.0).sqrt();
                    spark(&mut self.ps, a.cos() * s, a.sin() * s, rng.rangef(90.0, 140.0), GOLD, drag, TRAIL);
                }
            }
            Shell::Palm => {
                let drag = 0.955;
                let v0 = radius * (1.0 - drag) * 1.1;
                let n = rng.range(7, 12) as usize;
                for k in 0..n {
                    let a = k as f32 / n as f32 * TAU + rng.rangef(-0.1, 0.1);
                    spark(&mut self.ps, a.cos() * v0, a.sin() * v0 - 0.2, rng.rangef(60.0, 80.0), r.color, drag, TRAIL);
                }
            }
        }
    }

    fn build_city(&mut self, rng: &mut Rng) {
        let (w, ph) = (self.w, self.ph);
        self.sky = (0..w * ph)
            .map(|i| SKY_TOP.lerp(SKY_LOW, (i / w.max(1)) as f32 / ph.max(2) as f32))
            .collect();
        for _ in 0..w * ph / 150 {
            let i = rng.below(w * ph * 6 / 10);
            self.sky[i] = self.sky[i].lerp(Rgb(190, 200, 240), rng.rangef(0.05, 0.2));
        }
        self.skyline = vec![ph as i32; w];
        self.shade = vec![BUILDING; w];
        let max_h = (ph / 6) as i32;
        if max_h < 2 {
            return;
        }
        let mut x = 0;
        while x < w {
            let bw = rng.range(3, 9) as usize;
            let bh = rng.range(2, max_h + 1);
            let top = ph as i32 - bh;
            let shade = BUILDING.scale(rng.rangef(0.7, 1.4));
            let x1 = (x + bw).min(w);
            for xx in x..x1 {
                self.skyline[xx] = top;
                self.shade[xx] = shade;
            }
            // Windows on a grid, leaving a margin around the edges.
            let col = *rng.pick(&WINDOWS);
            for yy in (top + 2..ph as i32).step_by(2) {
                for xx in (x + 1..x1.saturating_sub(1)).step_by(2) {
                    self.windows.push((yy as usize * w + xx, col, rng.chance(0.3)));
                }
            }
            // Tall buildings get an antenna with a blinking light.
            if bh > max_h * 3 / 4 && bw >= 5 && rng.chance(0.35) {
                let ax = x + bw / 2;
                let tall = rng.range(2, 5);
                if ax < w && top - tall > 0 {
                    self.skyline[ax] = top - tall;
                    self.shade[ax] = shade;
                    self.beacons.push((((top - tall) as usize) * w + ax, rng.below(90) as u32));
                }
            }
            x += bw + rng.below(2);
        }
    }

    /// Add light to a single pixel.
    #[inline]
    fn plot(&mut self, x: i32, y: i32, c: [f32; 3], a: f32) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.ph {
            let p = &mut self.light[y as usize * self.w + x as usize];
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

    /// A hot pixel with an anti-aliased streak behind it, `len` frames of
    /// motion long, fading towards the tail.
    fn streak(&mut self, p: &P, c: [f32; 3], a: f32, len: f32) {
        let (dx, dy) = (p.vx * len, p.vy * len);
        let n = dx.abs().max(dy.abs()).min(16.0).ceil() as i32;
        for j in 1..n {
            let u = j as f32 / n as f32;
            self.splat(p.x - dx * u, p.y - dy * u, c, a * 0.4 * (1.0 - u));
        }
        self.plot(p.x.floor() as i32, p.y.floor() as i32, c, a);
    }

    fn draw_particles(&mut self, rng: &mut Rng) {
        self.light.fill([0.0; 3]);
        for i in 0..self.ps.len() {
            let p = self.ps[i];
            let r = (p.life / p.max).clamp(0.0, 1.0);
            if p.flags & STROBE != 0 && r < 0.6 && rng.chance(0.45) {
                continue;
            }
            if p.flags & ROCKET != 0 {
                self.streak(&p, rgb(Rgb(255, 220, 160)), 1.2, 3.0);
                continue;
            }
            let mut col = p.color.lerp(EMBER, (1.0 - r) * 1.1).scale(0.35 + 0.65 * r);
            // Short-lived bits (trail embers, rocket sparks, crackles) are
            // dimmer and never white-hot; real stars flash white, then bloom.
            let star = p.max > 20.0;
            if !star {
                self.streak(&p, rgb(col), 0.45, 2.0);
                continue;
            }
            if r > 0.94 {
                col = col.lerp(Rgb::WHITE, (r - 0.94) * 10.0);
            }
            let c = rgb(col);
            self.streak(&p, c, 1.0, 6.0);
            if r > 0.6 && r < 0.95 {
                let (x, y) = (p.x.floor() as i32, p.y.floor() as i32);
                let b = (r - 0.6) * 0.6;
                for (ox, oy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    self.plot(x + ox, y + oy, c, b);
                }
            }
        }
    }

    fn draw_glow(&mut self) {
        self.glow.fill([0.0; 3]);
        let (w, ph) = (self.w as i32, self.ph as i32);
        for f in &self.flashes {
            let r = f.radius;
            let (x0, x1) = (((f.x - r) as i32).max(0), ((f.x + r) as i32 + 1).min(w));
            let (y0, y1) = (((f.y - r) as i32).max(0), ((f.y + r) as i32 + 1).min(ph));
            for y in y0..y1 {
                for x in x0..x1 {
                    let (dx, dy) = ((x as f32 + 0.5 - f.x) / r, (y as f32 + 0.5 - f.y) / r);
                    let d = dx * dx + dy * dy;
                    if d >= 1.0 {
                        continue;
                    }
                    let g = f.power * (1.0 - d) * (1.0 - d);
                    let p = &mut self.glow[(y * w + x) as usize];
                    for k in 0..3 {
                        p[k] += f.color[k] * g;
                    }
                }
            }
        }
    }
}

impl Animation for Fireworks {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        let (w, ph) = (self.w, self.ph);
        self.frame = self.frame.wrapping_add(1);

        // Launch scheduling: calm stretches, then an occasional finale.
        self.until_finale -= 1;
        if self.until_finale <= 0 {
            self.finale = rng.range(90, 150);
            self.until_finale = rng.range(900, 1600);
        }
        self.timer -= 1;
        if self.timer <= 0 {
            if self.finale > 0 {
                for _ in 0..rng.range(1, 3) {
                    self.launch(rng);
                }
                self.timer = rng.range(3, 7);
            } else {
                self.launch(rng);
                self.timer = rng.range(12, 55);
            }
        }
        if self.finale > 0 {
            self.finale -= 1;
            if self.finale == 0 {
                self.timer = 70;
            }
        }

        // Simulate.
        let mut i = 0;
        let mut new: Vec<P> = Vec::new();
        while i < self.ps.len() {
            let p = &mut self.ps[i];
            p.vx *= p.drag;
            p.vy = p.vy * p.drag + p.grav;
            p.x += p.vx;
            p.y += p.vy;
            p.life -= 1.0;
            let p = *p;
            let mut dead = p.life <= 0.0 || p.y > ph as f32 + 2.0;
            if p.flags & ROCKET != 0 {
                if rng.chance(0.8) {
                    new.push(P {
                        x: p.x + rng.rangef(-0.3, 0.3),
                        y: p.y + 1.0,
                        vx: rng.rangef(-0.08, 0.08),
                        vy: rng.rangef(0.0, 0.15),
                        life: rng.rangef(6.0, 14.0),
                        max: 14.0,
                        color: Rgb(255, 170, 80),
                        drag: 0.9,
                        grav: 0.005,
                        flags: 0,
                        shell: Shell::Sphere,
                    });
                }
                if p.vy > -0.25 {
                    self.burst(&p, rng);
                    dead = true;
                }
            } else if p.flags & TRAIL != 0 && rng.chance(0.5) {
                new.push(P { vx: 0.0, vy: 0.02, life: 18.0, max: 18.0, drag: 0.9, grav: 0.002, flags: 0, ..p });
            }
            if dead && p.flags & CRACKLE != 0 {
                for _ in 0..2 {
                    new.push(P {
                        vx: rng.rangef(-0.25, 0.25),
                        vy: rng.rangef(-0.25, 0.25),
                        life: rng.rangef(4.0, 9.0),
                        max: 9.0,
                        color: Rgb::WHITE,
                        drag: 0.85,
                        flags: 0,
                        ..p
                    });
                }
            }
            if dead {
                self.ps.swap_remove(i);
            } else {
                i += 1;
            }
        }
        let room = MAX_PARTICLES.saturating_sub(self.ps.len());
        self.ps.extend(new.into_iter().take(room));

        for f in &mut self.flashes {
            f.power *= 0.82;
        }
        self.flashes.retain(|f| f.power > 0.02);
        self.draw_glow();
        self.draw_particles(rng);

        // Occasionally someone turns a light on or off.
        if !self.windows.is_empty() && rng.chance(0.05) {
            let k = rng.below(self.windows.len());
            self.windows[k].2 = !self.windows[k].2;
        }

        let to8 = |x: f32| (x * 255.0) as u8;
        for y in 0..ph {
            for x in 0..w {
                let i = y * w + x;
                let g = self.glow[i];
                self.px[i] = if (y as i32) < self.skyline[x] {
                    // Hot light spills over into white where a channel clips.
                    let l = self.light[i];
                    let over = (l[0].max(l[1]).max(l[2]) - 1.0).max(0.0) * 0.3;
                    let v = |k: usize| to8((l[k] + over).min(1.0) + g[k]);
                    self.sky[i].add(Rgb(v(0), v(1), v(2)))
                } else {
                    // Buildings hide the sparks but catch a little of the glow.
                    self.shade[x].add(Rgb(to8(g[0] * 0.3), to8(g[1] * 0.3), to8(g[2] * 0.3)))
                };
            }
        }
        for &(i, col, on) in &self.windows {
            if on {
                self.px[i] = col;
            }
        }
        for &(i, phase) in &self.beacons {
            if (self.frame + phase) % 90 < 45 {
                self.px[i] = BEACON;
            }
        }
        c.blit_pixels(&self.px);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    let ph = 2 * h;
    let mut f = Fireworks {
        w,
        ph,
        ps: Vec::with_capacity(2048),
        flashes: Vec::new(),
        sky: Vec::new(),
        skyline: Vec::new(),
        shade: Vec::new(),
        windows: Vec::new(),
        beacons: Vec::new(),
        light: vec![[0.0; 3]; w * ph],
        glow: vec![[0.0; 3]; w * ph],
        px: vec![SKY_TOP; w * ph],
        frame: 0,
        timer: 5,
        finale: 0,
        until_finale: rng.range(500, 1100),
    };
    f.build_city(rng);
    Box::new(f)
}
