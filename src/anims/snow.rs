//! Night snowfall: parallax flakes drifting in gusty wind, settling pixel by
//! pixel on pine branches, roofs and chimneys and piling up on the
//! ground, then slowly melting away before the next snowfall.
//!
//! Drawn in half-block pixels. The scenery is a static pixel-art layer; snow
//! that settles on it lives in its own per-pixel layer and grows caps a few
//! pixels thick on anything a flake can land on.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;

const SKY_TOP: Rgb = Rgb(4, 7, 20);
const SKY_BOT: Rgb = Rgb(26, 36, 64);
const HILLS: Rgb = Rgb(60, 72, 106);
const HILLS_SHADE: Rgb = Rgb(34, 44, 74);
const FAR_PINE: Rgb = Rgb(14, 22, 38);
const MOON: Rgb = Rgb(236, 236, 214);
const EARTH: Rgb = Rgb(34, 30, 32);
const SNOW_HI: Rgb = Rgb(232, 238, 250);
const SNOW_LO: Rgb = Rgb(150, 168, 206);
const PINE: Rgb = Rgb(14, 52, 34);
const PINE_LIT: Rgb = Rgb(40, 100, 60);
const TRUNK: Rgb = Rgb(82, 54, 36);
const ROOF: Rgb = Rgb(112, 38, 40);
const ROOF_DARK: Rgb = Rgb(82, 26, 32);
const WALL: Rgb = Rgb(86, 68, 56);
const WALL_DARK: Rgb = Rgb(66, 52, 44);
const BRICK: Rgb = Rgb(80, 50, 44);
const LIT: Rgb = Rgb(236, 192, 96);
const UNLIT: Rgb = Rgb(34, 36, 56);
const DOOR: Rgb = Rgb(58, 38, 30);
const SMOKE: Rgb = Rgb(150, 150, 160);
/// How many pixels of snow can stack on top of scenery.
const CAP: usize = 2;

struct Flake {
    x: f32,
    y: f32,
    z: f32,
    phase: f32,
}

struct Puff {
    x: f32,
    y: f32,
    age: u32,
}

/// A square window: top-left pixel, side and whether the lamp is on.
struct Window {
    x: usize,
    y: usize,
    n: usize,
    lit: bool,
}

struct Snow {
    w: usize,
    ph: usize,
    /// First pixel row of bare earth; scenery stands on the row above.
    ground: usize,
    /// Snow depth on the ground per column, in pixels.
    depth: Vec<f32>,
    /// Sky, moon and distant hills; never changes.
    bg: Vec<Rgb>,
    /// Scenery pixels (`None` where the background shows through).
    scen: Vec<Option<Rgb>>,
    /// Snow settled on scenery, per pixel in `[0, 1]`.
    frost: Vec<f32>,
    windows: Vec<Window>,
    chimneys: Vec<(f32, f32)>,
    stars: Vec<(usize, f32, f32)>,
    flakes: Vec<Flake>,
    puffs: Vec<Puff>,
    wind: f32,
    wind_target: f32,
    intensity: f32,
    melting: bool,
    max_depth: f32,
    pile_rate: f32,
    t: u32,
    px: Vec<Rgb>,
}

fn hash(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x27d4_eb2d) ^ (y as u32).wrapping_mul(0x1656_67b1);
    h = (h ^ (h >> 15)).wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    (h & 0xffff) as f32 / 65535.0
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    let ph = h * 2;
    let ground = ph.saturating_sub(2);
    let mut s = Snow {
        w,
        ph,
        ground,
        depth: vec![0.0; w],
        bg: Vec::new(),
        scen: vec![None; w * ph],
        frost: vec![0.0; w * ph],
        windows: Vec::new(),
        chimneys: Vec::new(),
        stars: Vec::new(),
        flakes: Vec::new(),
        puffs: Vec::new(),
        wind: 0.0,
        wind_target: rng.rangef(-0.2, 0.2),
        intensity: 1.0,
        melting: false,
        max_depth: (ph as f32 * 0.15).max(1.0),
        pile_rate: (h as f32 / 30.0).clamp(0.5, 3.0),
        t: 0,
        px: vec![Rgb::BLACK; w * ph],
    };
    s.build_sky(rng);
    if ph >= 12 {
        s.build_scenery(rng);
    }
    for _ in 0..s.target() {
        let mut f = s.spawn(rng);
        f.y = rng.rangef(0.0, ph as f32);
        s.flakes.push(f);
    }
    Box::new(s)
}

