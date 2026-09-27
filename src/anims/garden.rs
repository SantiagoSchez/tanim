//! A garden through the seasons, in half-block pixels: plants sprout and
//! grow pixel by pixel into branching stems with leaves, buds swell and open
//! into blooms; then autumn turns the leaves and strips them, the garden
//! fades, and spring starts over. Clouds drift across the sky meanwhile.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;

/// Pixel-art rows; `.` is transparent. Blooms use `p` petal, `d` shaded
/// petal, `l` highlight and `c` center, and sit with their bottom row on
/// the stem tip.
type Sprite = &'static [&'static str];

const DAISY: Sprite = &[
    ".l.l.",
    "lpdpl",
    ".dcd.",
    "lpdpl",
    ".l.l.",
];
const ROSE: Sprite = &[
    ".pll.",
    "plddp",
    "pdcdp",
    "pddpd",
    ".ppd.",
];
const TULIP: Sprite = &[
    "l.p.p",
    "lpdpp",
    "lpdpd",
    ".ppd.",
];
const SUNFLOWER: Sprite = &[
    "..l.l..",
    ".lpppl.",
    "lpcccpd",
    ".pcccp.",
    "lpcccpd",
    ".dpppd.",
    "..d.d..",
];
const BLOSSOM: Sprite = &[
    ".l.",
    "pcp",
    ".d.",
];
const CUP: Sprite = &[
    "l.p",
    "lpd",
    ".d.",
];
const BIG: [Sprite; 4] = [DAISY, ROSE, TULIP, SUNFLOWER];
const SMALL: [Sprite; 2] = [BLOSSOM, CUP];

const SKY_TOP: Rgb = Rgb(88, 148, 220);
const SKY_BOT: Rgb = Rgb(196, 224, 246);
const FALL_TOP: Rgb = Rgb(110, 112, 170);
const FALL_BOT: Rgb = Rgb(240, 196, 150);
const GRASS: Rgb = Rgb(58, 126, 48);
const GRASS_LIT: Rgb = Rgb(98, 176, 72);
const GRASS_FALL: Rgb = Rgb(128, 118, 56);
const SOIL: Rgb = Rgb(86, 58, 38);
const BROWN: Rgb = Rgb(104, 72, 44);
const BUD: Rgb = Rgb(70, 140, 60);
const LEAF_LIT: Rgb = Rgb(150, 220, 90);
const CENTER: Rgb = Rgb(246, 204, 64);
const CLOUD: Rgb = Rgb(250, 252, 255);

const PETALS: [Rgb; 8] = [
    Rgb(236, 64, 96),
    Rgb(250, 140, 190),
    Rgb(250, 214, 70),
    Rgb(170, 110, 230),
    Rgb(250, 250, 250),
    Rgb(250, 136, 60),
    Rgb(110, 150, 250),
    Rgb(220, 40, 60),
];
const STEMS: [Rgb; 4] = [Rgb(46, 120, 50), Rgb(64, 140, 56), Rgb(38, 100, 60), Rgb(82, 132, 44)];
const AUTUMN_LEAVES: [Rgb; 4] = [Rgb(236, 150, 40), Rgb(214, 70, 40), Rgb(244, 196, 60), Rgb(226, 110, 40)];

const HOLD: u32 = 240;
const AUTUMN: u32 = 260;
const FADE: u32 = 70;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Stem,
    Leaf,
    Flower,
}

#[derive(Clone, Copy)]
struct Part {
    color: Rgb,
    /// What the color turns into by the end of autumn.
    autumn: Rgb,
    kind: Kind,
}

struct Tip {
    x: i32,
    y: i32,
    dx: i32,
    len: i32,
    depth: u8,
    wait: u32,
    period: u32,
    stem: Rgb,
    petal: Rgb,
    bloom: Sprite,
    grew: bool,
}

struct Blossom {
    x: i32,
    y: i32,
    age: u32,
    petal: Rgb,
    sprite: Sprite,
}

struct Falling {
    x: f32,
    y: f32,
    vx: f32,
    phase: f32,
    color: Rgb,
}

struct Cloud {
    x: f32,
    y: f32,
    v: f32,
    puffs: Vec<(f32, f32, f32)>,
}

