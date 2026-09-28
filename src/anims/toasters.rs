//! Flying toasters, after the After Dark screensaver of 1989: toasters
//! flapping feathered wings drift diagonally down across a black sky with
//! slices of toast and the odd breakfast guest: a winged mug of coffee, a jar
//! of jam, a fried egg and a slowly turning croissant.
//!
//! Everything is drawn procedurally in half-block pixels, in a fixed
//! three-quarter view like the original. A toaster's body is its side profile
//! (a rounded rectangle: boxy, a tall retro one or a long four-slot one)
//! extruded up and to the left: each pixel is traced back along the depth
//! direction until it meets the body, and the surface it meets (side, top,
//! leading end or a rounded edge) picks the shading. Bodies come in chrome or
//! in fifties enamel with chrome trim, some have a face that blinks, and the
//! ones loaded with bread glow in the slots until the lever snaps up and the
//! toast pops out to fly on its own.
//!
//! Three depth layers give parallax: far ones are smaller, slower and dimmer.
//! Flyers in a layer share its speed, so once spawned apart they never
//! collide, and each layer is drawn over the ones behind it. Space cycles how
//! toasted the toast is, from light to burnt.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;
use std::f32::consts::{PI, TAU};

pub const BG: Rgb = Rgb(0, 0, 0);

/// Direction of flight in pixels: down and to the left.
const DIR: (f32, f32) = (-0.876, 0.482);
/// Size of each depth layer relative to the nearest one, and its brightness.
const LAYERS: [(f32, f32); 3] = [(0.55, 0.5), (0.75, 0.72), (1.0, 1.0)];
/// How often each layer is picked for a new flyer.
const LAYER_ODDS: [f32; 3] = [0.34, 0.33, 0.33];
/// Share of new flyers that are breakfast guests and loose slices; the rest
/// are toasters.
const GUEST_ODDS: f32 = 0.125;
const TOAST_ODDS: f32 = 0.25;
/// Guests are drawn this much bigger than their flyer's size.
const GUEST: f32 = 1.45;
/// Pixels per frame, relative to the flyer's size.
const SPEED: f32 = 0.05;
/// Share of toasters that are chrome, have a face, or carry bread.
const CHROME_ODDS: f32 = 0.45;
const FACE_ODDS: f32 = 0.4;
const LOADED_ODDS: f32 = 0.5;

/// The chrome side, top to bottom: highlight, sky reflection, dark band,
/// ground reflection and shadow.
const CHROME: [Rgb; 6] = [
    Rgb(250, 251, 255),
    Rgb(196, 204, 218),
    Rgb(118, 126, 144),
    Rgb(208, 216, 230),
    Rgb(160, 168, 184),
    Rgb(96, 102, 118),
];
/// Fifties enamel: cherry, mint, cream, sky and butter.
const ENAMEL: [Rgb; 5] = [
    Rgb(212, 46, 58),
    Rgb(124, 204, 172),
    Rgb(236, 224, 194),
    Rgb(112, 170, 226),
    Rgb(244, 204, 92),
];
const TOP: Rgb = Rgb(214, 220, 232);
const SLOT: Rgb = Rgb(26, 26, 32);
const GLOW: Rgb = Rgb(255, 112, 32);
const BASE: Rgb = Rgb(58, 60, 72);
const LEVER: Rgb = Rgb(34, 34, 42);
const EYE: Rgb = Rgb(250, 250, 250);
const PUPIL: Rgb = Rgb(20, 20, 26);
const CHEEK: Rgb = Rgb(255, 130, 140);
const FEATHER: Rgb = Rgb(246, 246, 250);
const FEATHER_EDGE: Rgb = Rgb(146, 152, 170);
/// Feather lengths from the leading edge down, relative to the flyer's size.
const FEATHERS: [f32; 4] = [1.0, 0.9, 0.78, 0.64];
/// Toast face and crust for each doneness: light, golden, dark and burnt.
const TOAST: [(Rgb, Rgb); 4] = [
    (Rgb(240, 218, 166), Rgb(196, 150, 90)),
    (Rgb(222, 166, 84), Rgb(150, 96, 44)),
    (Rgb(160, 96, 44), Rgb(98, 56, 26)),
    (Rgb(78, 50, 32), Rgb(44, 28, 20)),
];
const MUG: Rgb = Rgb(240, 236, 228);
const MUG_BAND: Rgb = Rgb(206, 52, 60);
const COFFEE: Rgb = Rgb(84, 50, 30);
const CREMA: Rgb = Rgb(150, 100, 60);
const STEAM: Rgb = Rgb(190, 194, 206);
const JAM: Rgb = Rgb(168, 22, 50);
const LABEL: Rgb = Rgb(242, 230, 200);
const GINGHAM: [Rgb; 2] = [Rgb(222, 44, 56), Rgb(250, 244, 236)];
const EGG_WHITE: Rgb = Rgb(250, 250, 244);
const EGG_EDGE: Rgb = Rgb(226, 190, 128);
const YOLK: [Rgb; 3] = [Rgb(255, 238, 160), Rgb(255, 188, 36), Rgb(222, 140, 16)];
const PASTRY: [Rgb; 3] = [Rgb(250, 212, 130), Rgb(222, 156, 66), Rgb(150, 88, 36)];

