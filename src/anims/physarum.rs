//! Slime mould: thousands of agents grow a living transport network.
//!
//! Each agent drops a little chemical where it stands, sniffs three points
//! ahead (left, centre, right) and turns towards the strongest; the chemical
//! spreads and evaporates every step. Only one agent fits in a pixel: one that
//! bumps into another picks a new heading instead, which is what stops the
//! population collapsing into a single blob. Trails recruit the agents
//! that stumble onto them and fade when abandoned, so the population pulls
//! itself into veins that merge, compete and reroute. Every so often the
//! network fades and regrows with other settings and colors.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;
use std::f32::consts::{PI, TAU};

/// Frames each network lives before fading out and regrowing.
const EPOCH: u32 = 2400;
const FADE: u32 = 90;

/// (sensor angle, turn angle, sensor distance factor, decay): each grows a
/// different kind of network, from fine lace to thick cords.
const PRESETS: [(f32, f32, f32, f32); 4] = [
    (PI / 8.0, PI / 4.0, 1.0, 0.90),
    (PI / 4.0, PI / 4.0, 1.2, 0.92),
    (PI / 6.0, PI / 5.0, 0.8, 0.88),
    (PI / 4.0, PI / 6.0, 1.4, 0.93),
];

/// Color ramps from faint trail to dense vein; the background stays black.
const PALETTES: [[Rgb; 3]; 5] = [
    [Rgb(70, 40, 0), Rgb(240, 170, 30), Rgb(255, 250, 210)],
    [Rgb(0, 50, 70), Rgb(20, 200, 230), Rgb(220, 255, 255)],
    [Rgb(60, 0, 60), Rgb(230, 60, 200), Rgb(255, 220, 250)],
    [Rgb(10, 60, 10), Rgb(90, 230, 80), Rgb(230, 255, 200)],
    [Rgb(40, 20, 90), Rgb(120, 110, 255), Rgb(240, 230, 255)],
];

struct Agent {
    x: f32,
    y: f32,
    a: f32,
}

struct Physarum {
    w: usize,
    ph: usize,
    field: Vec<f32>,
    tmp: Vec<f32>,
    agents: Vec<Agent>,
    taken: Vec<bool>,
    px: Vec<Rgb>,
    frame: u32,
    preset: usize,
    palette: usize,
    sensor: f32,
}

impl Physarum {
    fn new(w: usize, h: usize, rng: &mut Rng) -> Physarum {
        let ph = 2 * h;
        let mut p = Physarum {
            w,
            ph,
            field: vec![0.0; w * ph],
            tmp: vec![0.0; w * ph],
            agents: Vec::new(),
            taken: vec![false; w * ph],
            px: vec![Rgb::BLACK; w * ph],
            frame: 0,
            preset: 0,
            palette: 0,
            sensor: 0.0,
        };
        p.reseed(rng);
        p
    }

    fn reseed(&mut self, rng: &mut Rng) {
        let (w, ph) = (self.w as f32, self.ph as f32);
        self.preset = rng.below(PRESETS.len());
        self.palette = rng.below(PALETTES.len());
        // Sensors reach a few pixels: the veins end up a few times this apart,
        // so the network stays fine even on a small screen.
        self.sensor = (ph.min(w) / 16.0).clamp(2.5, 7.0) * PRESETS[self.preset].2;
        self.field.fill(0.0);
        self.taken.fill(false);
        let n = (self.w * self.ph * 3 / 20).clamp(1, 20_000);
        let (cx, cy) = (w / 2.0, ph / 2.0);
        let radius = w.min(ph) * 0.4;
        let layout = rng.below(3);
        let spots: Vec<Agent> = (0..n)
            .map(|_| match layout {
                // A ring of agents facing its centre.
                0 => {
                    let t = rng.rangef(0.0, TAU);
                    let r = radius * rng.rangef(0.6, 1.0);
                    Agent { x: cx + t.cos() * r, y: cy + t.sin() * r, a: t + PI }
                }
                // A burst from the middle.
                1 => {
                    let t = rng.rangef(0.0, TAU);
                    let r = radius * 0.15 * rng.f32();
                    Agent { x: cx + t.cos() * r, y: cy + t.sin() * r, a: t }
                }
                // Scattered everywhere.
                _ => Agent { x: rng.rangef(0.0, w), y: rng.rangef(0.0, ph), a: rng.rangef(0.0, TAU) },
            })
            .collect();
        // Keep one agent per pixel; crowded spots move to a random free one.
        self.agents.clear();
        for mut ag in spots {
            ag.x = ag.x.rem_euclid(w);
            ag.y = ag.y.rem_euclid(ph);
            let mut i = self.cell(ag.x, ag.y);
            if self.taken[i] {
                ag.x = rng.rangef(0.0, w);
                ag.y = rng.rangef(0.0, ph);
                i = self.cell(ag.x, ag.y);
                if self.taken[i] {
                    continue;
                }
            }
            self.taken[i] = true;
            self.agents.push(ag);
        }
    }