enum Phase {
    Grow,
    Hold(u32),
    Autumn(u32),
    Fade(u32),
}

struct Garden {
    w: i32,
    /// Height in pixels, twice the rows.
    ph: i32,
    /// First pixel row of the lawn; plants live above it.
    ground: i32,
    px: Vec<Rgb>,
    layer: Vec<Option<Part>>,
    tips: Vec<Tip>,
    blossoms: Vec<Blossom>,
    falling: Vec<Falling>,
    clouds: Vec<Cloud>,
    roots: Vec<i32>,
    /// Grass blade height and shade per column.
    tufts: Vec<(i32, f32)>,
    seed: u32,
    phase: Phase,
    spawn_wait: u32,
    fails: u32,
    season: f32,
    t: u32,
}

#[inline]
fn hash(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263) ^ seed;
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    let (w, ph) = (w as i32, 2 * h as i32);
    let ground = if ph >= 8 { ph - 4 } else { ph - 1 };
    let clouds = (0..(w / 30).max(1)).map(|_| cloud(rng, w, ground, true)).collect();
    let tufts = (0..w).map(|_| (rng.range(0, 3), rng.f32())).collect();
    Box::new(Garden {
        w,
        ph,
        ground,
        px: vec![Rgb::BLACK; (w * ph).max(0) as usize],
        layer: vec![None; (w * ground).max(0) as usize],
        tips: Vec::new(),
        blossoms: Vec::new(),
        falling: Vec::new(),
        clouds,
        roots: Vec::new(),
        tufts,
        seed: rng.next_u64() as u32,
        phase: Phase::Grow,
        spawn_wait: 0,
        fails: 0,
        season: 0.0,
        t: 0,
    })
}

/// A row of overlapping puffs, bigger in the middle.
fn cloud(rng: &mut Rng, w: i32, ground: i32, anywhere: bool) -> Cloud {
    let n = rng.range(3, 6);
    let r = (ground as f32 / 14.0).max(1.5);
    let mut puffs = Vec::new();
    let mut x = 0.0;
    for i in 0..n {
        let big = i > 0 && i < n - 1;
        let pr = r * if big { rng.rangef(1.2, 1.8) } else { rng.rangef(0.8, 1.1) };
        puffs.push((x, -pr * 0.3, pr));
        x += pr * rng.rangef(0.9, 1.3);
    }
    Cloud {
        x: if anywhere { rng.rangef(0.0, w as f32) } else { -x - 2.0 * r },
        y: rng.rangef(r * 2.0, (ground as f32 * 0.35).max(r * 2.0 + 1.0)),
        v: rng.rangef(0.01, 0.04),
        puffs,
    }
}

impl Garden {
    fn cell(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.w || y >= self.ground {
            None
        } else {
            Some((y * self.w + x) as usize)
        }
    }

    fn free(&self, x: i32, y: i32) -> bool {
        self.cell(x, y).is_some_and(|i| self.layer[i].is_none())
    }

    fn kind(&self, x: i32, y: i32) -> Option<Kind> {
        self.cell(x, y).and_then(|i| self.layer[i]).map(|p| p.kind)
    }

    fn set(&mut self, x: i32, y: i32, color: Rgb, autumn: Rgb, kind: Kind) {
        if let Some(i) = self.cell(x, y) {
            self.layer[i] = Some(Part { color, autumn, kind });
        }
    }

    fn set_flower(&mut self, x: i32, y: i32, color: Rgb) {
        self.set(x, y, color, color.scale(0.55), Kind::Flower);
    }

    fn sprout(&mut self, rng: &mut Rng) {
        let w = self.w;
        let max_roots = (w / 5).max(1) as usize;
        if self.roots.len() >= max_roots || self.ground < 4 {
            self.fails += 1;
            return;
        }
        for _ in 0..12 {
            let x = rng.range(1.min(w - 1), (w - 1).max(1));
            if self.roots.iter().all(|&r| (r - x).abs() >= 4) {
                self.roots.push(x);
                let g = self.ground as f32;
                // Leave room above the tallest stems for their bloom.
                let len = rng.range((g * 0.3) as i32, (g * 0.9) as i32 - 6).max(1);
                self.tips.push(Tip {
                    x,
                    y: self.ground,
                    dx: 0,
                    len,
                    depth: 0,
                    wait: 0,
                    period: rng.range(1, 4) as u32,
                    stem: *rng.pick(&STEMS),
                    petal: *rng.pick(&PETALS),
                    bloom: *rng.pick(&BIG),
                    grew: false,
                });
                return;
            }
        }
        self.fails += 1;
    }