impl Snow {
    fn target(&self) -> usize {
        (self.w as f32 * self.ph as f32 * 0.03 * self.intensity) as usize
    }

    fn spawn(&self, rng: &mut Rng) -> Flake {
        Flake {
            x: rng.rangef(-4.0, self.w as f32 + 4.0),
            y: rng.rangef(-6.0, 0.0),
            z: rng.f32(),
            phase: rng.rangef(0.0, 6.28),
        }
    }

    /// Sky gradient, a haloed moon, low hills with a far treeline, and stars.
    fn build_sky(&mut self, rng: &mut Rng) {
        let (w, ph) = (self.w, self.ph);
        self.bg = (0..w * ph).map(|i| Rgb::gradient(&[SKY_TOP, SKY_BOT], (i / w.max(1)) as f32 / ph.max(1) as f32)).collect();
        if ph < 8 {
            return;
        }
        let r = (ph as f32 / 16.0).clamp(1.5, 5.0);
        let (mx, my) = (rng.rangef(0.55, 0.9) * w as f32, rng.rangef(0.12, 0.25) * ph as f32);
        // Far hills: gentle snowy slopes in moonlight, with a dark treeline of
        // tiny pines along their crest.
        let phase = [rng.rangef(0.0, 6.3), rng.rangef(0.0, 6.3), rng.rangef(0.0, 6.3)];
        let hill = |x: f32| {
            (x * 0.06 + phase[0]).sin() * 0.5 + (x * 0.13 + phase[1]).sin() * 0.3 + (x * 0.29 + phase[2]).sin() * 0.2
        };
        let base = ph as f32 * 0.72;
        let ridge: Vec<f32> = (0..w).map(|x| base - ph as f32 * (0.08 + 0.08 * hill(x as f32))).collect();
        let mut trees = vec![f32::MAX; w];
        let mut x = 0;
        while x < w {
            let tall = rng.rangef(1.5, (ph as f32 * 0.05).max(2.0));
            for dx in -2i32..=2 {
                let xx = x as i32 + dx;
                if xx >= 0 && (xx as usize) < w {
                    let y = ridge[x] + 1.0 - tall + dx.abs() as f32 * 1.6;
                    trees[xx as usize] = trees[xx as usize].min(y);
                }
            }
            x += rng.range(1, 4) as usize;
        }
        for y in 0..ph {
            for x in 0..w {
                let i = y * w + x;
                let (dx, dy) = (x as f32 - mx, y as f32 - my);
                let d = (dx * dx + dy * dy).sqrt();
                if d < r {
                    // Soft limb and a mottled face.
                    let mare = if hash(x as i32, y as i32) > 0.75 { 0.9 } else { 1.0 };
                    self.bg[i] = self.bg[i].lerp(MOON.scale(mare), (r - d + 0.5).min(1.0));
                } else {
                    self.bg[i] = self.bg[i].add(Rgb(20, 22, 30).scale(r * 1.5 / d));
                }
                let yf = y as f32;
                if yf >= ridge[x] {
                    // Lit along the crest, fading into shadow lower down.
                    let k = ((yf - ridge[x]) / (ph as f32 * 0.12)).min(1.0);
                    self.bg[i] = HILLS.lerp(HILLS_SHADE, k);
                }
                if yf >= trees[x] && yf < ridge[x] + 2.0 {
                    self.bg[i] = FAR_PINE;
                }
            }
        }
        for _ in 0..(w * ph / 110) {
            let y = rng.below(ph * 45 / 100);
            let i = y * w + rng.below(w);
            let (dx, dy) = ((i % w) as f32 - mx, y as f32 - my);
            if dx * dx + dy * dy > (r * 3.0) * (r * 3.0) {
                self.stars.push((i, rng.rangef(0.3, 1.0), rng.rangef(0.0, 6.28)));
            }
        }
    }

    fn place(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.ground {
            self.scen[y as usize * self.w + x as usize] = Some(c);
        }
    }

    fn build_scenery(&mut self, rng: &mut Rng) {
        let (w, ph) = (self.w as i32, self.ph as i32);
        let base = self.ground as i32 - 1;
        let mut x = rng.range(0, 6);
        let mut houses = 0;
        while x < w {
            if rng.chance(0.28) && ph >= 24 && houses < 1 + w / 60 {
                houses += 1;
                x = self.house(x, base, rng) + rng.range(3, 9);
            } else {
                x += self.pine(x, base, rng) + rng.range(2, 10);
            }
        }
    }