    #[inline]
    fn cell(&self, x: f32, y: f32) -> usize {
        (y as usize).min(self.ph - 1) * self.w + (x as usize).min(self.w - 1)
    }

    #[inline]
    fn sample(&self, x: f32, y: f32) -> f32 {
        let (w, ph) = (self.w as i32, self.ph as i32);
        let xi = (x as i32).rem_euclid(w);
        let yi = (y as i32).rem_euclid(ph);
        self.field[(yi * w + xi) as usize]
    }

    fn move_agents(&mut self, rng: &mut Rng) {
        let (sa, ra, _, _) = PRESETS[self.preset];
        let so = self.sensor;
        let (w, ph) = (self.w as f32, self.ph as f32);
        for i in 0..self.agents.len() {
            let Agent { x, y, a } = self.agents[i];
            let probe = |da: f32| self.sample(x + (a + da).cos() * so, y + (a + da).sin() * so);
            let (l, f, r) = (probe(-sa), probe(0.0), probe(sa));
            // A little wobble keeps agents from locking into a single line.
            let mut a = a + rng.rangef(-0.08, 0.08);
            if f >= l && f >= r {
                // Straight on.
            } else if f < l && f < r {
                a += if rng.chance(0.5) { ra } else { -ra };
            } else if l > r {
                a -= ra;
            } else {
                a += ra;
            }
            let nx = (x + a.cos()).rem_euclid(w);
            let ny = (y + a.sin()).rem_euclid(ph);
            let (from, to) = (self.cell(x, y), self.cell(nx, ny));
            if to != from && self.taken[to] {
                // Blocked: stay put and try another heading next step.
                self.agents[i].a = rng.rangef(0.0, TAU);
                continue;
            }
            self.taken[from] = false;
            self.taken[to] = true;
            self.agents[i] = Agent { x: nx, y: ny, a };
            self.field[to] += 1.0;
        }
    }

    /// 3x3 box blur with wrap-around, then evaporation.
    fn diffuse(&mut self, decay: f32) {
        let (w, ph) = (self.w, self.ph);
        for y in 0..ph {
            let row = y * w;
            for x in 0..w {
                let l = self.field[row + (x + w - 1) % w];
                let r = self.field[row + (x + 1) % w];
                self.tmp[row + x] = l + self.field[row + x] + r;
            }
        }
        for y in 0..ph {
            let (up, down) = ((y + ph - 1) % ph * w, (y + 1) % ph * w);
            let row = y * w;
            for x in 0..w {
                let s = self.tmp[up + x] + self.tmp[row + x] + self.tmp[down + x];
                self.field[row + x] = s / 9.0 * decay;
            }
        }
    }
}

impl Animation for Physarum {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        if self.w == 0 || self.ph == 0 {
            return;
        }
        self.frame += 1;
        let phase = self.frame % EPOCH;
        if phase == 0 {
            self.reseed(rng);
        }
        let dying = phase > EPOCH - FADE;
        let decay = if dying { 0.8 } else { PRESETS[self.preset].3 };
        if !dying {
            self.move_agents(rng);
        }
        self.diffuse(decay);

        let [lo, mid, hi] = PALETTES[self.palette];
        let grow = (phase as f32 / FADE as f32).min(1.0);
        // Veins settle at a concentration of about 1 / (1 - decay), so scale
        // by that to get similar brightness whatever the preset.
        let gain = 0.35 * (1.0 - PRESETS[self.preset].3) / 0.1;
        for (p, &v) in self.px.iter_mut().zip(&self.field) {
            // Tone-map the unbounded concentration into [0, 1).
            let tone = (1.0 - (-v * gain).exp()) * grow;
            *p = if tone < 0.03 {
                Rgb::BLACK
            } else if tone < 0.5 {
                Rgb::BLACK.lerp(lo, tone * 2.0).lerp(mid, (tone * 2.0 - 0.4).max(0.0))
            } else {
                mid.lerp(hi, (tone - 0.5) * 2.0)
            };
        }
        c.blit_pixels(&self.px);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    Box::new(Physarum::new(w, h, rng))
}
