//! Classic sum-of-sines plasma: linear waves, a rotating wave and two
//! moving radial waves, colored through cycling palettes that crossfade.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;
use std::f32::consts::TAU;

const TABLE: usize = 1024;

/// Cyclic palettes: the last stop blends back into the first.
const PALETTES: &[&[Rgb]] = &[
    &[Rgb(20, 0, 60), Rgb(200, 0, 160), Rgb(255, 120, 200), Rgb(0, 200, 255), Rgb(0, 40, 140)],
    &[Rgb(40, 0, 50), Rgb(180, 20, 60), Rgb(255, 120, 20), Rgb(255, 220, 90), Rgb(120, 20, 80)],
    &[Rgb(0, 10, 40), Rgb(0, 90, 140), Rgb(40, 210, 200), Rgb(220, 255, 250), Rgb(0, 60, 120)],
    &[Rgb(0, 30, 10), Rgb(30, 130, 40), Rgb(190, 230, 60), Rgb(250, 250, 180), Rgb(20, 90, 60)],
    &[Rgb(0, 0, 0), Rgb(140, 0, 0), Rgb(255, 110, 0), Rgb(255, 240, 120), Rgb(90, 0, 20)],
    &[Rgb(255, 0, 0), Rgb(255, 255, 0), Rgb(0, 255, 0), Rgb(0, 255, 255), Rgb(0, 0, 255), Rgb(255, 0, 255)],
    &[Rgb(30, 20, 60), Rgb(120, 90, 200), Rgb(250, 200, 240), Rgb(90, 200, 220), Rgb(40, 40, 110)],
];

fn build_lut(stops: &[Rgb]) -> [Rgb; 256] {
    let mut lut = [Rgb::BLACK; 256];
    let n = stops.len();
    for (i, c) in lut.iter_mut().enumerate() {
        let t = i as f32 / 256.0 * n as f32;
        let k = t as usize % n;
        // Smoothstep between stops keeps the bands soft.
        let f = t - t.floor();
        let f = f * f * (3.0 - 2.0 * f);
        *c = stops[k].lerp(stops[(k + 1) % n], f);
    }
    lut
}

struct Plasma {
    w: usize,
    ph: usize,
    sin: Vec<f32>,
    px: Vec<Rgb>,
    luts: Vec<[Rgb; 256]>,
    cur: usize,
    next: usize,
    blend: f32,
    hold: u32,
    t: f32,
    scale: f32,
    // Per-frame scratch tables.
    col_a: Vec<f32>,
    col_s: Vec<f32>,
    col_c: Vec<f32>,
    row_a: Vec<f32>,
    row_s: Vec<f32>,
    row_c: Vec<f32>,
}

impl Plasma {
    #[inline]
    fn tsin(&self, x: f32) -> f32 {
        self.sin[((x * (TABLE as f32 / TAU)) as i32 & (TABLE as i32 - 1)) as usize]
    }
}

impl Animation for Plasma {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        let (w, ph) = (self.w, self.ph);
        // Wrap long before f32 loses integer precision.
        self.t = (self.t + 1.0) % 4_000_000.0;
        let t = self.t;
        let s = self.scale;

        // Palette schedule: hold, then crossfade to a random other palette.
        if self.blend > 0.0 {
            self.blend += 1.0 / 90.0;
            if self.blend >= 1.0 {
                self.cur = self.next;
                self.blend = 0.0;
                self.hold = 0;
            }
        } else {
            self.hold += 1;
            if self.hold > 500 {
                self.next = (self.cur + 1 + rng.below(self.luts.len() - 1)) % self.luts.len();
                self.blend = 1.0 / 90.0;
            }
        }
        let mut lut = self.luts[self.cur];
        if self.blend > 0.0 {
            let nx = &self.luts[self.next];
            for (a, b) in lut.iter_mut().zip(nx.iter()) {
                *a = a.lerp(*b, self.blend);
            }
        }

        // Wave parameters drift over time.
        let k1 = s * (1.0 + 0.3 * (t * 0.0031).sin());
        let k2 = s * (1.2 + 0.3 * (t * 0.0023).cos());
        let ang = t * 0.004;
        let (sa, ca) = ang.sin_cos();
        let k3 = s * 0.9;
        let k4 = s * 1.4;
        let fw = w as f32;
        let fh = ph as f32;
        let c1 = (
            fw * (0.5 + 0.4 * (t * 0.0071).sin()),
            fh * (0.5 + 0.4 * (t * 0.0053).cos()),
        );
        let c2 = (
            fw * (0.5 + 0.45 * (t * 0.0043 + 2.0).cos()),
            fh * (0.5 + 0.45 * (t * 0.0067 + 1.0).sin()),
        );

        for x in 0..w {
            let xf = x as f32;
            self.col_a[x] = self.tsin(xf * k1 + t * 0.05);
            let a = xf * ca * k3;
            self.col_s[x] = self.tsin(a);
            self.col_c[x] = self.tsin(a + TAU / 4.0);
        }
        for y in 0..ph {
            let yf = y as f32;
            self.row_a[y] = self.tsin(yf * k2 - t * 0.037);
            let b = yf * sa * k3 + t * 0.045;
            self.row_s[y] = self.tsin(b);
            self.row_c[y] = self.tsin(b + TAU / 4.0);
        }

        let shift = (t * 0.6) as i32;
        let tr1 = t * 0.08;
        let tr2 = t * 0.06;
        let idx_scale = TABLE as f32 / TAU;
        let mask = TABLE as i32 - 1;
        for y in 0..ph {
            let yf = y as f32;
            let dy1 = (yf - c1.1) * (yf - c1.1);
            let dy2 = (yf - c2.1) * (yf - c2.1);
            let (ra, rs, rc) = (self.row_a[y], self.row_s[y], self.row_c[y]);
            let row = &mut self.px[y * w..(y + 1) * w];
            for x in 0..w {
                let xf = x as f32;
                let d1 = ((xf - c1.0) * (xf - c1.0) + dy1).sqrt() * k4 - tr1;
                let d2 = ((xf - c2.0) * (xf - c2.0) + dy2).sqrt() * k4 * 0.8 + tr2;
                let r1 = self.sin[((d1 * idx_scale) as i32 & mask) as usize];
                let r2 = self.sin[((d2 * idx_scale) as i32 & mask) as usize];
                // sin(a + b) = sin a cos b + cos a sin b
                let rot = self.col_s[x] * rc + self.col_c[x] * rs;
                let v = self.col_a[x] + ra + rot + r1 + r2;
                // 64 color levels: neighbours share colors more often, which
                // cuts the escape-sequence output by about a third.
                row[x] = lut[(((v * 32.0) as i32 + shift) & 252) as usize];
            }
        }
        c.blit_pixels(&self.px);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    let ph = h * 2;
    let luts: Vec<[Rgb; 256]> = PALETTES.iter().map(|p| build_lut(p)).collect();
    let cur = rng.below(luts.len());
    Box::new(Plasma {
        w,
        ph,
        sin: (0..TABLE).map(|i| (i as f32 / TABLE as f32 * TAU).sin()).collect(),
        px: vec![Rgb::BLACK; w * ph],
        next: cur,
        cur,
        luts,
        blend: 0.0,
        hold: 0,
        t: rng.rangef(0.0, 5000.0),
        scale: rng.rangef(0.07, 0.11),
        col_a: vec![0.0; w],
        col_s: vec![0.0; w],
        col_c: vec![0.0; w],
        row_a: vec![0.0; ph],
        row_s: vec![0.0; ph],
        row_c: vec![0.0; ph],
    })
}