    /// A pine standing on row `base`; returns how far it spreads to the right.
    fn pine(&mut self, x: i32, base: i32, rng: &mut Rng) -> i32 {
        let th = rng.range(8, (self.ph as i32 / 2).max(9));
        let trunk = (th / 7).max(1);
        let crown = th - trunk;
        let tiers = (crown / 5).clamp(1, 5) as f32;
        let wide = rng.rangef(0.36, 0.46);
        let mut spread = 0;
        for r in 0..crown {
            let u = r as f32 / crown as f32;
            // Each tier flares out and the next starts narrower: the classic
            // layered fir outline.
            let f = (u * tiers).fract();
            let half = r as f32 * wide * (0.6 + 0.4 * f) + 0.3;
            let y = base - trunk - crown + 1 + r;
            let hi = half.round() as i32;
            spread = spread.max(hi);
            for dx in -hi..=hi {
                // Ragged tips of needles along the edges.
                if dx.abs() == hi && hi > 1 && hash(x + dx, y) < 0.35 {
                    continue;
                }
                // Moonlight from the upper left; needles speckle the shading.
                let side = 0.5 - dx as f32 / (2.0 * half.max(1.0));
                let k = (side * 0.7 + (1.0 - f) * 0.2 + hash(x + dx, y * 7) * 0.25).clamp(0.0, 1.0);
                self.place(x + dx, y, PINE.lerp(PINE_LIT, k * k));
            }
        }
        for r in 0..trunk {
            self.place(x, base - r, TRUNK);
            if th > 20 {
                self.place(x + 1, base - r, TRUNK.scale(0.75));
            }
        }
        spread
    }

    /// A cottage whose left wall starts at `x`; returns its right edge. It
    /// grows with the screen so deep drifts do not swallow it whole.
    fn house(&mut self, x: i32, base: i32, rng: &mut Rng) -> i32 {
        let ph = self.ph as i32;
        let walls = rng.range((ph / 8).max(6), (ph / 6).max(7) + 1);
        let half = walls + rng.range(-1, 3);
        let cx = x + half + 1;
        let wall_top = base - walls + 1;
        // Walls with plank lines.
        for y in wall_top..=base {
            for dx in -half..=half {
                let c = if (y - wall_top) % 3 == 2 { WALL_DARK } else { WALL };
                self.place(cx + dx, y, c);
            }
        }
        // Roof: 45 degree slopes overhanging the walls, shingle rows.
        let rh = half + 2;
        for k in 0..rh {
            let y = wall_top - rh + k;
            let span = k + 1;
            for dx in -span..=span {
                let c = if dx.abs() == span || k % 2 == 1 { ROOF_DARK } else { ROOF };
                self.place(cx + dx, y, c);
            }
        }
        // Chimney poking out of the right slope.
        let chx = cx + half / 2 + 1;
        let slope_y = wall_top - rh + (chx - cx).abs() - 1;
        let chy = slope_y - 3 - walls / 5;
        for y in chy..=slope_y {
            for dx in 0..2 {
                self.place(chx + dx, y, if (y + dx) % 2 == 0 { BRICK } else { BRICK.scale(0.8) });
            }
        }
        self.chimneys.push((chx as f32 + 0.5, chy as f32 - 1.0));
        // Door with a glinting knob, then a row of windows with dark sills.
        let door_x = cx - half / 2 - 1;
        let door_h = (walls * 2 / 3).max(4);
        for y in base - door_h + 1..=base {
            for dx in 0..3 {
                self.place(door_x + dx, y, DOOR);
            }
        }
        self.place(door_x + 2, base - door_h / 2, LIT.scale(0.6));
        let n = if walls >= 12 { 3 } else { 2 };
        let wy = wall_top + (walls - n) / 3;
        let mut wx = cx - half + 2;
        while wx + n < cx + half {
            if wx + n < door_x || wx > door_x + 3 {
                for dx in -1..=n {
                    self.place(wx + dx, wy + n, WALL_DARK);
                }
                if wx >= 0 && wx + n <= self.w as i32 && wy >= 0 {
                    self.windows.push(Window { x: wx as usize, y: wy as usize, n: n as usize, lit: rng.chance(0.6) });
                }
            }
            wx += n + 2;
        }
        cx + half
    }

    fn solid(&self, x: usize, y: usize) -> bool {
        self.scen[y * self.w + x].is_some()
    }

    /// Whether snow can settle in empty pixel `(x, y)`: something solid
    /// lies below it with at most `CAP` pixels of settled snow in between.
    fn supported(&self, x: usize, y: usize) -> bool {
        for k in 1..=CAP + 1 {
            let yy = y + k;
            if yy >= self.ground {
                return false;
            }
            if self.solid(x, yy) {
                return true;
            }
            if self.frost[yy * self.w + x] < 1.0 {
                return false;
            }
        }
        false
    }