    fn grow(&mut self, rng: &mut Rng) {
        let mut i = 0;
        while i < self.tips.len() {
            if self.tips[i].wait > 0 {
                self.tips[i].wait -= 1;
                i += 1;
                continue;
            }
            let t = &self.tips[i];
            let (x, y, depth, len) = (t.x, t.y, t.depth, t.len);
            let mut dx = t.dx;
            if depth == 0 {
                if dx != 0 && rng.chance(0.7) {
                    dx = 0;
                } else if dx == 0 && rng.chance(0.12) {
                    dx = if rng.chance(0.5) { 1 } else { -1 };
                }
            } else if rng.chance(0.35) {
                dx = if dx == 0 { *rng.pick(&[-1, 1]) } else { 0 };
            }
            let (nx, ny) = (x + dx, y - 1);
            let blocked = !self.free(nx, ny) || (dx != 0 && !self.free(nx, ny + 1) && !self.free(x, ny));
            if len <= 0 || blocked {
                let t = self.tips.swap_remove(i);
                if t.grew {
                    self.bloom(&t);
                }
                continue;
            }
            let (stem, petal) = (self.tips[i].stem, self.tips[i].petal);
            self.set(nx, ny, stem, BROWN, Kind::Stem);
            if dx == 0 && rng.chance(0.14) {
                self.leaf(nx, ny, stem, rng);
            }
            if depth < 2 && len > 6 && rng.chance(0.1) {
                let bdx = if rng.chance(0.5) { 1 } else { -1 };
                let blen = (len as f32 * rng.rangef(0.35, 0.7)) as i32;
                self.tips.push(Tip {
                    x: nx,
                    y: ny,
                    dx: bdx,
                    len: blen,
                    depth: depth + 1,
                    wait: 2,
                    period: self.tips[i].period + 1,
                    stem,
                    petal: if rng.chance(0.7) { petal } else { *rng.pick(&PETALS) },
                    bloom: *rng.pick(&SMALL),
                    grew: false,
                });
            }
            let t = &mut self.tips[i];
            t.x = nx;
            t.y = ny;
            t.dx = dx;
            t.len -= 1;
            t.grew = true;
            t.wait = t.period;
            i += 1;
        }
    }

    /// A small leaf pointing up and away from the stem at `(x, y)`.
    fn leaf(&mut self, x: i32, y: i32, stem: Rgb, rng: &mut Rng) {
        let s = if rng.chance(0.5) { 1 } else { -1 };
        let shape: &[(i32, i32)] = if rng.chance(0.5) { &[(1, 0), (2, -1)] } else { &[(1, 0), (2, 0), (2, -1), (3, -1)] };
        if !shape.iter().all(|&(dx, dy)| self.free(x + s * dx, y + dy)) {
            return;
        }
        let autumn = *rng.pick(&AUTUMN_LEAVES);
        for (j, &(dx, dy)) in shape.iter().enumerate() {
            let c = stem.lerp(LEAF_LIT, 0.3 + 0.12 * j as f32);
            self.set(x + s * dx, y + dy, c, autumn, Kind::Leaf);
        }
    }

    fn bloom(&mut self, t: &Tip) {
        let (x, y) = if self.free(t.x, t.y - 1) { (t.x, t.y - 1) } else { (t.x, t.y) };
        // Reserve the pixel right away so no other stem grows through it.
        self.set_flower(x, y, BUD);
        self.blossoms.push(Blossom { x, y, age: 0, petal: t.petal, sprite: t.bloom });
    }

    /// Paint a bloom pixel unless a stem is in the way.
    fn petal(&mut self, x: i32, y: i32, c: Rgb) {
        if self.cell(x, y).is_some() && self.kind(x, y) != Some(Kind::Stem) {
            self.set_flower(x, y, c);
        }
    }

