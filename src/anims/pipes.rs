//! The classic pipes screensaver: colored pipes grow and turn at random,
//! respawning at the edges, until the screen fills up and gets wiped.
//! Pipes are shaded pixel cylinders lit from the top left, joined at turns by
//! rounded elbows, balls or a mix of both depending on the round.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;

pub const BG: Rgb = Rgb(6, 7, 14);

/// How turns are drawn in a round, like the original's joint setting.
#[derive(Clone, Copy)]
enum Joints {
    Elbow,
    Ball,
    Mixed,
}

const JOINTS: [Joints; 3] = [Joints::Elbow, Joints::Ball, Joints::Mixed];

// Directions: 0 up, 1 right, 2 down, 3 left.
const DX: [i32; 4] = [0, 1, 0, -1];
const DY: [i32; 4] = [-1, 0, 1, 0];

struct Pipe {
    x: i32,
    y: i32,
    dir: usize,
    /// Side the pipe leaves the current cell by, once chosen.
    out: usize,
    /// The first half of the cell (entry and bend) is drawn; the exit is next.
    half: bool,
    color: Rgb,
}

struct Pipes {
    w: usize,
    h: usize,
    /// Grid cell size, pipe thickness and the pipe's offset in a cell, in pixels.
    cs: i32,
    t: i32,
    o: i32,
    gw: i32,
    gh: i32,
    pipes: Vec<Pipe>,
    painted: Vec<bool>,
    count: usize,
    style: usize,
    wipe: Option<i32>,
    hue: f32,
    fresh: bool,
    steps: usize,
}

/// Lit color of a surface whose normal has in-plane part `(nx, ny)`, with
/// the light coming from the top left and in front of the screen.
fn shade(col: Rgb, nx: f32, ny: f32) -> Rgb {
    let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
    let diff = (-0.45 * nx - 0.6 * ny + 0.66 * nz).max(0.0);
    let spec = diff.powi(14);
    col.scale(0.2 + 0.95 * diff).lerp(Rgb::WHITE, spec * 0.75)
}

impl Pipes {
    fn new(w: usize, h: usize, rng: &mut Rng) -> Pipes {
        let cs = if 2 * h >= 100 { 6 } else { 4 };
        let t = if cs == 6 { 5 } else { 3 };
        let gw = w as i32 / cs;
        let gh = 2 * h as i32 / cs;
        let n = ((gw * gh) as usize / 300).clamp(2, 6);
        let mut p = Pipes {
            w,
            h,
            cs,
            t,
            o: (cs - t) / 2,
            gw,
            gh,
            pipes: Vec::with_capacity(n),
            painted: vec![false; (gw * gh).max(0) as usize],
            count: 0,
            style: rng.below(JOINTS.len()),
            wipe: None,
            hue: rng.f32(),
            fresh: true,
            steps: 0,
        };
        for _ in 0..n {
            let pipe = p.spawn(rng);
            p.pipes.push(pipe);
        }
        p
    }

    fn spawn(&mut self, rng: &mut Rng) -> Pipe {
        self.hue += 0.618_034;
        let color = Rgb::hsv(self.hue, rng.rangef(0.55, 0.85), rng.rangef(0.8, 1.0));
        let dir = rng.below(4);
        let (x, y) = match dir {
            0 => (rng.range(0, self.gw), self.gh - 1),
            1 => (0, rng.range(0, self.gh)),
            2 => (rng.range(0, self.gw), 0),
            _ => (self.gw - 1, rng.range(0, self.gh)),
        };
        Pipe { x, y, dir, out: dir, half: false, color }
    }

    /// Top-left pixel of the pipe's cross-section square in cell `(x, y)`.
    fn core(&self, x: i32, y: i32) -> (i32, i32) {
        (x * self.cs + self.o, y * self.cs + self.o)
    }

    /// Straight run from the core square out to the cell edge on `side`.
    fn arm(&self, c: &mut Canvas, x: i32, y: i32, side: usize, col: Rgb) {
        let (bx, by) = self.core(x, y);
        let (x0, y0) = (x * self.cs, y * self.cs);
        let t = self.t;
        let (xs, ys) = match side {
            0 => (bx..bx + t, y0..by),
            1 => (bx + t..x0 + self.cs, by..by + t),
            2 => (bx..bx + t, by + t..y0 + self.cs),
            _ => (x0..bx, by..by + t),
        };
        for py in ys {
            for px in xs.clone() {
                // Shading runs across the pipe, so along a run it is constant.
                let v = if side % 2 == 1 { py - by } else { px - bx };
                let v = (v as f32 + 0.5) / t as f32 * 2.0 - 1.0;
                let (nx, ny) = if side % 2 == 1 { (0.0, v) } else { (v, 0.0) };
                c.pixel(px, py, shade(col, nx, ny));
            }
        }
    }