/// A toaster's side profile, in units of the flyer's size: length, height,
/// how round its top corners are (share of the height) and the stretches of
/// its length the slots run along (two slots deep each).
struct Model {
    len: f32,
    hgt: f32,
    round: f32,
    slots: &'static [(f32, f32)],
}

const MODELS: [Model; 3] = [
    Model { len: 1.0, hgt: 0.62, round: 0.1, slots: &[(0.14, 0.86)] },
    Model { len: 0.84, hgt: 0.74, round: 0.38, slots: &[(0.26, 0.74)] },
    Model { len: 1.36, hgt: 0.58, round: 0.12, slots: &[(0.08, 0.47), (0.53, 0.92)] },
];
/// Depth of the slots across the top, near one first.
const SLOT_DEPTHS: [(f32, f32); 2] = [(0.22, 0.4), (0.6, 0.78)];

struct Toaster {
    model: usize,
    /// 0 is chrome, else `ENAMEL[finish - 1]`.
    finish: usize,
    face: bool,
    flap: f32,
    rate: f32,
    /// Bread in the slots, and frames until it pops.
    loaded: bool,
    timer: u32,
    /// Lever height: 1 pushed down, 0 up.
    lever: f32,
}

enum Kind {
    Toaster(Toaster),
    /// `vy` is the leftover of the jump out of a toaster.
    Toast { vy: f32 },
    Mug { flap: f32 },
    Jam { flap: f32 },
    Egg,
    Croissant { spin: f32 },
}

struct Flyer {
    /// Top-left corner of the body, in pixels.
    x: f32,
    y: f32,
    layer: usize,
    kind: Kind,
    /// Popped out of a toaster: not replaced when it leaves.
    extra: bool,
    /// Per-flyer offset for blinking, steam and wobble.
    phase: f32,
}

struct Toasters {
    w: usize,
    ph: usize,
    px: Vec<Rgb>,
    flyers: Vec<Flyer>,
    /// Size of a flyer in each layer.
    size: [f32; 3],
    level: usize,
    t: u32,
}

