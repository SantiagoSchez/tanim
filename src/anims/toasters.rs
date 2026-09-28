//! Flying toasters, after the After Dark screensaver of 1989: chrome toasters
//! flapping feathered wings, some with a slice of toast in the slot, drift
//! diagonally down across a black sky together with loose slices of toast.
//!
//! Everything is drawn procedurally in half-block pixels, in a fixed
//! three-quarter view like the original: a box whose long side, top (with two
//! slots) and leading end each get their own chrome shading, fans of capsule
//! feathers for wings, and slices with a little thickness. So the same shapes
//! work at any size. Three depth layers give parallax: far ones are smaller,
//! slower and dimmer. Flyers in a layer share its speed, so once spawned apart
//! they never collide, and each layer is drawn over the ones behind it. Space
//! cycles how toasted the toast is, from light to burnt.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;
use std::f32::consts::TAU;

pub const BG: Rgb = Rgb(0, 0, 0);

/// Direction of flight in pixels: down and to the left.
const DIR: (f32, f32) = (-0.876, 0.482);
/// Size of each depth layer relative to the nearest one, and its brightness.
const LAYERS: [(f32, f32); 3] = [(0.55, 0.5), (0.75, 0.72), (1.0, 1.0)];
/// How often each layer is picked for a new flyer (more far away).
const LAYER_ODDS: [f32; 3] = [0.34, 0.33, 0.33];
const TOAST_ODDS: f32 = 0.25;
/// Pixels per frame, relative to the flyer's size.
const SPEED: f32 = 0.05;

/// The toaster's side, top to bottom: highlight, sky reflection, dark band,
/// ground reflection and shadow.
const CHROME: [Rgb; 6] = [
    Rgb(250, 251, 255),
    Rgb(196, 204, 218),
    Rgb(118, 126, 144),
    Rgb(208, 216, 230),
    Rgb(160, 168, 184),
    Rgb(96, 102, 118),
];
const TOP: Rgb = Rgb(214, 220, 232);
const SLOT: Rgb = Rgb(26, 26, 32);
const BASE: Rgb = Rgb(58, 60, 72);
const LEVER: Rgb = Rgb(34, 34, 42);
const FEATHER: Rgb = Rgb(246, 246, 250);
const FEATHER_EDGE: Rgb = Rgb(146, 152, 170);
/// Feather lengths from the leading edge down, relative to the body width.
const FEATHERS: [f32; 4] = [1.0, 0.9, 0.78, 0.64];
/// Toast face and crust for each doneness: light, golden, dark and burnt.
const TOAST: [(Rgb, Rgb); 4] = [
    (Rgb(240, 218, 166), Rgb(196, 150, 90)),
    (Rgb(222, 166, 84), Rgb(150, 96, 44)),
    (Rgb(160, 96, 44), Rgb(98, 56, 26)),
    (Rgb(78, 50, 32), Rgb(44, 28, 20)),
];

enum Kind {
    Toaster { toast: bool, flap: f32, rate: f32 },
    Toast,
}

struct Flyer {
    /// Top-left corner of the body (or slice), in pixels.
    x: f32,
    y: f32,
    layer: usize,
    kind: Kind,
}

struct Toasters {
    w: usize,
    ph: usize,
    px: Vec<Rgb>,
    flyers: Vec<Flyer>,
    /// Body width of a toaster in each layer.
    size: [f32; 3],
    level: usize,
}

/// Area a flyer can cover, as (x, y, w, h) from its corner: the toaster's
/// top and near end recede up and to the left, its raised wings reach up and
/// back, and a slice of toast stands out of each slot.
fn extent(kind: &Kind, s: f32) -> (f32, f32, f32, f32) {
    match kind {
        Kind::Toaster { .. } => (-0.3 * s, -1.2 * s, 2.2 * s, 1.95 * s),
        Kind::Toast => (-0.1 * s, -0.1 * s, 0.82 * s, 0.76 * s),
    }
}