    #[inline]
    fn blend(&mut self, x: i32, y: i32, col: Rgb, a: f32) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.ph {
            let p = &mut self.px[y as usize * self.w + x as usize];
            *p = p.lerp(col, a);
        }
    }

    /// Anti-aliased dot spread over the four pixels around `(x, y)`.
    fn splat(&mut self, x: f32, y: f32, col: Rgb, a: f32) {
        let (xf, yf) = (x.floor(), y.floor());
        let (fx, fy) = (x - xf, y - yf);
        let (xi, yi) = (xf as i32, yf as i32);
        self.blend(xi, yi, col, a * (1.0 - fx) * (1.0 - fy));
        self.blend(xi + 1, yi, col, a * fx * (1.0 - fy));
        self.blend(xi, yi + 1, col, a * (1.0 - fx) * fy);
        self.blend(xi + 1, yi + 1, col, a * fx * fy);
    }
}

impl Animation for Snow {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        let (w, ph) = (self.w, self.ph);
        if w == 0 || ph == 0 {
            return;
        }
        self.t += 1;
        let t = self.t as f32;

        // Weather and the season cycle.
        if self.t % 100 == 0 {
            self.wind_target = if rng.chance(0.15) { rng.rangef(-0.7, 0.7) } else { rng.rangef(-0.2, 0.2) };
        }
        self.wind += (self.wind_target - self.wind) * 0.02;
        let mean = self.depth.iter().sum::<f32>() / w as f32;
        if !self.melting && mean >= self.max_depth {
            self.melting = true;
        }
        if self.melting {
            self.intensity = (self.intensity - 0.004).max(0.0);
            let mut any = false;
            for d in &mut self.depth {
                *d = (*d - 0.012 - *d * 0.0015).max(0.0);
                any |= *d > 0.0;
            }
            for f in &mut self.frost {
                *f = (*f - 0.004).max(0.0);
            }
            if !any {
                self.melting = false;
            }
        } else {
            self.intensity = (self.intensity + 0.003).min(1.0);
        }

        // Flakes: move, settle, respawn.
        let target = self.target();
        let wind = self.wind;
        let ground = self.ground;
        let mut i = 0;
        while i < self.flakes.len() {
            let f = &mut self.flakes[i];
            f.y += 0.14 + f.z * 0.4;
            f.x += wind * (0.3 + 0.7 * f.z) + (t * 0.06 + f.phase).sin() * 0.06;
            let span = w as f32 + 8.0;
            if f.x < -4.0 {
                f.x += span;
            } else if f.x > w as f32 + 4.0 {
                f.x -= span;
            }
            let (fx, fy, z) = (f.x.round() as i32, f.y as i32, f.z);
            let mut gone = f.y > ph as f32 + 1.0;
            if fx >= 0 && (fx as usize) < w {
                let col = fx as usize;
                if f.y >= ground as f32 - self.depth[col] {
                    gone = true;
                    if z > 0.3 {
                        // Spread over neighbouring columns so the pile grows smoothly.
                        let amount = (0.3 + z * 0.3) * self.pile_rate;
                        for (dx, k) in [(-1, 0.25), (0, 0.5), (1, 0.25)] {
                            let xx = (fx + dx).clamp(0, w as i32 - 1) as usize;
                            self.depth[xx] = (self.depth[xx] + amount * k).min(ground as f32);
                        }
                    }
                } else if fy >= 0 && (fy as usize) < ground && z > 0.4 && !self.melting {
                    let (x, y) = (col, fy as usize);
                    let idx = y * w + x;
                    if self.scen[idx].is_none() && self.supported(x, y) && rng.chance(0.3) {
                        self.frost[idx] = (self.frost[idx] + 0.3).min(1.0);
                        gone = true;
                    }
                }
            }
            if gone {
                if self.flakes.len() <= target {
                    self.flakes[i] = self.spawn(rng);
                } else {
                    self.flakes.swap_remove(i);
                    continue;
                }
            }
            i += 1;
        }
        if self.flakes.len() < target {
            for _ in 0..(target - self.flakes.len()).min(w / 4 + 2) {
                let f = self.spawn(rng);
                self.flakes.push(f);
            }
        }

        // Let steep piles slump so drifts stay smooth.
        for x in 1..w {
            let d = self.depth[x] - self.depth[x - 1];
            if d.abs() > 0.7 {
                let m = d * 0.25;
                self.depth[x] -= m;
                self.depth[x - 1] += m;
            }
        }