/// Area a flyer can cover, as (x, y, w, h) from its corner: a toaster's top
/// and leading end recede up and to the left, raised wings reach up and back,
/// bread stands out of the slots and steam rises from the coffee.
fn extent(kind: &Kind, s: f32) -> (f32, f32, f32, f32) {
    match kind {
        Kind::Toaster(t) => {
            let m = &MODELS[t.model];
            let right = 0.74 * m.len * s + 1.12 * s;
            let bottom = (m.hgt * s).max(0.75 * s) + 0.05 * s;
            (-0.3 * s, -1.2 * s, right + 0.3 * s, bottom + 1.2 * s)
        }
        Kind::Toast { .. } => (-0.1 * s, -0.1 * s, 0.82 * s, 0.76 * s),
        Kind::Mug { .. } | Kind::Jam { .. } => (-0.05 * GUEST * s, -0.7 * GUEST * s, 1.1 * GUEST * s, 1.35 * GUEST * s),
        Kind::Egg => (0.0, -0.05 * GUEST * s, 0.82 * GUEST * s, 0.52 * GUEST * s),
        Kind::Croissant { .. } => (0.0, 0.0, GUEST * s, GUEST * s),
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
    let mut depth = Vec::with_capacity(caps.len());
    for y in y0.floor() as i32..=y1.ceil() as i32 {
        for x in x0.floor() as i32..=x1.ceil() as i32 {
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            // Depth of the pixel inside each capsule (negative outside).
            depth.clear();
            depth.extend(caps.iter().map(|&(p, q, r)| {
                let (dx, dy, qx, qy) = (q.0 - p.0, q.1 - p.1, cx - p.0, cy - p.1);
                let t = ((qx * dx + qy * dy) / (dx * dx + dy * dy).max(1e-6)).clamp(0.0, 1.0);
                r - ((qx - t * dx).powi(2) + (qy - t * dy).powi(2)).sqrt()
            }));
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

/// Angle of the leading feather at a point of the wingbeat.
fn wing_angle(flap: f32) -> f32 {
    0.55 + 0.9 * flap.sin()
}

/// The side profile of a body: a `len x hgt` box with top corners rounded
/// by `rt` and bottom ones by `rb`, in pixels.
struct Section {
    len: f32,
    hgt: f32,
    rt: f32,
    rb: f32,
}

impl Section {
    /// The center of the corner arc nearest to (x, y), with its radius.
    fn center(&self, x: f32, y: f32) -> (f32, f32, f32) {
        let top = y < self.hgt * 0.5;
        let r = if top { self.rt } else { self.rb };
        let cy = if top { y.max(r) } else { y.min(self.hgt - r) };
        (x.clamp(r, self.len - r), cy, r)
    }

    fn contains(&self, x: f32, y: f32) -> bool {
        if x < 0.0 || y < 0.0 || x > self.len || y > self.hgt {
            return false;
        }
        let (cx, cy, r) = self.center(x, y);
        (x - cx).powi(2) + (y - cy).powi(2) <= r * r
    }

    /// Outward normal at a point on the outline.
    fn normal(&self, x: f32, y: f32) -> (f32, f32) {
        let (cx, cy, _) = self.center(x, y);
        let (dx, dy) = (x - cx, y - cy);
        let d = (dx * dx + dy * dy).sqrt();
        if d > 1e-3 {
            (dx / d, dy / d)
        } else if y < x {
            (0.0, -1.0)
        } else {
            (-1.0, 0.0)
        }
    }
}

/// Color of a toaster's side at height `v` (0 top, 1 bottom of the side).
fn side_color(finish: usize, v: f32) -> Rgb {
    if finish == 0 {
        return Rgb::gradient(&CHROME, v);
    }
    let c = ENAMEL[finish - 1];
    if (0.56..0.66).contains(&v) {
        // The chrome trim along the middle.
        return Rgb::gradient(&CHROME, 0.1 + (v - 0.56) * 3.0);
    }
    Rgb::gradient(&[c.lerp(Rgb::WHITE, 0.4), c, c.scale(0.9), c.scale(0.72)], v)
}

/// A toaster in three-quarter view, flying left, with its side's corner at
/// (ox, oy). Drawn back to front: far wing, body, the slices standing in the
/// slots, near wing.
fn draw_toaster(px: &mut [Rgb], w: usize, ph: usize, ox: i32, oy: i32, s: f32, t: &Toaster, toast: (Rgb, Rgb), phase: f32, time: u32, dim: f32) {
    let m = &MODELS[t.model];
    let len = ((m.len * s).round() as i32).max(4);
    let hgt = ((m.hgt * s).round() as i32).max(3);
    let (lf, hf) = (len as f32, hgt as f32);
    // Depth offset of the far side.
    let (ddx, ddy) = (((s * 0.28).round() as i32).max(1), ((s * 0.25).round() as i32).max(1));
    let (dfx, dfy) = (ddx as f32, ddy as f32);
    let a = wing_angle(t.flap);
    let pivot = (ox as f32 + 0.74 * lf, oy as f32 + 0.15 * hf);
    draw_wing(px, w, ph, (pivot.0 - dfx, pivot.1 - dfy), s, a, dim * 0.62);

    let sec = Section { len: lf, hgt: hf, rt: (m.round * hf).max(0.6), rb: (0.06 * hf).max(0.6) };
    let base = ((hf * 0.12).round() as i32).max(1);
    let glow = SLOT.lerp(GLOW, 0.45 + 0.25 * (time as f32 * 0.3 + phase).sin());
    let lever_x = (lf * 0.86).round() as i32;
    let (lever_top, lever_bot) = ((hf * 0.28).round() as i32, (hf * 0.72).round() as i32 - base);
    let knob = lever_top + ((lever_bot - lever_top) as f32 * t.lever).round() as i32;
    let shine = (lf * 0.1).round() as i32;
    let steps = 2 * ddx.max(ddy);
    for j in -ddy..hgt {
        for i in -ddx..len {
            let (x, y) = (i as f32 + 0.5, j as f32 + 0.5);
            let c = if sec.contains(x, y) {
                if j >= hgt - base {
                    BASE
                } else if let Some(c) = side_detail(i, j, len, hgt, t, time, phase) {
                    c
                } else if len >= 8 && i == lever_x && (lever_top..=lever_bot).contains(&j) {
                    if j == knob { LEVER } else { LEVER.lerp(side_color(t.finish, 0.5), 0.5) }
                } else if len >= 8 && i == lever_x + 1 && j == knob {
                    LEVER
                } else {
                    let v = y / (hf - base as f32);
                    let u = x / lf;
                    let c = side_color(t.finish, v).scale(1.0 - 0.3 * (2.0 * u - 1.0).abs().powi(4));
                    if len >= 10 && i == shine { c.lerp(Rgb::WHITE, 0.4) } else { c }
                }
            } else {
                // Trace back along the depth direction to the first point of
                // the body behind this pixel, then refine it.
                let at = |b: f32| (x + b * dfx, y + b * dfy);
                let Some(k) = (1..=steps).find(|&k| {
                    let (qx, qy) = at(k as f32 / steps as f32);
                    sec.contains(qx, qy)
                }) else {
                    continue;
                };
                let (mut lo, mut hi) = ((k - 1) as f32 / steps as f32, k as f32 / steps as f32);
                for _ in 0..4 {
                    let mid = 0.5 * (lo + hi);
                    let (qx, qy) = at(mid);
                    if sec.contains(qx, qy) { hi = mid } else { lo = mid }
                }
                let b = hi;
                let (qx, qy) = at(b);
                let n = sec.normal(qx, qy);
                let up = (-n.1).clamp(0.0, 1.0);
                if up > 0.97 {
                    // The flat top, with its slots.
                    let u = qx / lf;
                    let slot = SLOT_DEPTHS.iter().any(|d| (d.0..d.1).contains(&b)) && m.slots.iter().any(|r| (r.0..r.1).contains(&u));
                    if slot {
                        if t.loaded { glow } else { SLOT }
                    } else {
                        TOP.lerp(CHROME[2], b * 0.35)
                    }
                } else {
                    let v = (qy / (hf - base as f32)).min(1.0);
                    let end = if qy >= hf - base as f32 { BASE.scale(0.8) } else { side_color(t.finish, v).scale(0.62) };
                    let top = TOP.lerp(CHROME[2], b * 0.35);
                    let sheen = (1.0 - (up - 0.6).abs() / 0.4).max(0.0) * 0.35;
                    end.lerp(top, up.powf(1.5)).lerp(Rgb::WHITE, sheen)
                }
            };
            put(px, w, ph, ox + i, oy + j, c, dim);
        }
    }

    if t.loaded {
        // One slice per slot, far ones first; each stands in its slot's plane
        // and the top hides what is below the slot.
        let th = ((hf * 0.75).round() as i32).max(2);
        let rise = ((hf * 0.38).round() as i32).max(1);
        for (d0, d1) in SLOT_DEPTHS.iter().rev() {
            let b = 0.5 * (d0 + d1);
            let (sx, sy) = (ox - (b * dfx).round() as i32, oy - (b * dfy).round() as i32);
            for (u0, u1) in m.slots {
                let x0 = sx + ((u0 + 0.05) * lf).round() as i32;
                let tw = (((u1 - u0 - 0.1) * lf).round() as i32).max(2);
                draw_slice(px, w, ph, x0, sy - rise, tw, th, toast, sy, 0, dim);
            }
        }
    }
    draw_wing(px, w, ph, pivot, s, a, dim);
}

/// The face on a toaster's side, near its leading end: eyes looking ahead
/// (they blink now and then), a smile and, on enamel, rosy cheeks.
fn side_detail(i: i32, j: i32, len: i32, hgt: i32, t: &Toaster, time: u32, phase: f32) -> Option<Rgb> {
    if !t.face || len < 8 {
        return None;
    }
    let e = ((hgt as f32 * 0.16).round() as i32).max(1);
    let ey = (hgt as f32 * 0.3).round() as i32;
    let blink = (time + (phase * 40.0) as u32) % 110 < 3;
    for ex in [(len as f32 * 0.16).round() as i32, (len as f32 * 0.34).round() as i32] {
        if (ex..ex + e).contains(&i) && (ey..ey + e).contains(&j) {
            if blink {
                return (j == ey + e / 2).then_some(PUPIL);
            }
            // White with the pupil in the corner that looks ahead.
            return Some(if e == 1 || (i == ex && j >= ey + e / 2) { PUPIL } else { EYE });
        }
    }
    let my = ey + e + (hgt as f32 * 0.14).round() as i32;
    let (m0, m1) = ((len as f32 * 0.2).round() as i32, (len as f32 * 0.34).round() as i32 + e - 1);
    if (m0..=m1).contains(&i) && (j == my && i > m0 && i < m1 || j == my - 1 && (i == m0 || i == m1)) {
        return Some(PUPIL);
    }
    if t.finish != 0 && e >= 2 && j == my - 1 && (i == m0 - 2 || i == m1 + 2) {
        return Some(CHEEK);
    }
    None
}

/// A winged mug of coffee in three-quarter view: a shaded cylinder with a
/// red band, coffee under the rim, a handle at the back and two wisps of
/// steam trailing up and back.
fn draw_mug(px: &mut [Rgb], w: usize, ph: usize, ox: i32, oy: i32, s: f32, flap: f32, phase: f32, time: u32, dim: f32) {
    let mw = ((s * 0.46).round() as i32).max(3);
    let mh = ((s * 0.5).round() as i32).max(3);
    let (mwf, mhf) = (mw as f32, mh as f32);
    let ery = (mwf * 0.2).max(0.8);
    let a = wing_angle(flap);
    let pivot = (ox as f32 + mwf * 0.9, oy as f32 + mhf * 0.3);
    draw_wing(px, w, ph, (pivot.0 - 0.12 * s, pivot.1 - 0.1 * s), s * 0.45, a, dim * 0.62);

    // Steam: two wisps that rise, sway and fade, drifting back as it flies.
    let n = ((s * 0.6).round() as i32).max(2);
    for (k, base) in [0.35, 0.68].iter().enumerate() {
        for step in 1..=n {
            let f = step as f32 / n as f32;
            let sway = (step as f32 * 0.55 - time as f32 * 0.22 + phase + k as f32 * 2.0).sin() * (0.4 + f * mwf * 0.12);
            let x = ox as f32 + base * mwf + sway + f * mwf * 0.5;
            put(px, w, ph, x.floor() as i32, oy - step, STEAM.scale(0.75 * (1.0 - f)), dim);
        }
    }

    // Handle: a ring on the back, only the part outside the cup.
    let (hx, hy) = (ox as f32 + mwf - 0.3, oy as f32 + mhf * 0.45);
    let (ro, ri) = (mhf * 0.32, (mhf * 0.32 - (s * 0.07).max(1.0)).max(0.0));
    for y in (hy - ro).floor() as i32..=(hy + ro).ceil() as i32 {
        for x in ox + mw - 1..=(hx + ro).ceil() as i32 {
            let d = ((x as f32 + 0.5 - hx).powi(2) + (y as f32 + 0.5 - hy).powi(2)).sqrt();
            if d <= ro && d >= ri {
                put(px, w, ph, x, y, MUG.scale(0.78), dim);
            }
        }
    }

    for i in 0..mw {
        let ex = (i as f32 + 0.5) / mwf * 2.0 - 1.0;
        let arc = ery * (1.0 - ex * ex).max(0.0).sqrt();
        // Lit from the front left.
        let lum = 0.55 + 0.45 * ((ex + 0.35) * PI * 0.5).cos().max(0.0);
        for j in (-ery).floor() as i32..=(mhf + ery).ceil() as i32 {
            let y = j as f32 + 0.5;
            let c = if y >= -arc && y <= arc {
                // Inside the rim: coffee with a ring of crema, then the lip.
                let r = (ex * ex + (y / ery).powi(2)).sqrt();
                if r < 0.62 { COFFEE } else if r < 0.8 { CREMA } else { MUG }
            } else if y > arc && y <= mhf + arc {
                let band = (y - arc - mhf * 0.28).abs() < (mhf * 0.08).max(0.5);
                (if band { MUG_BAND } else { MUG }).scale(lum)
            } else {
                continue;
            };
            put(px, w, ph, ox + i, oy + j, c, dim);
        }
    }
    draw_wing(px, w, ph, pivot, s * 0.45, a, dim);
}

/// A winged jar of strawberry jam: glass full of jam with a glint, a paper
/// label and a gingham cloth tied over the lid.
fn draw_jam(px: &mut [Rgb], w: usize, ph: usize, ox: i32, oy: i32, s: f32, flap: f32, dim: f32) {
    let jw = ((s * 0.44).round() as i32).max(3);
    let jh = ((s * 0.46).round() as i32).max(3);
    let a = wing_angle(flap);
    let pivot = (ox as f32 + jw as f32 * 0.9, oy as f32 + jh as f32 * 0.3);
    draw_wing(px, w, ph, (pivot.0 - 0.12 * s, pivot.1 - 0.1 * s), s * 0.45, a, dim * 0.62);
    let r = (jw / 4).max(1);
    for j in 0..jh {
        for i in 0..jw {
            if !rounded(i, j, jw, jh, 1, r) {
                continue;
            }
            let (u, v) = ((i as f32 + 0.5) / jw as f32, (j as f32 + 0.5) / jh as f32);
            let c = if (0.14..0.86).contains(&u) && (0.3..0.72).contains(&v) {
                // A strawberry dot in the middle of the label.
                if (u - 0.5).abs() < 0.12 && (v - 0.51).abs() < 0.1 { MUG_BAND } else { LABEL }
            } else if (0.12..0.24).contains(&u) {
                JAM.lerp(Rgb::WHITE, 0.45)
            } else {
                JAM.scale(0.75 + 0.35 * (u * PI).sin())
            };
            put(px, w, ph, ox + i, oy + j, c, dim);
        }
    }
    // The cloth: a gingham band a little wider than the jar, and a darker
    // string where it is tied.
    let ch = ((s * 0.16).round() as i32).max(1);
    let cell = ((s * 0.06).round() as i32).max(1);
    for j in -ch..1 {
        for i in -1..=jw {
            let c = if j == 0 {
                GINGHAM[0].scale(0.6)
            } else {
                let check = ((i + 1) / cell + (j + ch) / cell) % 2;
                GINGHAM[check as usize]
            };
            put(px, w, ph, ox + i, oy + j, c, dim);
        }
    }
    draw_wing(px, w, ph, pivot, s * 0.45, a, dim);
}

/// A fried egg gliding flat: a wobbly white with a crisp golden rim, a
/// little thickness underneath and a glossy yolk.
fn draw_egg(px: &mut [Rgb], w: usize, ph: usize, ox: i32, oy: i32, s: f32, phase: f32, dim: f32) {
    let (cx, cy) = (0.41 * s, 0.24 * s);
    let (rx, ry) = ((0.39 * s).max(1.5), (0.19 * s).max(1.0));
    let edge = |x: f32, y: f32| {
        let (dx, dy) = ((x - cx) / rx, (y - cy) / ry);
        let th = dy.atan2(dx);
        let r = 0.86 + 0.09 * (3.0 * th + phase).sin() + 0.05 * (5.0 * th - phase * 1.3).sin();
        (dx * dx + dy * dy).sqrt() / r
    };
    for j in -1..=(0.5 * s).ceil() as i32 {
        for i in 0..=(0.82 * s).ceil() as i32 {
            let (x, y) = (i as f32 + 0.5, j as f32 + 0.5);
            let (d, below) = (edge(x, y), edge(x, y - 1.0));
            let c = if d <= 1.0 {
                if d > 1.0 - 1.2 / rx.min(ry * 2.0) { EGG_EDGE } else { EGG_WHITE.scale(1.0 - 0.1 * ((y - cy) / ry).max(0.0)) }
            } else if below <= 1.0 {
                EGG_EDGE.scale(0.7)
            } else {
                continue;
            };
            put(px, w, ph, ox + i, oy + j, c, dim);
        }
    }
    let (yx, yy, yr) = (0.37 * s, 0.19 * s, (0.12 * s).max(0.8));
    for j in (yy - yr).floor() as i32..=(yy + yr).ceil() as i32 {
        for i in (yx - yr).floor() as i32..=(yx + yr).ceil() as i32 {
            let (dx, dy) = ((i as f32 + 0.5 - yx) / yr, (j as f32 + 0.5 - yy) / yr);
            let d = (dx * dx + dy * dy).sqrt();
            if d > 1.0 {
                continue;
            }
            let c = if yr >= 2.0 && (dx + 0.35).powi(2) + (dy + 0.35).powi(2) < 0.12 {
                YOLK[0]
            } else if d > 0.7 && dy > 0.0 {
                YOLK[2]
            } else {
                YOLK[1]
            };
            put(px, w, ph, ox + i, oy + j, c, dim);
        }
    }
}

/// A croissant turning slowly: five lobes on a crescent, the middle one
/// biggest and on top, each shaded as a little bun lit from the top left.
fn draw_croissant(px: &mut [Rgb], w: usize, ph: usize, ox: i32, oy: i32, s: f32, spin: f32, dim: f32) {
    let (cx, cy) = (ox as f32 + 0.5 * s, oy as f32 + 0.5 * s);
    let arc = 0.3 * s;
    let sizes = [0.09, 0.14, 0.19, 0.14, 0.09];
    for &i in &[0usize, 4, 1, 3, 2] {
        let ang = spin + (i as f32 - 2.0) * 0.72;
        let (lx, ly) = (cx + ang.cos() * arc, cy + ang.sin() * arc);
        let r = (sizes[i] * s).max(0.8);
        for y in (ly - r).floor() as i32..=(ly + r).ceil() as i32 {
            for x in (lx - r).floor() as i32..=(lx + r).ceil() as i32 {
                let (dx, dy) = ((x as f32 + 0.5 - lx) / r, (y as f32 + 0.5 - ly) / r);
                let d = (dx * dx + dy * dy).sqrt();
                if d > 1.0 {
                    continue;
                }
                let c = if r >= 1.5 && d > 1.0 - 1.0 / r {
                    PASTRY[2]
                } else {
                    let lit = (-(dx + dy) * 0.7 + 0.3).clamp(0.0, 1.0);
                    PASTRY[1].lerp(PASTRY[0], lit)
                };
                put(px, w, ph, x, y, c, dim);
            }
        }
    }
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
        t: 0,
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
        let roll = rng.f32();
        let flap = rng.f32() * TAU;
        let kind = if roll < GUEST_ODDS {
            match rng.below(4) {
                0 => Kind::Mug { flap },
                1 => Kind::Jam { flap },
                2 => Kind::Egg,
                _ => Kind::Croissant { spin: flap },
            }
        } else if roll < GUEST_ODDS + TOAST_ODDS {
            Kind::Toast { vy: 0.0 }
        } else {
            let loaded = rng.chance(LOADED_ODDS);
            Kind::Toaster(Toaster {
                model: rng.below(MODELS.len()),
                finish: if rng.chance(CHROME_ODDS) { 0 } else { 1 + rng.below(ENAMEL.len()) },
                face: rng.chance(FACE_ODDS),
                flap,
                rate: TAU / rng.rangef(10.0, 14.0),
                loaded,
                timer: rng.range(60, 500) as u32,
                lever: if loaded { 1.0 } else { 0.0 },
            })
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
        let mut f = Flyer { x, y, layer, kind, extra: false, phase: rng.f32() * TAU };
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
        self.t = self.t.wrapping_add(1);
        let (w, ph) = (self.w as f32, self.ph as f32);
        let mut gone = Vec::new();
        let mut popped = Vec::new();
        for (i, f) in self.flyers.iter_mut().enumerate() {
            let s = self.size[f.layer];
            f.x += DIR.0 * s * SPEED;
            f.y += DIR.1 * s * SPEED;
            match &mut f.kind {
                Kind::Toaster(t) => {
                    t.flap = (t.flap + t.rate) % TAU;
                    t.timer = t.timer.saturating_sub(1);
                    if !t.loaded {
                        t.lever = (t.lever - 0.25).max(0.0);
                    } else if t.timer == 0 && f.x > 0.0 && f.y > 0.0 && f.x + s < w && f.y < ph {
                        // Done: the lever snaps up and the slices jump out.
                        t.loaded = false;
                        let m = &MODELS[t.model];
                        for (u0, u1) in m.slots {
                            let mid = 0.5 * (u0 + u1) * m.len * s;
                            for b in [0.69, 0.31] {
                                let (x, y) = (f.x + mid - 0.36 * s - b * 0.28 * s, f.y - 0.5 * s - b * 0.25 * s);
                                popped.push(Flyer { x, y, layer: f.layer, kind: Kind::Toast { vy: -0.09 * s }, extra: true, phase: 0.0 });
                            }
                        }
                    }
                }
                Kind::Toast { vy } => {
                    f.y += *vy;
                    *vy = (*vy + 0.006 * s).min(0.0);
                }
                Kind::Mug { flap } | Kind::Jam { flap } => *flap = (*flap + TAU / 12.0) % TAU,
                Kind::Croissant { spin } => *spin = (*spin + 0.03) % TAU,
                Kind::Egg => {}
            }
            let (bx, by, bw, _) = extent(&f.kind, s);
            if f.x + bx + bw < 0.0 || f.y + by > ph {
                gone.push(i);
            }
        }
        for i in gone.into_iter().rev() {
            let f = self.flyers.swap_remove(i);
            if !f.extra {
                let f = self.spawn(rng);
                self.flyers.push(f);
            }
        }
        self.flyers.extend(popped);
        self.flyers.sort_by_key(|f| f.layer);

        self.px.fill(BG);
        let toast = TOAST[self.level];
        let (pw, pph, time) = (self.w, self.ph, self.t);
        for f in &self.flyers {
            let (s, dim) = (self.size[f.layer], LAYERS[f.layer].1);
            let (x, y) = (f.x.round() as i32, f.y.round() as i32);
            let px = &mut self.px;
            match &f.kind {
                Kind::Toaster(t) => draw_toaster(px, pw, pph, x, y, s, t, toast, f.phase, time, dim),
                Kind::Toast { .. } => {
                    let (tw, th) = (((s * 0.72).round() as i32).max(3), ((s * 0.66).round() as i32).max(3));
                    let thick = ((s * 0.08).round() as i32).max(1);
                    draw_slice(px, pw, pph, x, y, tw, th, toast, i32::MAX, thick, dim)
                }
                Kind::Mug { flap } => draw_mug(px, pw, pph, x, y, s * GUEST, *flap, f.phase, time, dim),
                Kind::Jam { flap } => draw_jam(px, pw, pph, x, y, s * GUEST, *flap, dim),
                Kind::Egg => draw_egg(px, pw, pph, x, y, s * GUEST, f.phase, dim),
                Kind::Croissant { spin } => draw_croissant(px, pw, pph, x, y, s * GUEST, *spin, dim),
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

    fn field(seed: u64) -> Toasters {
        Toasters { w: 240, ph: 140, px: vec![BG; 240 * 140], flyers: Vec::new(), size: [9.9, 13.5, 18.0], level: 1, t: seed as u32 }
    }

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
        let mut t = field(0);
        for _ in 0..30 {
            let f = t.spawn(&mut rng);
            assert!(t.free(f.x, f.y, &f));
            t.flyers.push(f);
        }
    }

    #[test]
    fn popped_toast_leaves_without_being_replaced() {
        let mut rng = Rng::new(5);
        let mut t = field(0);
        for _ in 0..20 {
            let f = t.spawn(&mut rng);
            t.flyers.push(f);
        }
        let mut c = Canvas::new(240, 70);
        let mut popped = false;
        for _ in 0..4000 {
            t.step(&mut c, &mut rng);
            popped |= t.flyers.iter().any(|f| f.extra);
            assert_eq!(t.flyers.iter().filter(|f| !f.extra).count(), 20);
        }
        assert!(popped, "no toaster ever popped its toast");
    }
}