fn put(px: &mut [Rgb], w: usize, ph: usize, x: i32, y: i32, c: Rgb, dim: f32) {
    if x < 0 || y < 0 || x as usize >= w || y as usize >= ph {
        return;
    }
    let c = c.scale(dim);
    // Near-black would show as a dark box on a transparent terminal.
    px[y as usize * w + x as usize] = if c.0 <= 2 && c.1 <= 2 && c.2 <= 2 { BG } else { c };
}

/// Whether pixel (i, j) of a `w x h` box falls inside it once its top corners
/// are rounded by `rt` pixels and its bottom ones by `rb`.
fn rounded(i: i32, j: i32, w: i32, h: i32, rt: i32, rb: i32) -> bool {
    let corner = |r: i32, dx: i32, dy: i32| {
        if r <= 0 || dx >= r || dy >= r {
            return true;
        }
        let (fx, fy) = ((r - dx) as f32 - 0.5, (r - dy) as f32 - 0.5);
        fx * fx + fy * fy <= (r * r) as f32
    };
    let dx = i.min(w - 1 - i);
    if j < h / 2 {
        corner(rt, dx, j)
    } else {
        corner(rb, dx, h - 1 - j)
    }
}

/// A slice of bread: a rounded top over a slightly narrower base, crust
/// around a face that darkens a little towards it. Rows from `clip` down are
/// left out (hidden in a slot), and `thick` pixels of crust stack up and to
/// the left behind the face, the slice's own depth.
fn draw_slice(px: &mut [Rgb], w: usize, ph: usize, x0: i32, y0: i32, tw: i32, th: i32, (face, crust): (Rgb, Rgb), clip: i32, thick: i32, dim: f32) {
    let r = (tw / 3).min(th * 2 / 5).max(1);
    let neck = (th as f32 * 0.38).round() as i32;
    let inset = i32::from(tw >= 7);
    let inside = |i: i32, j: i32| {
        i >= 0 && j >= 0 && i < tw && j < th && rounded(i, j, tw, th, r, 1) && (j < neck || (i >= inset && i < tw - inset))
    };
    for k in (0..=thick).rev() {
        for j in 0..th.min(clip.saturating_sub(y0).saturating_add(k)) {
            for i in 0..tw {
                if !inside(i, j) {
                    continue;
                }
                let edge = |d: i32| !inside(i - d, j) || !inside(i + d, j) || !inside(i, j - d) || !inside(i, j + d);
                let c = if k > 0 {
                    crust.scale(0.8)
                } else if edge(1) {
                    crust
                } else if tw >= 8 && edge(2) {
                    face.lerp(crust, 0.3)
                } else {
                    face
                };
                put(px, w, ph, x0 + i - k, y0 + j - k, c, dim);
            }
        }
    }
}

