//! A tilted spiral galaxy. Old disk stars orbit with differential rotation;
//! young blue stars are born on logarithmic arms that turn at a slower pattern
//! speed, drift off them and fade, so the arms persist without winding up.
//!
//! Drawn in half-block pixels over a diffuse glow: a warm core and bulge, a
//! faint disk and softly lit arms that turn with the pattern, with dark dust
//! lanes along their inner edges dimming everything behind them.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;
use std::f32::consts::{PI, TAU};

pub const BG: Rgb = Rgb(1, 1, 6);
const PATTERN_SPEED: f32 = 0.0022;
/// Dust lanes sit this far upstream of where the arms give birth to stars
/// (in radians of arm phase).
const LANE: f32 = 0.55;
const CORE: [f32; 3] = [1.0, 0.88, 0.68];
const DISK: [f32; 3] = [1.0, 0.82, 0.62];
const ARM: [f32; 3] = [0.5, 0.62, 1.0];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Bulge,
    Disk,
    Arm,
}

struct Star {
    r: f32,
    a: f32,
    life: f32,
    max: f32,
    color: [f32; 3],
    bright: f32,
    kind: Kind,
}

struct Twinkle {
    i: usize,
    phase: f32,
    speed: f32,
}

/// Static geometry of one pixel of the diffuse glow. The arm pattern turns
/// rigidly, so only a rotation is left to apply each frame.
struct Glow {
    i: usize,
    /// cos/sin of the arm phase at this pixel, for the lit arms and the lanes.
    arm: (f32, f32),
    lane: (f32, f32),
    disk: f32,
    lit: f32,
    dust: f32,
    core: f32,
}

struct Galaxy {
    w: usize,
    ph: usize,
    stars: Vec<Star>,
    twinkles: Vec<Twinkle>,
    glow: Vec<Glow>,
    acc: Vec<[f32; 3]>,
    px: Vec<Rgb>,
    t: f32,
    arms: usize,
    pitch: f32,
    tilt: f32,
    pa: (f32, f32),
    radius: f32,
}

fn gauss(rng: &mut Rng) -> f32 {
    (rng.f32() + rng.f32() + rng.f32() - 1.5) * 1.4
}

fn omega(r: f32) -> f32 {
    0.0024 / (r + 0.2)
}

fn rgb(c: Rgb) -> [f32; 3] {
    [c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0]
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Galaxy {
    fn arm_star(&self, rng: &mut Rng) -> Star {
        let r = 0.12 + rng.f32().powf(0.8) * 0.9;
        let arm = rng.below(self.arms) as f32 * TAU / self.arms as f32;
        let pink = rng.chance(0.05);
        let spread = if pink { 0.04 } else { 0.14 };
        let a = arm + (r / 0.12).ln() / self.pitch + PATTERN_SPEED * self.t + gauss(rng) * spread / r;
        let color = if pink {
            Rgb(255, 110, 180)
        } else if rng.chance(0.15) {
            Rgb(175, 150, 255)
        } else {
            Rgb(160, 195, 255).lerp(Rgb::WHITE, rng.f32() * 0.5)
        };
        let max = rng.rangef(150.0, 420.0);
        Star {
            r,
            a,
            life: max * rng.f32(),
            max,
            color: rgb(color),
            bright: if pink { 1.3 } else { rng.rangef(0.4, 1.0) },
            kind: Kind::Arm,
        }
    }

    /// Screen position (in pixels) of a point on the disk.
    fn project(&self, r: f32, a: f32) -> (f32, f32) {
        let (sp, cp) = self.pa;
        let (sa, ca) = a.sin_cos();
        let (x, y) = (r * ca, r * sa * self.tilt);
        let (px, py) = (x * cp - y * sp, x * sp + y * cp);
        (self.w as f32 * 0.5 + px * self.radius, self.ph as f32 * 0.5 + py * self.radius)
    }

    fn add(&mut self, x: i32, y: i32, c: [f32; 3], b: f32) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.ph {
            let p = &mut self.acc[y as usize * self.w + x as usize];
            for k in 0..3 {
                p[k] += c[k] * b;
            }
        }
    }

    /// Precompute the diffuse glow by inverting the projection per pixel.
    fn build_glow(&mut self) {
        let (sp, cp) = self.pa;
        let (cx, cy) = (self.w as f32 * 0.5, self.ph as f32 * 0.5);
        let n = self.arms as f32;
        // Arm stars drift ahead of their birth line before they peak, so the
        // lit arms trail the lanes by that drift.
        let life = 140.0;
        for y in 0..self.ph {
            for x in 0..self.w {
                let dx = (x as f32 + 0.5 - cx) / self.radius;
                let dy = (y as f32 + 0.5 - cy) / self.radius;
                let (u, v) = (dx * cp + dy * sp, (-dx * sp + dy * cp) / self.tilt);
                let r = (u * u + v * v).sqrt();
                let d2 = dx * dx + dy * dy;
                let core = 1.6 * (-d2 / 0.006).exp() + 0.22 * (-d2 / 0.05).exp();
                let edge = 1.0 - smoothstep(0.75, 1.15, r);
                if edge <= 0.0 && core < 0.004 {
                    continue;
                }
                let spiral = v.atan2(u) - (r.max(0.01) / 0.12).ln() / self.pitch;
                let drift = (omega(r) - PATTERN_SPEED).max(0.0) * life;
                let ga = n * (spiral - drift);
                let la = n * spiral + LANE;
                let inner = smoothstep(0.06, 0.2, r);
                self.glow.push(Glow {
                    i: y * self.w + x,
                    arm: (ga.cos(), ga.sin()),
                    lane: (la.cos(), la.sin()),
                    disk: 0.16 * (-r / 0.3).exp() * edge,
                    lit: 0.55 * (-r / 0.55).exp() * edge * inner,
                    dust: 0.9 * inner * (1.0 - smoothstep(0.7, 1.05, r)),
                    core,
                });
            }
        }
    }
}