    fn open_blossoms(&mut self) {
        let mut done = Vec::new();
        for (i, b) in self.blossoms.iter_mut().enumerate() {
            b.age += 1;
            if matches!(b.age, 1 | 12 | 24 | 36) {
                done.push((i, b.age));
            }
        }
        for (i, age) in done {
            let Blossom { x, y, petal, sprite, .. } = self.blossoms[i];
            if self.kind(x, y) != Some(Kind::Flower) {
                continue;
            }
            let bud = petal.lerp(BUD, 0.5);
            match age {
                1 => self.set_flower(x, y, BUD),
                // The bud swells, then shows its color.
                12 => {
                    self.set_flower(x, y, BUD);
                    self.petal(x, y - 1, bud);
                }
                24 => {
                    self.set_flower(x, y, BUD);
                    self.petal(x, y - 1, petal.lerp(bud, 0.3));
                    self.petal(x - 1, y - 1, bud);
                    self.petal(x + 1, y - 1, bud);
                    self.petal(x, y - 2, bud);
                }
                _ => {
                    let (sw, sh) = (sprite[0].len() as i32, sprite.len() as i32);
                    let (x0, y0) = (x - sw / 2, y - sh + 1);
                    for (ry, row) in sprite.iter().enumerate() {
                        for (rx, ch) in row.bytes().enumerate() {
                            let c = match ch {
                                b'p' => petal,
                                b'd' => petal.scale(0.78),
                                b'l' => petal.lerp(Rgb::WHITE, 0.35),
                                b'c' => CENTER,
                                _ => continue,
                            };
                            self.petal(x0 + rx as i32, y0 + ry as i32, c);
                        }
                    }
                }
            }
        }
        self.blossoms.retain(|b| b.age < 36);
    }

    fn shed(&mut self, rng: &mut Rng, p: f32, force: bool) {
        for i in 0..self.layer.len() {
            let Some(part) = self.layer[i] else { continue };
            let chance = match part.kind {
                Kind::Flower => 0.006 + p * 0.016,
                Kind::Leaf => 0.001 + p * p * 0.02,
                Kind::Stem => continue,
            };
            if force || rng.chance(chance) {
                self.layer[i] = None;
                // Some petals just wither rather than all raining down.
                if part.kind == Kind::Flower && rng.chance(0.5) {
                    continue;
                }
                self.falling.push(Falling {
                    x: (i as i32 % self.w) as f32,
                    y: (i as i32 / self.w) as f32,
                    vx: rng.rangef(-0.08, 0.12),
                    phase: rng.rangef(0.0, 6.28),
                    color: part.color.lerp(part.autumn, p * 2.5),
                });
            }
        }
    }

    fn reset(&mut self) {
        self.layer.fill(None);
        self.tips.clear();
        self.blossoms.clear();
        self.roots.clear();
        self.fails = 0;
        self.spawn_wait = 10;
        self.phase = Phase::Grow;
    }

    #[inline]
    fn put(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && x < self.w && y < self.ph {
            self.px[(y * self.w + x) as usize] = c;
        }
    }

    fn draw_sky(&mut self) {
        let (w, ground) = (self.w, self.ground);
        let top = SKY_TOP.lerp(FALL_TOP, self.season);
        let bot = SKY_BOT.lerp(FALL_BOT, self.season);
        for y in 0..ground.max(0) {
            let c = top.lerp(bot, y as f32 / ground.max(1) as f32);
            self.px[(y * w) as usize..((y + 1) * w) as usize].fill(c);
        }
        let white = CLOUD.lerp(FALL_BOT, 0.3 * self.season);
        let under = white.lerp(bot, 0.3);
        for i in 0..self.clouds.len() {
            let (x0, y0) = (self.clouds[i].x, self.clouds[i].y);
            for p in 0..self.clouds[i].puffs.len() {
                let (px, py, r) = self.clouds[i].puffs[p];
                let (cx, cy) = (x0 + px, y0 + py);
                for y in (cy - r) as i32..=(cy + r) as i32 {
                    if y >= ground {
                        break;
                    }
                    for x in (cx - r) as i32..=(cx + r) as i32 {
                        let (dx, dy) = (x as f32 - cx, y as f32 - cy);
                        // Flat-bottomed puffs.
                        if dx * dx + dy * dy <= r * r && y as f32 <= y0 + r * 0.5 {
                            let c = if y as f32 > y0 + r * 0.1 { under } else { white };
                            self.put(x, y, c);
                        }
                    }
                }
            }
        }
    }