    /// The core square: straight through, or a rounded bend between `a` and
    /// `b` shaded as a torus around the inner corner.
    fn bend(&self, c: &mut Canvas, x: i32, y: i32, a: usize, b: usize, col: Rgb) {
        let (bx, by) = self.core(x, y);
        let t = self.t as f32;
        let r = t / 2.0;
        let straight = a % 2 == b % 2;
        let pivot = (
            bx as f32 + if a == 1 || b == 1 { t } else { 0.0 },
            by as f32 + if a == 2 || b == 2 { t } else { 0.0 },
        );
        for j in 0..self.t {
            for i in 0..self.t {
                let (qx, qy) = (bx as f32 + i as f32 + 0.5, by as f32 + j as f32 + 0.5);
                let (nx, ny) = if straight {
                    let v = if a % 2 == 1 { j } else { i };
                    let v = (v as f32 + 0.5) / t * 2.0 - 1.0;
                    if a % 2 == 1 { (0.0, v) } else { (v, 0.0) }
                } else {
                    let (dx, dy) = (qx - pivot.0, qy - pivot.1);
                    let d = (dx * dx + dy * dy).sqrt().max(1e-3);
                    let u = (d - r) / r;
                    if u.abs() > 1.0 {
                        continue;
                    }
                    (dx / d * u, dy / d * u)
                };
                c.pixel(bx + i, by + j, shade(col, nx, ny));
            }
        }
    }

    /// A ball joint centered on the core square, a bit fatter than the pipe.
    fn ball(&self, c: &mut Canvas, x: i32, y: i32, col: Rgb) {
        let (bx, by) = self.core(x, y);
        let (cx, cy) = (bx as f32 + self.t as f32 / 2.0, by as f32 + self.t as f32 / 2.0);
        let r = self.t as f32 / 2.0 + 1.3;
        for py in (cy - r).floor() as i32..(cy + r).ceil() as i32 {
            for px in (cx - r).floor() as i32..(cx + r).ceil() as i32 {
                let (dx, dy) = ((px as f32 + 0.5 - cx) / r, (py as f32 + 0.5 - cy) / r);
                if dx * dx + dy * dy <= 1.0 {
                    c.pixel(px, py, shade(col, dx, dy));
                }
            }
        }
    }

    /// Every frame each pipe grows by half a cell: first the entry and the
    /// bend, then the exit and any ball joint, before moving on.
    fn grow(&mut self, c: &mut Canvas, rng: &mut Rng) {
        for i in 0..self.pipes.len() {
            let (x, y, dir, color) = {
                let p = &self.pipes[i];
                (p.x, p.y, p.dir, p.color)
            };
            if !self.pipes[i].half {
                let turn = rng.chance(if dir % 2 == 1 { 0.12 } else { 0.2 });
                let out = if turn { (dir + if rng.chance(0.5) { 1 } else { 3 }) % 4 } else { dir };
                // `dir` is the travel direction when entering the cell, so the
                // pipe comes in from its opposite side.
                let entry = (dir + 2) % 4;
                self.arm(c, x, y, entry, color);
                self.bend(c, x, y, entry, out, color);
                let k = (y * self.gw + x) as usize;
                if !self.painted[k] {
                    self.painted[k] = true;
                    self.count += 1;
                }
                let p = &mut self.pipes[i];
                p.out = out;
                p.half = true;
                continue;
            }
            let out = self.pipes[i].out;
            self.arm(c, x, y, out, color);
            if out != dir {
                let ball = match JOINTS[self.style] {
                    Joints::Elbow => false,
                    Joints::Ball => true,
                    Joints::Mixed => rng.chance(0.5),
                };
                if ball {
                    self.ball(c, x, y, color);
                }
            }
            self.steps += 1;
            let (nx, ny) = (x + DX[out], y + DY[out]);
            if nx < 0 || ny < 0 || nx >= self.gw || ny >= self.gh {
                self.pipes[i] = self.spawn(rng);
            } else {
                let p = &mut self.pipes[i];
                p.x = nx;
                p.y = ny;
                p.dir = out;
                p.half = false;
            }
        }
    }
}

impl Animation for Pipes {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        if self.fresh {
            c.clear(BG);
            self.fresh = false;
        }
        if self.gw < 1 || self.gh < 1 {
            return;
        }
        if let Some(col) = self.wipe {
            // Diagonal-ish wipe sweeping left to right.
            let rows = self.h as i32;
            let speed = (self.w as i32 / 40).max(2);
            for x in col..(col + speed) {
                for y in 0..rows {
                    c.set(x - y / 2, y, ' ', BG, BG);
                }
            }
            let next = col + speed;
            if next - rows / 2 > self.w as i32 {
                c.clear(BG);
                self.wipe = None;
                self.painted.fill(false);
                self.count = 0;
                self.steps = 0;
                self.style = (self.style + 1 + rng.below(JOINTS.len() - 1)) % JOINTS.len();
                for i in 0..self.pipes.len() {
                    self.pipes[i] = self.spawn(rng);
                }
            } else {
                self.wipe = Some(next);
            }
            return;
        }
        self.grow(c, rng);
        let cells = self.painted.len();
        if self.count * 10 >= cells * 6 || self.steps >= cells * 2 {
            self.wipe = Some(0);
        }
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    Box::new(Pipes::new(w, h, rng))
}
