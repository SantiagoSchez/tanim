//! Living neurons under a fluorescence microscope.
//!
//! Somas sprout short branching dendrites and axons that wander towards
//! neighbouring cells, forking into terminals that end in synaptic boutons.
//! Each neuron integrates the input it receives, leaks it slowly and fires
//! once it crosses a threshold: a spike races down every branch of its axon
//! and, at each synapse, flashes and releases a puff of neurotransmitter that
//! nudges the next cell towards firing (or, at inhibitory synapses, away from
//! it). Cascades of activity ripple through the network; a slow homeostatic
//! gain keeps it from falling silent or running away. A blurred second layer
//! stands in for the out-of-focus tissue behind.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;
use std::f32::consts::{PI, TAU};

pub const BG: Rgb = Rgb(2, 3, 10);
/// Frames a network lives before fading out and regrowing.
const EPOCH: u32 = 3600;
const FADE: u32 = 60;
/// Frames the network takes to grow in before it starts firing.
const GROW: u32 = 150;
const REFRACTORY: u32 = 25;

/// (neurites, excitatory spikes, inhibitory spikes) per palette.
const PALETTES: [(Rgb, Rgb, Rgb); 5] = [
    (Rgb(30, 110, 160), Rgb(120, 230, 255), Rgb(255, 120, 200)),
    (Rgb(120, 40, 140), Rgb(240, 120, 240), Rgb(120, 200, 255)),
    (Rgb(140, 90, 30), Rgb(255, 205, 90), Rgb(120, 220, 255)),
    (Rgb(30, 120, 60), Rgb(130, 255, 150), Rgb(255, 150, 90)),
    (Rgb(70, 60, 170), Rgb(160, 150, 255), Rgb(255, 210, 110)),
];

struct Neuron {
    x: f32,
    y: f32,
    r: f32,
    v: f32,
    refr: u32,
    out: Vec<usize>,
}

struct Conn {
    to: usize,
    path: Vec<(i32, i32)>,
    weight: f32,
    inhibitory: bool,
}

struct Spike {
    conn: usize,
    pos: f32,
}

struct Puff {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: u32,
    inhibitory: bool,
}

struct Neurons {
    w: usize,
    ph: usize,
    neurons: Vec<Neuron>,
    conns: Vec<Conn>,
    spikes: Vec<Spike>,
    puffs: Vec<Puff>,
    /// Static neurite brightness (with its halo) and the frame each pixel
    /// appears on while the network grows.
    structure: Vec<f32>,
    reveal: Vec<u16>,
    far: Vec<f32>,
    /// Transient light from spikes and flashes, excitatory and inhibitory.
    light: Vec<f32>,
    light2: Vec<f32>,
    bloom: Vec<f32>,
    bloom2: Vec<f32>,
    px: Vec<Rgb>,
    frame: u32,
    gain: f32,
    palette: usize,
}

/// Wander from `start` towards `target`, one pixel per step, steering with
/// some noise. Returns the points visited.
fn walk(start: (f32, f32), angle: f32, target: (f32, f32), reach: f32, rng: &mut Rng) -> Vec<(f32, f32)> {
    let (mut x, mut y) = start;
    let mut a = angle;
    let dist = ((target.0 - x).powi(2) + (target.1 - y).powi(2)).sqrt();
    let mut pts = Vec::new();
    for _ in 0..(dist * 3.0) as usize + 20 {
        let want = (target.1 - y).atan2(target.0 - x);
        let diff = (want - a + PI).rem_euclid(TAU) - PI;
        a += diff * 0.18 + rng.rangef(-0.35, 0.35);
        x += a.cos();
        y += a.sin();
        pts.push((x, y));
        if (target.0 - x).powi(2) + (target.1 - y).powi(2) <= reach * reach {
            break;
        }
    }
    pts
}

