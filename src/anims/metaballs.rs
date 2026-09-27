//! Glossy blobs that bulge towards each other and merge like drops of gel.
//!
//! Every ball adds a field r²/d² that falls off with distance; wherever the
//! sum passes one is inside the surface, so two balls that approach grow a
//! bridge and fuse. The same sum, read as a height, gives each pixel a normal
//! (from the field's gradient, which is exact and cheap to add up alongside
//! it), so the blobs are lit like solid drops with a specular highlight. Each
//! ball carries a color, and a pixel takes the blend weighted by how much each
//! ball contributes there, so merging drops mix their colors at the seam.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;

const THEMES: [[Rgb; 4]; 5] = [
    [Rgb(255, 60, 140), Rgb(255, 170, 40), Rgb(90, 210, 255), Rgb(170, 90, 255)],
    [Rgb(60, 240, 180), Rgb(30, 150, 255), Rgb(160, 255, 120), Rgb(20, 200, 220)],
    [Rgb(255, 90, 40), Rgb(255, 200, 60), Rgb(230, 40, 60), Rgb(255, 130, 90)],
    [Rgb(210, 215, 230), Rgb(150, 170, 200), Rgb(240, 200, 150), Rgb(180, 190, 210)],
    [Rgb(250, 120, 200), Rgb(120, 110, 250), Rgb(90, 230, 240), Rgb(250, 240, 130)],
];
const BG_TOP: Rgb = Rgb(10, 6, 24);
const BG_BOT: Rgb = Rgb(24, 10, 36);
/// Frames each theme lasts, and how long the colors blend into the next.
const THEME_LEN: f32 = 800.0;
const BLEND: f32 = 120.0;

struct Ball {
    /// Lissajous frequencies and phases for x and y.
    fx: f32,
    fy: f32,
    px: f32,
    py: f32,
    /// Radius as a fraction of the screen's smaller side.
    r: f32,
    slot: usize,
}

struct Metaballs {
    w: usize,
    ph: usize,
    balls: Vec<Ball>,
    px: Vec<Rgb>,
    t: f32,
    theme: usize,
    next: usize,
}

impl Metaballs {
    fn new(w: usize, h: usize, rng: &mut Rng) -> Metaballs {
        let n = rng.range(5, 9) as usize;
        let balls = (0..n)
            .map(|i| Ball {
                fx: rng.rangef(0.004, 0.011),
                fy: rng.rangef(0.005, 0.013),
                px: rng.rangef(0.0, 6.3),
                py: rng.rangef(0.0, 6.3),
                r: rng.rangef(0.09, 0.17),
                slot: i % 4,
            })
            .collect();
        let theme = rng.below(THEMES.len());
        Metaballs {
            w,
            ph: 2 * h,
            balls,
            px: vec![Rgb::BLACK; w * 2 * h],
            t: rng.rangef(0.0, 5000.0),
            theme,
            next: (theme + 1 + rng.below(THEMES.len() - 1)) % THEMES.len(),
        }
    }
}

impl Animation for Metaballs {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        self.t += 1.0;
        let (w, ph) = (self.w as f32, self.ph as f32);
        let side = w.min(ph).max(1.0);

        // Crossfade ball colors between themes.
        let into = self.t % THEME_LEN;
        if into < 1.0 {
            self.theme = self.next;
            self.next = (self.theme + 1 + rng.below(THEMES.len() - 1)) % THEMES.len();
        }
        let blend = ((into - (THEME_LEN - BLEND)) / BLEND).clamp(0.0, 1.0);

        struct B {
            x: f32,
            y: f32,
            r2: f32,
            col: [f32; 3],
        }
        let balls: Vec<B> = self
            .balls
            .iter()
            .map(|b| {
                let col = THEMES[self.theme][b.slot].lerp(THEMES[self.next][b.slot], blend);
                B {
                    x: w * (0.5 + 0.4 * (self.t * b.fx + b.px).sin()),
                    y: ph * (0.5 + 0.38 * (self.t * b.fy + b.py).sin()),
                    r2: (b.r * side).powi(2),
                    col: [col.0 as f32, col.1 as f32, col.2 as f32],
                }
            })
            .collect();

        // Light from the upper left, towards the viewer.
        let (lx, ly, lz) = (-0.45f32, -0.55f32, 0.70f32);
        let norm = (lx * lx + ly * ly + lz * lz).sqrt();
        let (lx, ly, lz) = (lx / norm, ly / norm, lz / norm);
        // Halfway vector between the light and the viewer, for highlights.
        let (hx, hy, hz) = (lx, ly, lz + 1.0);
        let hn = (hx * hx + hy * hy + hz * hz).sqrt();
        let (hx, hy, hz) = (hx / hn, hy / hn, hz / hn);
        // Typical ball radius: how steeply the domes curve at their rims.
        let steep = side * 0.13;

        for y in 0..self.ph {
            let fy = y as f32 + 0.5;
            let bg = BG_TOP.lerp(BG_BOT, fy / ph);
            for x in 0..self.w {
                let fx = x as f32 + 0.5;
                let (mut f, mut gx, mut gy) = (0.0f32, 0.0f32, 0.0f32);
                let mut col = [0.0f32; 3];
                for b in &balls {
                    let (dx, dy) = (fx - b.x, fy - b.y);
                    let d2 = (dx * dx + dy * dy).max(0.25);
                    let v = b.r2 / d2;
                    f += v;
                    // d/dx of r²/d² is -2 r² dx / d⁴.
                    let g = -2.0 * v / d2;
                    gx += g * dx;
                    gy += g * dy;
                    for k in 0..3 {
                        col[k] += v * b.col[k];
                    }
                }
                let base = Rgb((col[0] / f) as u8, (col[1] / f) as u8, (col[2] / f) as u8);
                let p = if f >= 1.0 {
                    // Height h = 1 - 1/f: flat on top of each drop and steep at
                    // its rim, like a dome. Its gradient is the field's over f².
                    let f2 = f * f;
                    let (nx, ny) = (-gx / f2 * steep, -gy / f2 * steep);
                    let nn = (nx * nx + ny * ny + 1.0).sqrt();
                    let (nx, ny, nz) = (nx / nn, ny / nn, 1.0 / nn);
                    let diff = (nx * lx + ny * ly + nz * lz).max(0.0);
                    let spec = (nx * hx + ny * hy + nz * hz).max(0.0).powi(24);
                    base.scale(0.25 + 0.85 * diff).lerp(Rgb::WHITE, spec * 0.85)
                } else {
                    // Outside: a soft glow that thickens near the surface.
                    let glow = f * f * f * 0.55;
                    bg.lerp(base, glow)
                };
                self.px[y * self.w + x] = p;
            }
        }
        c.blit_pixels(&self.px);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    Box::new(Metaballs::new(w, h, rng))
}