    fn draw_ground(&mut self) {
        let (w, ph, ground) = (self.w, self.ph, self.ground);
        let grass = GRASS.lerp(GRASS_FALL, self.season);
        let lit = GRASS_LIT.lerp(GRASS_FALL.scale(1.3), self.season);
        for x in 0..w {
            let (tuft, shade) = self.tufts[x as usize];
            let blade = grass.lerp(lit, shade);
            for y in ground - tuft..ground {
                self.put(x, y, blade);
            }
            self.put(x, ground, lit.lerp(grass, shade * 0.5));
            self.put(x, ground + 1, grass);
            for y in ground + 2..ph {
                let speck = hash(x, y, self.seed) % 7 == 0;
                self.put(x, y, if speck { SOIL.scale(1.35) } else { SOIL.scale(1.0 - 0.06 * (y - ground) as f32) });
            }
        }
    }
}

impl Animation for Garden {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        let (w, ph) = (self.w, self.ph);
        if w <= 0 || ph <= 0 {
            return;
        }
        self.t += 1;

        // Season state machine.
        let (mut autumn, mut fade) = (0.0f32, 0.0f32);
        match self.phase {
            Phase::Grow => {
                if self.spawn_wait == 0 {
                    self.sprout(rng);
                    self.spawn_wait = rng.range(15, 60) as u32;
                } else {
                    self.spawn_wait -= 1;
                }
                self.grow(rng);
                self.open_blossoms();
                if self.fails >= 6 && self.tips.is_empty() && self.blossoms.is_empty() {
                    self.phase = Phase::Hold(0);
                }
            }
            Phase::Hold(n) => {
                self.phase = if n >= HOLD { Phase::Autumn(0) } else { Phase::Hold(n + 1) };
            }
            Phase::Autumn(n) => {
                autumn = n as f32 / AUTUMN as f32;
                self.shed(rng, autumn, n + 1 >= AUTUMN);
                self.phase = if n >= AUTUMN { Phase::Fade(0) } else { Phase::Autumn(n + 1) };
            }
            Phase::Fade(n) => {
                autumn = 1.0;
                fade = n as f32 / FADE as f32;
                if n >= FADE {
                    self.reset();
                } else {
                    self.phase = Phase::Fade(n + 1);
                }
            }
        }
        let season_target = if matches!(self.phase, Phase::Autumn(_) | Phase::Fade(_)) { 1.0 } else { 0.0 };
        self.season += (season_target - self.season) * 0.012;

        for i in 0..self.clouds.len() {
            let cl = &mut self.clouds[i];
            cl.x += cl.v;
            let width = cl.puffs.last().map_or(0.0, |p| p.0 + p.2);
            if cl.x - width > w as f32 {
                self.clouds[i] = cloud(rng, w, self.ground, false);
            }
        }
        self.draw_sky();
        self.draw_ground();

        // Plants.
        for i in 0..self.layer.len() {
            let Some(p) = self.layer[i] else { continue };
            // The layer covers the rows above the ground, so it indexes `px` directly.
            // Leaves turn early, so most show their color before they drop,
            // while stems only slowly wither to brown.
            let turn = if p.kind == Kind::Stem { autumn } else { autumn * 2.5 };
            let mut col = p.color.lerp(p.autumn, turn);
            if fade > 0.0 {
                col = col.lerp(self.px[i], fade);
            }
            self.px[i] = col;
        }

        // Falling petals and leaves.
        let t = self.t as f32;
        let ground = self.ground;
        let mut falling = std::mem::take(&mut self.falling);
        falling.retain_mut(|f| {
            f.y += 0.16;
            f.x += f.vx + (t * 0.08 + f.phase).sin() * 0.12;
            let (x, y) = (f.x.round() as i32, f.y as i32);
            self.put(x, y, f.color);
            y < ground
        });
        self.falling = falling;
        c.blit_pixels(&self.px);
    }
}