/// Turn a run of points into distinct consecutive pixels.
fn pixels(pts: &[(f32, f32)]) -> Vec<(i32, i32)> {
    let mut out: Vec<(i32, i32)> = Vec::with_capacity(pts.len());
    for &(x, y) in pts {
        let p = (x.round() as i32, y.round() as i32);
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    out
}

impl Neurons {
    fn new(w: usize, h: usize, rng: &mut Rng) -> Neurons {
        let ph = 2 * h;
        let mut n = Neurons {
            w,
            ph,
            neurons: Vec::new(),
            conns: Vec::new(),
            spikes: Vec::new(),
            puffs: Vec::new(),
            structure: vec![0.0; w * ph],
            reveal: vec![0; w * ph],
            far: vec![0.0; w * ph],
            light: vec![0.0; w * ph],
            light2: vec![0.0; w * ph],
            bloom: vec![0.0; w * ph],
            bloom2: vec![0.0; w * ph],
            px: vec![BG; w * ph],
            frame: 0,
            gain: 1.0,
            palette: 0,
        };
        n.grow(rng);
        n
    }

    #[inline]
    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.ph).then(|| y as usize * self.w + x as usize)
    }

    fn mark(&mut self, x: i32, y: i32, v: f32, at: u32) {
        if let Some(i) = self.idx(x, y) {
            if v > self.structure[i] {
                self.structure[i] = v;
            }
            let at = at.min(u16::MAX as u32) as u16;
            if self.reveal[i] == 0 || at < self.reveal[i] {
                self.reveal[i] = at.max(1);
            }
        }
    }

    /// Place neurons, wire them up and rasterize the static structure.
    fn grow(&mut self, rng: &mut Rng) {
        let (w, ph) = (self.w as f32, self.ph as f32);
        let scale = (ph / 48.0).max(0.6);
        self.palette = rng.below(PALETTES.len());
        self.neurons.clear();
        self.conns.clear();
        self.spikes.clear();
        self.puffs.clear();
        self.structure.fill(0.0);
        self.reveal.fill(0);
        self.light.fill(0.0);
        self.light2.fill(0.0);
        self.gain = 1.0;

        // Somas spread out: reject spots too close to an existing one.
        let target = ((w * ph) / (320.0 * scale * scale)).clamp(4.0, 90.0) as usize;
        let spacing = (w * ph / target as f32).sqrt() * 0.65;
        for _ in 0..target * 30 {
            if self.neurons.len() >= target {
                break;
            }
            let (x, y) = (rng.rangef(0.05, 0.95) * w, rng.rangef(0.08, 0.92) * ph);
            if self.neurons.iter().all(|n| (n.x - x).powi(2) + (n.y - y).powi(2) > spacing * spacing) {
                self.neurons.push(Neuron {
                    x,
                    y,
                    r: rng.rangef(1.3, 2.4) * scale,
                    v: rng.rangef(0.0, 0.6),
                    refr: 0,
                    out: Vec::new(),
                });
            }
        }

        // Each neuron's axon reaches its nearest few neighbours; later
        // branches fork off the first one, so axons look like trees.
        let count = self.neurons.len();
        for i in 0..count {
            let (xi, yi, ri) = (self.neurons[i].x, self.neurons[i].y, self.neurons[i].r);
            let mut near: Vec<(f32, usize)> = (0..count)
                .filter(|&j| j != i)
                .map(|j| ((self.neurons[j].x - xi).powi(2) + (self.neurons[j].y - yi).powi(2), j))
                .collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            let k = rng.range(2, 5) as usize;
            let mut trunk: Vec<(f32, f32)> = Vec::new();
            let delay = rng.range(0, 40) as u32;
            for &(d2, j) in near.iter().take(k) {
                if d2.sqrt() > spacing * 2.6 {
                    break;
                }
                let (tx, ty, tr) = (self.neurons[j].x, self.neurons[j].y, self.neurons[j].r);
                let pts = if trunk.is_empty() {
                    let a = (ty - yi).atan2(tx - xi) + rng.rangef(-0.8, 0.8);
                    let start = (xi + a.cos() * ri, yi + a.sin() * ri);
                    let mut p = vec![start];
                    p.extend(walk(start, a, (tx, ty), tr + 1.5, rng));
                    trunk = p.clone();
                    p
                } else {
                    let fork = ((trunk.len() as f32) * rng.rangef(0.2, 0.55)) as usize;
                    let fork = fork.min(trunk.len() - 1);
                    let from = trunk[fork];
                    let prev = trunk[fork.saturating_sub(1)];
                    let a = (from.1 - prev.1).atan2(from.0 - prev.0) + rng.rangef(-0.9, 0.9);
                    let mut p = trunk[..=fork].to_vec();
                    p.extend(walk(from, a, (tx, ty), tr + 1.5, rng));
                    p
                };
                let path = pixels(&pts);
                for (s, &(x, y)) in path.iter().enumerate() {
                    self.mark(x, y, 0.9, delay + 10 + s as u32);
                }
                // The synaptic bouton at the end.
                if let Some(&(bx, by)) = path.last() {
                    let at = delay + 10 + path.len() as u32;
                    self.mark(bx, by, 1.0, at);
                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        self.mark(bx + dx, by + dy, 0.55, at);
                    }
                }
                let inhibitory = rng.chance(0.2);
                self.neurons[i].out.push(self.conns.len());
                self.conns.push(Conn { to: j, path, weight: rng.rangef(0.35, 0.7), inhibitory });
            }
        }

        // Dendrites: short, dim, branching tufts around each soma.
        for i in 0..count {
            let (x, y, r) = (self.neurons[i].x, self.neurons[i].y, self.neurons[i].r);
            for _ in 0..rng.range(3, 7) {
                let a = rng.rangef(0.0, TAU);
                let len = rng.rangef(3.0, 8.0) * scale;
                let start = (x + a.cos() * r, y + a.sin() * r);
                let end = (start.0 + a.cos() * len, start.1 + a.sin() * len);
                let pts = walk(start, a, end, 1.0, rng);
                let mut path = pixels(&pts);
                if path.len() > 3 && rng.chance(0.6) {
                    let fork = path.len() / 2;
                    let (fx, fy) = (path[fork].0 as f32, path[fork].1 as f32);
                    let b = a + if rng.chance(0.5) { 0.8 } else { -0.8 };
                    let tip = (fx + b.cos() * len * 0.5, fy + b.sin() * len * 0.5);
                    path.extend(pixels(&walk((fx, fy), b, tip, 1.0, rng)));
                }
                for (s, &(px, py)) in path.iter().enumerate() {
                    self.mark(px, py, 0.5 - 0.02 * s as f32, 4 + 2 * s as u32);
                }
            }
            // The soma itself: bright core, softer rim.
            let rr = r.ceil() as i32 + 1;
            for dy in -rr..=rr {
                for dx in -rr..=rr {
                    let d = ((dx * dx + dy * dy) as f32).sqrt();
                    if d <= r + 0.5 {
                        // Brighter than the neurites: the cell bodies anchor the picture.
                        self.mark(x as i32 + dx, y as i32 + dy, 1.9 - 0.8 * d / r, 1);
                    }
                }
            }
        }

        // Fluorescent halo: blend in a blurred copy.
        let blurred = self.blurred(&self.structure);
        for i in 0..self.structure.len() {
            let halo = blurred[i] * 0.7;
            if halo > self.structure[i] {
                self.structure[i] = halo;
            }
        }
        // Halo pixels appear with their brightest neighbour.
        let (wi, hi) = (self.w as i32, self.ph as i32);
        let reveal = self.reveal.clone();
        for y in 0..hi {
            for x in 0..wi {
                let i = (y * wi + x) as usize;
                if reveal[i] == 0 && self.structure[i] > 0.0 {
                    let mut best = u16::MAX;
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            if let Some(j) = self.idx(x + dx, y + dy) {
                                if reveal[j] != 0 {
                                    best = best.min(reveal[j]);
                                }
                            }
                        }
                    }
                    self.reveal[i] = if best == u16::MAX { 1 } else { best };
                }
            }
        }

        // Out-of-focus tissue behind: random strands, blurred twice.
        self.far.fill(0.0);
        for _ in 0..(target * 2) {
            let start = (rng.rangef(0.0, w), rng.rangef(0.0, ph));
            let a = rng.rangef(0.0, TAU);
            let len = rng.rangef(8.0, 30.0) * scale;
            let end = (start.0 + a.cos() * len, start.1 + a.sin() * len);
            for (x, y) in pixels(&walk(start, a, end, 1.5, rng)) {
                if let Some(i) = self.idx(x, y) {
                    self.far[i] = 1.0;
                }
            }
        }
        let once = self.blurred(&self.far);
        self.far = self.blurred(&once);
    }

    /// 3x3 box blur, clamped at the edges.
    fn blurred(&self, src: &[f32]) -> Vec<f32> {
        let (w, ph) = (self.w, self.ph);
        let mut out = vec![0.0; w * ph];
        for y in 0..ph {
            for x in 0..w {
                let mut s = 0.0;
                for yy in y.saturating_sub(1)..(y + 2).min(ph) {
                    for xx in x.saturating_sub(1)..(x + 2).min(w) {
                        s += src[yy * w + xx];
                    }
                }
                out[y * w + x] = s / 9.0;
            }
        }
        out
    }

    fn glow(buf: &mut [f32], w: usize, ph: usize, x: f32, y: f32, radius: f32, power: f32) {
        let r = radius.ceil() as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                let (px, py) = (x as i32 + dx, y as i32 + dy);
                if px < 0 || py < 0 || px as usize >= w || py as usize >= ph {
                    continue;
                }
                let d = ((dx * dx + dy * dy) as f32).sqrt();
                if d <= radius {
                    buf[py as usize * w + px as usize] += power * (1.0 - d / (radius + 1.0));
                }
            }
        }
    }

    fn fire(&mut self, i: usize) {
        let n = &mut self.neurons[i];
        n.v = 0.0;
        n.refr = REFRACTORY;
        let (x, y, r) = (n.x, n.y, n.r);
        Self::glow(&mut self.light, self.w, self.ph, x, y, r + 2.5, 1.4);
        for k in 0..self.neurons[i].out.len() {
            let conn = self.neurons[i].out[k];
            self.spikes.push(Spike { conn, pos: 0.0 });
        }
    }

    fn simulate(&mut self, rng: &mut Rng) {
        let scale = (self.ph as f32 / 48.0).max(0.6);
        // Homeostasis: nudge synaptic gain to keep activity lively but sane.
        let busy = self.spikes.len() as f32 / self.conns.len().max(1) as f32;
        if busy > 0.6 {
            self.gain = (self.gain * 0.99).max(0.3);
        } else if busy < 0.2 {
            self.gain = (self.gain * 1.01).min(1.8);
        }
        let spontaneous = if busy < 0.1 { 0.01 } else { 0.003 };
        let mut firing = Vec::new();
        for (i, n) in self.neurons.iter_mut().enumerate() {
            n.v = (n.v * 0.985 + rng.rangef(0.0, 0.004)).max(-0.5);
            if n.refr > 0 {
                n.refr -= 1;
            } else if n.v >= 1.0 || rng.chance(spontaneous) {
                firing.push(i);
            }
        }
        for i in firing {
            self.fire(i);
        }

        // Spikes travel along their axon; arriving ones cross the synapse.
        let speed = 1.3 * scale;
        let mut arrived = Vec::new();
        self.spikes.retain_mut(|s| {
            s.pos += speed;
            if s.pos as usize >= self.conns[s.conn].path.len() {
                arrived.push(s.conn);
                false
            } else {
                true
            }
        });
        for c in arrived {
            let conn = &self.conns[c];
            let Some(&(bx, by)) = conn.path.last() else { continue };
            let (to, inhibitory) = (conn.to, conn.inhibitory);
            let push = conn.weight * self.gain * if inhibitory { -0.8 } else { 1.0 };
            let (bx, by) = (bx as f32, by as f32);
            let buf = if inhibitory { &mut self.light2 } else { &mut self.light };
            Self::glow(buf, self.w, self.ph, bx, by, 2.0 * scale, 1.2);
            // Neurotransmitter drifting across the cleft towards the soma.
            let (tx, ty) = (self.neurons[to].x, self.neurons[to].y);
            let a = (ty - by).atan2(tx - bx);
            for _ in 0..rng.range(3, 6) {
                let b = a + rng.rangef(-0.7, 0.7);
                let v = rng.rangef(0.15, 0.4) * scale;
                self.puffs.push(Puff { x: bx, y: by, vx: b.cos() * v, vy: b.sin() * v, life: rng.range(10, 22) as u32, inhibitory });
            }
            self.neurons[to].v += push;
        }
        for p in self.puffs.iter_mut() {
            p.x += p.vx;
            p.y += p.vy;
            p.life -= 1;
        }
        self.puffs.retain(|p| p.life > 0);
    }

    fn draw_light(&mut self) {
        for (a, b) in self.light.iter_mut().zip(self.light2.iter_mut()) {
            *a *= 0.72;
            *b *= 0.72;
        }
        let (w, ph) = (self.w, self.ph);
        for s in &self.spikes {
            let conn = &self.conns[s.conn];
            let head = (s.pos as usize).min(conn.path.len().saturating_sub(1));
            let buf = if conn.inhibitory { &mut self.light2 } else { &mut self.light };
            // Bright head with a short fading tail behind it.
            for k in 0..7 {
                if k > head {
                    break;
                }
                let (x, y) = conn.path[head - k];
                if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < ph {
                    buf[y as usize * w + x as usize] += 1.3 * 0.7f32.powi(k as i32);
                }
            }
        }
        for p in &self.puffs {
            let (x, y) = (p.x as i32, p.y as i32);
            if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < ph {
                let buf = if p.inhibitory { &mut self.light2 } else { &mut self.light };
                buf[y as usize * w + x as usize] += 0.08 * p.life as f32;
            }
        }
        // Somas glow with their membrane potential.
        for n in &self.neurons {
            if n.v > 0.05 {
                Self::glow(&mut self.light, w, ph, n.x, n.y, n.r, 0.12 * n.v.min(1.0));
            }
        }
    }
}

