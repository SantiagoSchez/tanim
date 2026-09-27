//! Flying down an endless tiled tunnel that bends and rolls.
//!
//! Seen from inside, a straight tube maps every screen pixel to a point on its
//! wall: the pixel's angle around the centre is the angle around the tube, and
//! its depth along the tube is proportional to one over its distance from the
//! centre. Both only depend on where the pixel is, so they live in tables built
//! once. A frame then costs one lookup per pixel: scroll the depth to fly, add
//! to the angle to roll, and slide the screen over a table twice its size to
//! make the tunnel bend.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;
use std::f32::consts::TAU;

/// Texture size along both axes; angles and depths wrap at this.
const TEX: usize = 256;
/// Frames between texture/palette changes.
const THEME_LEN: u32 = 900;
const FADE: u32 = 45;

#[derive(Clone, Copy)]
enum Pattern {
    /// Tiles with dark grout.
    Tiles,
    /// Glowing grid lines on black.
    Grid,
    /// Diagonal bands that spiral down the tube.
    Spiral,
    /// Rings of alternating color.
    Rings,
}

const PATTERNS: [Pattern; 4] = [Pattern::Tiles, Pattern::Grid, Pattern::Spiral, Pattern::Rings];

const PALETTES: [[Rgb; 4]; 5] = [
    [Rgb(20, 230, 255), Rgb(255, 40, 200), Rgb(120, 60, 255), Rgb(30, 90, 200)],
    [Rgb(255, 70, 20), Rgb(255, 170, 30), Rgb(200, 30, 40), Rgb(255, 230, 120)],
    [Rgb(40, 220, 120), Rgb(20, 120, 90), Rgb(160, 255, 90), Rgb(10, 70, 60)],
    [Rgb(240, 240, 250), Rgb(120, 130, 160), Rgb(60, 70, 100), Rgb(190, 200, 230)],
    [Rgb(255, 90, 90), Rgb(255, 220, 80), Rgb(80, 220, 140), Rgb(90, 140, 255)],
];

struct Tunnel {
    w: usize,
    ph: usize,
    /// Table size: the screen is a movable window into it.
    tw: usize,
    th: usize,
    angle: Vec<u8>,
    depth: Vec<u16>,
    /// Brightness from distance to the vanishing point.
    fog: Vec<u8>,
    /// Texture cells: palette index in the low bits, 4 means grout/background.
    tex: Vec<u8>,
    px: Vec<Rgb>,
    t: f32,
    frame: u32,
    pattern: usize,
    palette: usize,
    /// Random phases for the bends and roll.
    seeds: [f32; 4],
}

impl Tunnel {
    fn new(w: usize, h: usize, rng: &mut Rng) -> Tunnel {
        let ph = 2 * h;
        let (tw, th) = (2 * w.max(1), 2 * ph.max(1));
        let (cx, cy) = (tw as f32 / 2.0, th as f32 / 2.0);
        // One texture repeat along the depth per this many pixels of radius.
        let k = (ph.max(w / 2).max(1) as f32) * 24.0;
        let reach = (ph.max(w / 2).max(1) as f32) * 0.5;
        let mut angle = vec![0; tw * th];
        let mut depth = vec![0; tw * th];
        let mut fog = vec![0; tw * th];
        for y in 0..th {
            for x in 0..tw {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let r = (dx * dx + dy * dy).sqrt().max(0.5);
                let i = y * tw + x;
                // Two texture repeats around the tube.
                angle[i] = ((dy.atan2(dx) / TAU + 0.5) * 2.0 * TEX as f32) as u32 as u8;
                depth[i] = (k / r) as u32 as u16;
                fog[i] = ((r / reach).min(1.0).powf(1.4) * 255.0) as u8;
            }
        }
        let mut t = Tunnel {
            w,
            ph,
            tw,
            th,
            angle,
            depth,
            fog,
            tex: vec![0; TEX * TEX],
            px: vec![Rgb::BLACK; w * ph],
            t: 0.0,
            frame: 0,
            pattern: rng.below(PATTERNS.len()),
            palette: rng.below(PALETTES.len()),
            seeds: [rng.rangef(0.0, TAU), rng.rangef(0.0, TAU), rng.rangef(0.0, TAU), rng.rangef(0.0, TAU)],
        };
        t.build_texture();
        t
    }