/// A wing: a fan of capsule feathers from `pivot`, the leading one `a`
/// radians above the backward horizontal and each next one lower and
/// shorter, their roots hidden under a broad covert. Only the outline of the
/// whole wing is dark; where feathers overlap a lighter line parts them.
fn draw_wing(px: &mut [Rgb], w: usize, ph: usize, pivot: (f32, f32), s: f32, a: f32, dim: f32) {
    // (start, end, radius) of each capsule, front to back: covert first.
    let mut caps = Vec::with_capacity(FEATHERS.len() + 1);
    let ray = |ang: f32, len: f32| (pivot.0 + ang.cos() * len * s, pivot.1 - ang.sin() * len * s);
    caps.push((pivot, ray(a - 0.3, 0.3), (s * 0.14).max(0.75)));
    for (i, len) in FEATHERS.iter().enumerate() {
        caps.push((pivot, ray(a - i as f32 * 0.22, *len), (s * 0.1).max(0.75)));
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &(p, q, r) in &caps {
        x0 = x0.min(p.0.min(q.0) - r);
        y0 = y0.min(p.1.min(q.1) - r);
        x1 = x1.max(p.0.max(q.0) + r);
        y1 = y1.max(p.1.max(q.1) + r);
    }
    let reach = FEATHERS[0] * s;
    for y in y0.floor() as i32..=y1.ceil() as i32 {
        for x in x0.floor() as i32..=x1.ceil() as i32 {
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            // Depth of the pixel inside each capsule (negative outside).
            let depth = caps.iter().map(|&(p, q, r)| {
                let (dx, dy, qx, qy) = (q.0 - p.0, q.1 - p.1, cx - p.0, cy - p.1);
                let t = ((qx * dx + qy * dy) / (dx * dx + dy * dy).max(1e-6)).clamp(0.0, 1.0);
                r - ((qx - t * dx).powi(2) + (qy - t * dy).powi(2)).sqrt()
            });
            let depth: Vec<f32> = depth.collect();
            let Some(front) = depth.iter().position(|&d| d >= 0.0) else { continue };
            let thin = caps[front].2 < 1.5;
            let c = if !thin && depth[front] < 1.0 {
                if depth.iter().enumerate().any(|(j, &d)| j != front && d >= 1.0) {
                    FEATHER.lerp(FEATHER_EDGE, 0.5)
                } else {
                    FEATHER_EDGE
                }
            } else {
                let along = ((cx - pivot.0).powi(2) + (cy - pivot.1).powi(2)).sqrt() / reach;
                FEATHER.lerp(FEATHER_EDGE, (along * 0.35).min(0.35))
            };
            put(px, w, ph, x, y, c, dim);
        }
    }
}

/// A chrome toaster in three-quarter view, flying left: its long side faces
/// us with the body's corner at (ox, oy), and the top (two slots) and the
/// leading end recede up and to the left. Drawn back to front: far wing, body,
/// the slices standing in the slots, near wing.
fn draw_toaster(px: &mut [Rgb], w: usize, ph: usize, ox: i32, oy: i32, s: f32, flap: f32, toast: Option<(Rgb, Rgb)>, dim: f32) {
    let len = (s.round() as i32).max(4);
    let hgt = ((s * 0.62).round() as i32).max(3);
    // Depth offset of the far side.
    let (ddx, ddy) = (((s * 0.28).round() as i32).max(1), ((s * 0.25).round() as i32).max(1));
    let a = 0.55 + 0.9 * flap.sin();
    let pivot = (ox as f32 + 0.74 * s, oy as f32 + 0.15 * hgt as f32);
    draw_wing(px, w, ph, (pivot.0 - ddx as f32, pivot.1 - ddy as f32), s, a, dim * 0.62);

    let (lf, hf, df) = (len as f32, hgt as f32, (ddx as f32, ddy as f32));
    let base = ((hf * 0.12).round() as i32).max(1);
    let lever = (lf * 0.86).round() as i32;
    let knob = (hf * 0.45) as i32;
    let shine = (lf * 0.1).round() as i32;
    for j in -ddy..hgt {
        for i in -ddx..len {
            let (x, y) = (i as f32 + 0.5, j as f32 + 0.5);
            let c = if i >= 0 && j >= 0 {
                // The long side: chrome bands, darker towards both ends.
                let u = x / lf;
                if j >= hgt - base {
                    BASE
                } else if len >= 8 && (i == lever || i == lever + 1) && j == knob {
                    LEVER
                } else if len >= 8 && i == lever && (j - knob).abs() <= hgt / 5 {
                    LEVER.lerp(CHROME[4], 0.3)
                } else {
                    let v = j as f32 / (hgt - base - 1).max(1) as f32;
                    let c = Rgb::gradient(&CHROME, v).scale(1.0 - 0.3 * (2.0 * u - 1.0).abs().powi(4));
                    if j == 0 {
                        c.lerp(Rgb::WHITE, 0.6)
                    } else if len >= 10 && i == shine {
                        c.lerp(Rgb::WHITE, 0.45)
                    } else {
                        c
                    }
                }
            } else {
                // The top, where b runs from the near edge (0) to the far one
                // (1), else the leading end, in shade.
                let (bt, ut) = (-y / df.1, (x - y / df.1 * df.0) / lf);
                let (be, ve) = (-x / df.0, (y - x / df.0 * df.1) / hf);
                if j < 0 && (0.0..=1.0).contains(&bt) && (0.0..=1.0).contains(&ut) {
                    let slot = (0.22..0.4).contains(&bt) || (0.6..0.78).contains(&bt);
                    if slot && (0.12..0.88).contains(&ut) { SLOT } else { TOP.lerp(CHROME[2], bt * 0.35) }
                } else if i < 0 && (0.0..=1.0).contains(&be) && (0.0..=1.0).contains(&ve) {
                    if ve >= 1.0 - base as f32 / hf { BASE.scale(0.8) } else { Rgb::gradient(&CHROME, ve).scale(0.62) }
                } else {
                    continue;
                }
            };
            put(px, w, ph, ox + i, oy + j, c, dim);
        }
    }

    if let Some(t) = toast {
        // One slice per slot, far one first; each stands in its slot's plane
        // and the top hides what is below the slot.
        let tw = ((lf * 0.6).round() as i32).max(2);
        let th = ((hf * 0.75).round() as i32).max(2);
        let rise = ((hf * 0.38).round() as i32).max(1);
        for b in [0.69, 0.31] {
            let (sx, sy) = (ox - (b * df.0).round() as i32, oy - (b * df.1).round() as i32);
            draw_slice(px, w, ph, sx + (lf * 0.2).round() as i32, sy - rise, tw, th, t, sy, 0, dim);
        }
    }
    draw_wing(px, w, ph, pivot, s, a, dim);
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    let ph = 2 * h;
    let near = (ph as f32 * 0.13).min(w as f32 * 0.085).clamp(6.0, 26.0);
    let mut t = Toasters {
        w,
        ph,
        px: vec![BG; w * ph],
        flyers: Vec::new(),
        size: LAYERS.map(|(k, _)| near * k),
        level: 1,
    };
    let n = ((w * ph) as f32 / (near * near * 5.0)).round().clamp(3.0, 40.0) as usize;
    for _ in 0..n {
        let mut f = t.spawn(rng);
        // Fill the screen from the start instead of waiting for a first wave.
        let s = t.size[f.layer];
        let (bx, by, bw, bh) = extent(&f.kind, s);
        for _ in 0..8 {
            let (x, y) = (rng.rangef(-bw, w as f32) - bx, rng.rangef(-bh, ph as f32) - by);
            if t.free(x, y, &f) {
                (f.x, f.y) = (x, y);
                break;
            }
        }
        t.flyers.push(f);
    }
    t.flyers.sort_by_key(|f| f.layer);
    Box::new(t)
}

impl Toasters {
    /// Whether `f` placed at (x, y) keeps clear of every flyer in its layer.
    fn free(&self, x: f32, y: f32, f: &Flyer) -> bool {
        let s = self.size[f.layer];
        let m = 0.15 * s;
        let (ax, ay, aw, ah) = extent(&f.kind, s);
        let (ax, ay) = (x + ax, y + ay);
        self.flyers.iter().filter(|o| o.layer == f.layer).all(|o| {
            let (bx, by, bw, bh) = extent(&o.kind, s);
            let (bx, by) = (o.x + bx, o.y + by);
            ax + aw + m <= bx || bx + bw + m <= ax || ay + ah + m <= by || by + bh + m <= ay
        })
    }

    /// A new flyer just off the top or right edge, weighted by how much of the
    /// flow crosses each, and pushed back along its lane until it is clear.
    fn spawn(&self, rng: &mut Rng) -> Flyer {
        let roll = rng.f32();
        let layer = if roll < LAYER_ODDS[0] { 0 } else if roll < LAYER_ODDS[0] + LAYER_ODDS[1] { 1 } else { 2 };
        let kind = if rng.chance(TOAST_ODDS) {
            Kind::Toast
        } else {
            Kind::Toaster { toast: rng.chance(0.4), flap: rng.f32() * TAU, rate: TAU / rng.rangef(10.0, 14.0) }
        };
        let s = self.size[layer];
        let (bx, by, bw, bh) = extent(&kind, s);
        let (w, ph) = (self.w as f32, self.ph as f32);
        let top = (w + bw) * DIR.1;
        let side = (ph + bh) * -DIR.0;
        let (mut x, mut y) = if rng.f32() * (top + side) < top {
            (rng.rangef(0.0, w + bw) - bx, -(by + bh) - 1.0)
        } else {
            (w + 1.0 - bx, rng.rangef(-bh, ph) - by)
        };
        let mut f = Flyer { x, y, layer, kind };
        for _ in 0..30 {
            if self.free(x, y, &f) {
                break;
            }
            x -= DIR.0 * s * 0.5;
            y -= DIR.1 * s * 0.5;
        }
        (f.x, f.y) = (x, y);
        f
    }
}

impl Animation for Toasters {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        let ph = self.ph as f32;
        let mut gone = Vec::new();
        for (i, f) in self.flyers.iter_mut().enumerate() {
            let s = self.size[f.layer];
            f.x += DIR.0 * s * SPEED;
            f.y += DIR.1 * s * SPEED;
            if let Kind::Toaster { flap, rate, .. } = &mut f.kind {
                *flap = (*flap + *rate) % TAU;
            }
            let (bx, by, bw, _) = extent(&f.kind, s);
            if f.x + bx + bw < 0.0 || f.y + by > ph {
                gone.push(i);
            }
        }
        for i in gone.into_iter().rev() {
            self.flyers.swap_remove(i);
            let f = self.spawn(rng);
            self.flyers.push(f);
        }
        self.flyers.sort_by_key(|f| f.layer);

        self.px.fill(BG);
        let toast = TOAST[self.level];
        for f in &self.flyers {
            let (s, dim) = (self.size[f.layer], LAYERS[f.layer].1);
            let (x, y) = (f.x.round() as i32, f.y.round() as i32);
            match f.kind {
                Kind::Toaster { toast: has, flap, .. } => {
                    draw_toaster(&mut self.px, self.w, self.ph, x, y, s, flap, has.then_some(toast), dim)
                }
                Kind::Toast => {
                    let (tw, th) = (((s * 0.72).round() as i32).max(3), ((s * 0.66).round() as i32).max(3));
                    let thick = ((s * 0.08).round() as i32).max(1);
                    draw_slice(&mut self.px, self.w, self.ph, x, y, tw, th, toast, i32::MAX, thick, dim)
                }
            }
        }
        c.blit_pixels(&self.px);
    }

    fn key(&mut self, key: crate::Key) {
        if key == crate::Key::Space {
            self.level = (self.level + 1) % TOAST.len();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_cycles_the_toast_and_wraps() {
        let mut t = new(80, 24, &mut Rng::new(1));
        let mut c = Canvas::new(80, 24);
        t.step(&mut c, &mut Rng::new(2));
        for _ in 0..TOAST.len() {
            t.key(crate::Key::Space);
        }
        // Back to where it started: the same frame as a fresh run.
        let mut u = new(80, 24, &mut Rng::new(1));
        let mut d = Canvas::new(80, 24);
        u.step(&mut d, &mut Rng::new(2));
        t.step(&mut c, &mut Rng::new(3));
        u.step(&mut d, &mut Rng::new(3));
        assert!((0..24).all(|y| (0..80).all(|x| c.get(x, y) == d.get(x, y))));
    }

    #[test]
    fn flyers_in_a_layer_never_overlap() {
        let mut rng = Rng::new(7);
        let mut t = Toasters { w: 240, ph: 140, px: vec![BG; 240 * 140], flyers: Vec::new(), size: [15.4, 21.0, 28.0], level: 1 };
        for _ in 0..30 {
            let f = t.spawn(&mut rng);
            assert!(t.free(f.x, f.y, &f));
            t.flyers.push(f);
        }
    }
}