impl Animation for Neurons {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        if self.w == 0 || self.ph == 0 {
            return;
        }
        self.frame += 1;
        let phase = self.frame % EPOCH;
        if phase == 0 {
            self.grow(rng);
        }
        if phase > GROW && phase < EPOCH - FADE {
            self.simulate(rng);
        }
        self.draw_light();

        let fade = if phase > EPOCH - FADE { (EPOCH - phase) as f32 / FADE as f32 } else { (phase as f32 / 30.0).min(1.0) };
        let (neurite, exc, inh) = PALETTES[self.palette];
        // Bloom: add a blurred copy of the light so flashes bleed softly.
        let (w, ph) = (self.w, self.ph);
        for y in 0..ph {
            for x in 0..w {
                let (mut s1, mut s2) = (0.0, 0.0);
                for yy in y.saturating_sub(1)..(y + 2).min(ph) {
                    for xx in x.saturating_sub(1)..(x + 2).min(w) {
                        s1 += self.light[yy * w + xx];
                        s2 += self.light2[yy * w + xx];
                    }
                }
                self.bloom[y * w + x] = s1 / 9.0;
                self.bloom2[y * w + x] = s2 / 9.0;
            }
        }
        for i in 0..w * ph {
            let grown = if self.reveal[i] == 0 {
                0.0
            } else {
                ((phase as f32 - self.reveal[i] as f32) / 12.0).clamp(0.0, 1.0)
            };
            let st = self.structure[i] * grown * 0.55 + self.far[i] * 0.22;
            let l1 = self.light[i] * 0.6 + self.bloom[i] * 0.9;
            let l2 = self.light2[i] * 0.6 + self.bloom2[i] * 0.9;
            let (st, l1, l2) = (st * fade, l1 * fade, l2 * fade);
            if st < 0.015 && l1 < 0.015 && l2 < 0.015 {
                self.px[i] = BG;
                continue;
            }
            let hot = (l1 + l2 - 1.0).max(0.0) * 0.6;
            let ch = |b: u8, n: u8, e: u8, j: u8| {
                (b as f32 + n as f32 * st + e as f32 * l1 + j as f32 * l2 + 255.0 * hot).min(255.0) as u8
            };
            self.px[i] = Rgb(
                ch(BG.0, neurite.0, exc.0, inh.0),
                ch(BG.1, neurite.1, exc.1, inh.1),
                ch(BG.2, neurite.2, exc.2, inh.2),
            );
        }
        c.blit_pixels(&self.px);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    Box::new(Neurons::new(w, h, rng))
}