    fn build_texture(&mut self) {
        let pattern = PATTERNS[self.pattern];
        for v in 0..TEX {
            for u in 0..TEX {
                let (tu, tv) = (u / 16, v / 32);
                let (fu, fv) = (u % 16, v % 32);
                let cell = match pattern {
                    Pattern::Tiles => {
                        if fu == 0 || fv < 3 {
                            4
                        } else {
                            ((tu + tv * 3) % 4) as u8
                        }
                    }
                    Pattern::Grid => {
                        if fu == 0 || fv < 2 {
                            ((tu / 2 + tv) % 4) as u8
                        } else {
                            4
                        }
                    }
                    Pattern::Spiral => {
                        let band = (u + v / 2) / 16;
                        if (u + v / 2) % 16 < 2 {
                            4
                        } else {
                            (band % 4) as u8
                        }
                    }
                    Pattern::Rings => {
                        if fv < 4 {
                            4
                        } else if fu < 8 {
                            (tv % 4) as u8
                        } else {
                            ((tv + 2) % 4) as u8
                        }
                    }
                };
                self.tex[v * TEX + u] = cell;
            }
        }
    }
}

impl Animation for Tunnel {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        self.frame += 1;
        self.t += 1.0;
        let t = self.t;
        let [s0, s1, s2, s3] = self.seeds;

        // Change pattern and palette now and then, through black.
        let phase = self.frame % THEME_LEN;
        if phase == 0 {
            self.pattern = (self.pattern + 1 + rng.below(PATTERNS.len() - 1)) % PATTERNS.len();
            self.palette = (self.palette + 1 + rng.below(PALETTES.len() - 1)) % PALETTES.len();
            self.build_texture();
        }
        let fade = if phase < FADE {
            phase as f32 / FADE as f32
        } else if phase > THEME_LEN - FADE {
            (THEME_LEN - phase) as f32 / FADE as f32
        } else {
            1.0
        };

        // Speed surges, the roll sways, the vanishing point wanders.
        let speed = 2.2 + 1.2 * (t * 0.011 + s0).sin();
        let fly = (t * speed) as u32;
        let roll = ((t * 0.004 + s1).sin() * 90.0 + t * 0.25) as i32;
        let (w, ph, tw, th) = (self.w as f32, self.ph as f32, self.tw, self.th);
        // Keep the vanishing point in the middle half of the screen.
        let ox = ((tw as f32 - w) / 2.0 + w * 0.25 * (t * 0.013 + s2).sin()).max(0.0) as usize;
        let oy = ((th as f32 - ph) / 2.0 + ph * 0.22 * (t * 0.017 + s3).sin()).max(0.0) as usize;
        let ox = ox.min(tw - self.w.min(tw));
        let oy = oy.min(th - self.ph.min(th));

        // Palette slowly rotates through its colors; grout stays dark.
        let base = PALETTES[self.palette];
        let shift = t * 0.004;
        let mut pal = [Rgb::BLACK; 5];
        for (i, p) in pal.iter_mut().take(4).enumerate() {
            let f = (shift + i as f32 * 0.25).fract() * 4.0;
            let (a, b) = (f as usize % 4, (f as usize + 1) % 4);
            *p = base[a].lerp(base[b], f.fract()).scale(fade);
        }
        pal[4] = base[3].scale(0.12 * fade);

        for y in 0..self.ph {
            let row = (y + oy) * tw + ox;
            for x in 0..self.w {
                let i = row + x;
                let u = (self.angle[i] as i32 + roll) as u32 as usize % TEX;
                let v = (self.depth[i] as u32).wrapping_add(fly) as usize % TEX;
                let col = pal[self.tex[v * TEX + u] as usize];
                let f = self.fog[i] as u32;
                self.px[y * self.w + x] =
                    Rgb((col.0 as u32 * f / 255) as u8, (col.1 as u32 * f / 255) as u8, (col.2 as u32 * f / 255) as u8);
            }
        }
        c.blit_pixels(&self.px);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    Box::new(Tunnel::new(w, h, rng))
}