        // Background: sky, moon, hills and twinkling stars.
        self.px.copy_from_slice(&self.bg);
        for &(i, b, p) in &self.stars {
            let tw = b * (0.6 + 0.4 * (t * 0.05 + p).sin());
            self.px[i] = self.px[i].lerp(Rgb(210, 220, 250), tw);
        }

        // Far flakes behind the scenery.
        for f in &self.flakes {
            if f.z < 0.33 {
                let (x, y) = (f.x.round() as i32, f.y as i32);
                if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < ph {
                    let p = &mut self.px[y as usize * w + x as usize];
                    *p = p.lerp(SNOW_HI, 0.25 + f.z);
                }
            }
        }

        // Scenery, lit windows, and the snow resting on them.
        for win in &mut self.windows {
            if rng.chance(0.0015) {
                win.lit = !win.lit;
            }
        }
        for (idx, s) in self.scen.iter().enumerate() {
            if let Some(col) = s {
                self.px[idx] = *col;
            }
        }
        for win in &self.windows {
            let col = if win.lit { LIT } else { UNLIT };
            for dy in 0..win.n {
                for dx in 0..win.n {
                    let (x, y) = (win.x + dx, win.y + dy);
                    if x < w && y < ph {
                        // Lit panes are brighter at the top, like a lamp inside.
                        self.px[y * w + x] = if win.lit && dy > 0 { col.scale(0.9 - 0.05 * dy as f32) } else { col };
                    }
                }
            }
        }
        for (idx, &fr) in self.frost.iter().enumerate() {
            if fr > 0.0 {
                // Settled snow is brightest on its top pixel.
                let top = idx < w || self.frost[idx - w] < 0.5;
                let col = if top { SNOW_HI } else { SNOW_LO.lerp(SNOW_HI, 0.5) };
                self.px[idx] = self.px[idx].lerp(col, fr);
            }
        }

        // Chimney smoke.
        if self.t % 4 == 0 {
            for &(x, y) in &self.chimneys {
                self.puffs.push(Puff { x, y, age: 0 });
            }
        }
        let mut puffs = std::mem::take(&mut self.puffs);
        puffs.retain_mut(|p| {
            p.age += 1;
            p.y -= 0.12 + p.age as f32 * 0.001;
            p.x += wind * 0.5 + 0.02 + (p.age as f32 * 0.15 + p.y).sin() * 0.03;
            let a = 0.45 * (1.0 - p.age as f32 / 60.0);
            self.splat(p.x, p.y, SMOKE, a);
            // Older puffs billow out.
            if p.age > 20 {
                self.splat(p.x + 1.0, p.y + 0.5, SMOKE, a * 0.6);
            }
            p.age < 60
        });
        self.puffs = puffs;

        // Ground and snow pile, shaded brighter towards the crest.
        for x in 0..w {
            let d = self.depth[x];
            // The first dusting whitens the earth evenly, not in streaks.
            let near = &self.depth[x.saturating_sub(2)..(x + 3).min(w)];
            let cover = (near.iter().sum::<f32>() / near.len() as f32 * 3.0).min(1.0);
            for y in ground..ph {
                let earth = if (x + y) % 3 == 0 { EARTH.scale(1.3) } else { EARTH };
                self.px[y * w + x] = earth.lerp(SNOW_LO, cover);
            }
            let surf = ground as f32 - d;
            let top = surf.floor().max(0.0) as usize;
            for y in top..ground {
                let cov = (y as f32 + 1.0 - surf).clamp(0.0, 1.0);
                let below = (y as f32 - surf) / self.max_depth;
                let col = SNOW_HI.lerp(SNOW_LO, below * 0.9);
                let p = &mut self.px[y * w + x];
                *p = p.lerp(col, cov);
            }
        }

        // Nearer flakes in front: crisp dots, the closest ones soft and big.
        for i in 0..self.flakes.len() {
            let Flake { x, y, z, .. } = self.flakes[i];
            if z < 0.33 {
                continue;
            }
            let xi = x.round() as i32;
            if xi >= 0 && (xi as usize) < w && y >= ground as f32 - self.depth[xi as usize] {
                continue;
            }
            if z > 0.75 {
                self.splat(x - 0.5, y - 0.5, SNOW_HI, 1.0);
                self.blend(xi, y as i32, SNOW_HI, 0.5);
            } else {
                self.blend(xi, y as i32, SNOW_HI, 0.5 + z * 0.5);
            }
        }
        c.blit_pixels(&self.px);
    }
}