impl Animation for Galaxy {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        self.t += 1.0;
        self.acc.fill([0.0; 3]);

        for i in 0..self.stars.len() {
            let s = &mut self.stars[i];
            s.a += omega(s.r);
            let mut b = s.bright;
            if s.kind == Kind::Arm {
                s.life -= 1.0;
                if s.life <= 0.0 {
                    let mut fresh = self.arm_star(rng);
                    fresh.life = fresh.max;
                    self.stars[i] = fresh;
                    continue;
                }
                b *= (s.life / s.max * PI).sin();
            }
            let s = &self.stars[i];
            let (color, big) = (s.color, s.bright > 1.2);
            let (sx, sy) = self.project(s.r, s.a);
            let (x, y) = (sx.floor() as i32, sy.floor() as i32);
            self.add(x, y, color, b);
            // Star-forming knots glow a little wider.
            if big {
                for (ox, oy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    self.add(x + ox, y + oy, color, b * 0.15);
                }
            }
        }

        // Diffuse light, turned to the current pattern angle, and dust.
        let (sb, cb) = (self.arms as f32 * PATTERN_SPEED * self.t).sin_cos();
        for g in &self.glow {
            let arm = 0.5 + 0.5 * (g.arm.0 * cb + g.arm.1 * sb);
            let arm = arm * arm;
            let lane = 0.5 + 0.5 * (g.lane.0 * cb + g.lane.1 * sb);
            let lane = lane * lane;
            let lane = lane * lane;
            let dim = 1.0 - g.dust * lane;
            let p = &mut self.acc[g.i];
            for k in 0..3 {
                p[k] = (p[k] + DISK[k] * g.disk + ARM[k] * g.lit * arm) * dim + CORE[k] * g.core;
            }
        }

        for (p, a) in self.px.iter_mut().zip(&self.acc) {
            let m = a[0].max(a[1]).max(a[2]);
            // Compress highlights, keeping the hue.
            let k = 255.0 * 1.3 / (1.0 + 1.3 * m);
            // Snap faint light to the exact background so it stays see-through.
            *p = if m * k < 3.0 {
                BG
            } else {
                let v = |x: f32, b: u8| (b as f32 + x * k).min(255.0) as u8;
                Rgb(v(a[0], BG.0), v(a[1], BG.1), v(a[2], BG.2))
            };
        }

        for tw in &self.twinkles {
            if self.px[tw.i] == BG {
                let v = 0.5 + 0.5 * (self.t * tw.speed + tw.phase).sin();
                self.px[tw.i] = BG.lerp(Rgb(200, 210, 255), 0.1 + 0.6 * v * v);
            }
        }
        c.blit_pixels(&self.px);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    let ph = 2 * h;
    let tilt = rng.rangef(0.45, 0.7);
    let pa = rng.rangef(-0.35, 0.35);
    // Fit the rotated, tilted disk (radius ~1.05) inside the screen.
    let (ps, pc) = (pa.sin().abs(), pa.cos().abs());
    let rx = w as f32 * 0.47 / (1.05 * (pc + tilt * ps));
    let ry = ph as f32 * 0.47 / (1.05 * (ps + tilt * pc));
    let radius = rx.min(ry).max(1.0);
    let mut g = Galaxy {
        w,
        ph,
        stars: Vec::new(),
        twinkles: Vec::new(),
        glow: Vec::new(),
        acc: vec![[0.0; 3]; w * ph],
        px: vec![BG; w * ph],
        t: 0.0,
        arms: if rng.chance(0.7) { 2 } else { 3 },
        pitch: rng.rangef(12.0, 18.0).to_radians().tan(),
        tilt,
        pa: pa.sin_cos(),
        radius,
    };
    let area = (radius * radius * tilt * 0.5) as usize;
    let n = (area * 7).clamp(500, 7500);
    for i in 0..n {
        let star = match i % 10 {
            0 | 1 => Star {
                r: gauss(rng).abs() * 0.1,
                a: rng.f32() * TAU,
                life: 0.0,
                max: 1.0,
                color: rgb(Rgb(255, 225, 180)),
                bright: rng.rangef(0.3, 0.7),
                kind: Kind::Bulge,
            },
            2..=4 => {
                // Mostly sun-like, with the odd red giant.
                let color = if rng.chance(0.06) {
                    Rgb(255, 150, 90)
                } else {
                    Rgb(255, 215, 170).lerp(Rgb(200, 200, 230), rng.f32())
                };
                Star {
                    r: (-rng.f32().max(1e-4).ln() * 0.33).min(1.1),
                    a: rng.f32() * TAU,
                    life: 0.0,
                    max: 1.0,
                    color: rgb(color),
                    bright: rng.rangef(0.15, 0.4),
                    kind: Kind::Disk,
                }
            }
            _ => g.arm_star(rng),
        };
        g.stars.push(star);
    }
    for _ in 0..(w * ph / 140).max(1) {
        g.twinkles.push(Twinkle {
            i: rng.below(w * ph),
            phase: rng.f32() * TAU,
            speed: rng.rangef(0.02, 0.09),
        });
    }
    g.build_glow();
    Box::new(g)
}
