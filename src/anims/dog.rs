//! A meadow seen from the side, with a dog running from edge to edge. Now and
//! then it stops to sit and pant, nap, sniff the grass or chase a passing
//! butterfly, while clouds drift by and the day turns into night and back.
//! The world has a fixed size; Up/Down zoom a camera that trails the dog.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;
use crate::Key;
use std::f32::consts::PI;

/// Pixel-art rows facing right, the last row standing on the ground.
/// `B` coat, `D` patches, `w` cream, `k` eye and nose, `c` collar, `p` tongue.
type Sprite = &'static [&'static str];

const SPRITE_W: i32 = 22;

const STAND: Sprite = &[
    "................BBB...",
    "...............BBBBB..",
    "..w...........DBBkBBB.",
    "..B...........DDBBBBBk",
    "..B...........DDBBwww.",
    "..BB..........ccBww...",
    "...BBDDDDDDBBBccBw....",
    "...BBBDDDDBBBBBBBw....",
    "...BBBBBBBBBBBBBBw....",
    "...BBwwwwwwwwwBBw.....",
    "...BB.BB.....BB.BB....",
    "...BB.BB.....BB.BB....",
    "...ww.ww.....ww.ww....",
];

const STAND_L: Sprite = &[
    "................BBB...",
    "...............BBBBB..",
    "w.............DBBkBBB.",
    ".B............DDBBBBBk",
    ".B............DDBBwww.",
    "..BB..........ccBww...",
    "...BBDDDDDDBBBccBw....",
    "...BBBDDDDBBBBBBBw....",
    "...BBBBBBBBBBBBBBw....",
    "...BBwwwwwwwwwBBw.....",
    "...BB.BB.....BB.BB....",
    "...BB.BB.....BB.BB....",
    "...ww.ww.....ww.ww....",
];

const STAND_R: Sprite = &[
    "................BBB...",
    "...............BBBBB..",
    "....w.........DBBkBBB.",
    "...B..........DDBBBBBk",
    "...B..........DDBBwww.",
    "..BB..........ccBww...",
    "...BBDDDDDDBBBccBw....",
    "...BBBDDDDBBBBBBBw....",
    "...BBBBBBBBBBBBBBw....",
    "...BBwwwwwwwwwBBw.....",
    "...BB.BB.....BB.BB....",
    "...BB.BB.....BB.BB....",
    "...ww.ww.....ww.ww....",
];

const RUN_A: Sprite = &[
    "................BBB...",
    "...............BBBBB..",
    "..............DBBkBBB.",
    "..............DDBBBBBk",
    "..............DDBBwww.",
    "wB............ccBww...",
    ".BBBBDDDDDDBBBccBw....",
    "...BBBDDDDBBBBBBBw....",
    "...BBBBBBBBBBBBBBw....",
    "...BBwwwwwwwwwBBw.....",
    "..BB...........BB.....",
    ".BB.............BB....",
    "ww...............ww...",
];

const RUN_B: Sprite = &[
    "................BBB...",
    "...............BBBBB..",
    "..............DBBkBBB.",
    "..............DDBBBBBk",
    "..............DDBBwww.",
    "wB............ccBww...",
    ".BBBBDDDDDDBBBccBw....",
    "...BBBDDDDBBBBBBBw....",
    "...BBBBBBBBBBBBBBw....",
    "...BBwwwwwwwwwBBw.....",
    "...BBB.......BBB......",
    "...B.BB.....BB.B......",
    "...w..ww...ww..w......",
];

const RUN_C: Sprite = &[
    "................BBB...",
    "...............BBBBB..",
    "..............DBBkBBB.",
    "..............DDBBBBBk",
    "..............DDBBwww.",
    "wB............ccBww...",
    ".BBBBDDDDDDBBBccBw....",
    "...BBBDDDDBBBBBBBw....",
    "...BBBBBBBBBBBBBBw....",
    "...BBwwwwwwwwwBBw.....",
    "....BB......BB........",
    ".....BB....BB.........",
    "......ww..ww..........",
];

const SIT: Sprite = &[
    "..............BBB.....",
    ".............BBBBB....",
    "............DBBkBBB...",
    "............DDBBBBBk..",
    "............DDBBwwww..",
    "............ccBBwp....",
    "...........BccBBwp....",
    "..........BBBBBBw.....",
    ".........BBBBBBBw.....",
    "........BBDDBBBBw.....",
    "........BDDDBBBBw.....",
    ".wBB...BBBDDBBBBw.....",
    "...BBBBBBBBBBwwww.....",
];

const SIT_PANT: Sprite = &[
    "..............BBB.....",
    ".............BBBBB....",
    "............DBBkBBB...",
    "............DDBBBBBk..",
    "............DDBBwwww..",
    "............ccBBwp....",
    "...........BccBBwpp...",
    "..........BBBBBBw.....",
    ".........BBBBBBBw.....",
    "........BBDDBBBBw.....",
    "........BDDDBBBBw.....",
    "..BB...BBBDDBBBBw.....",
    "wBBBBBBBBBBBwwww......",
];

const LIE: Sprite = &[
    "..............BBB.....",
    ".............DBBBB....",
    "........DDDDDDBDBBBk..",
    "wBB...BBDDDDBDBBBwww..",
    "..BBBBBBBBBBBBBBww....",
    "..BBBBBBBBBBBBBBw.....",
    "...BBBBwwwwwwwwwwww...",
];

const SNIFF: Sprite = &[
    "..w...................",
    "..B...................",
    "..B...................",
    "..BB..................",
    "...BBDDDDDDBBB........",
    "...BBBDDDDBBBBBcc.....",
    "...BBBBBBBBBBBBccBB...",
    "...BBwwwwwwwwwBBDBBB..",
    "...BB.BB.....BBDDBkBB.",
    "...BB.BB.....BBDDBBBB.",
    "...ww.ww.....ww...wwwk",
];

const SNIFF_WAG: Sprite = &[
    ".w....................",
    ".B....................",
    "..B...................",
    "..BB..................",
    "...BBDDDDDDBBB........",
    "...BBBDDDDBBBBBcc.....",
    "...BBBBBBBBBBBBccBB...",
    "...BBwwwwwwwwwBBDBBB..",
    "...BB.BB.....BBDDBkBB.",
    "...BB.BB.....BBDDBBBB.",
    "...ww.ww.....ww...wwwk",
];

const RUN: [Sprite; 4] = [RUN_A, RUN_B, RUN_C, RUN_B];
const WAG: [Sprite; 4] = [STAND, STAND_L, STAND, STAND_R];

/// Height of the world in pixels; its width follows the terminal's aspect.
const WORLD_H: i32 = 128;
/// Camera zoom levels, as the number of world pixel rows in view.
const ZOOMS: [f32; 7] = [128.0, 106.0, 88.0, 72.0, 58.0, 46.0, 36.0];
const DEFAULT_ZOOM: usize = 2;
/// World pixels kept clear around the dog at every zoom level.
const CAM_MARGIN: f32 = 3.0;
/// Tallest the dog gets: the tallest sprite plus the running bob.
const DOG_H: f32 = 14.0;
/// Highest the dog leaps when chasing a butterfly.
const MAX_HOP: f32 = 11.0;

/// Coat, patches and cream.
const COATS: [(Rgb, Rgb, Rgb); 4] = [
    (Rgb(200, 134, 70), Rgb(122, 74, 40), Rgb(246, 230, 202)),
    (Rgb(226, 174, 94), Rgb(184, 124, 62), Rgb(250, 236, 200)),
    (Rgb(156, 158, 168), Rgb(86, 88, 100), Rgb(242, 242, 244)),
    (Rgb(136, 90, 58), Rgb(88, 56, 36), Rgb(234, 206, 174)),
];
const COLLARS: [Rgb; 4] = [Rgb(206, 40, 52), Rgb(40, 110, 210), Rgb(40, 160, 90), Rgb(240, 180, 30)];
const EYE: Rgb = Rgb(28, 22, 20);
const TONGUE: Rgb = Rgb(236, 104, 128);

/// Frames for a full day.
const DAY_LEN: f32 = 3600.0;
const NIGHT_TOP: Rgb = Rgb(6, 8, 26);
const NIGHT_BOT: Rgb = Rgb(24, 28, 62);
const NIGHT_TINT: Rgb = Rgb(78, 88, 140);
const DAY_TOP: Rgb = Rgb(70, 140, 225);
const DAY_BOT: Rgb = Rgb(186, 218, 246);
/// Time of day keyframes: (t, sky top, sky bottom, light tint on the scene).
const KEYS: [(f32, Rgb, Rgb, Rgb); 8] = [
    (0.0, NIGHT_TOP, NIGHT_BOT, NIGHT_TINT),
    (0.17, NIGHT_TOP, NIGHT_BOT, NIGHT_TINT),
    (0.23, Rgb(72, 72, 142), Rgb(250, 162, 122), Rgb(232, 184, 172)),
    (0.3, DAY_TOP, DAY_BOT, Rgb::WHITE),
    (0.68, DAY_TOP, DAY_BOT, Rgb::WHITE),
    (0.75, Rgb(82, 60, 132), Rgb(252, 140, 80), Rgb(240, 172, 142)),
    (0.82, NIGHT_TOP, NIGHT_BOT, NIGHT_TINT),
    (1.0, NIGHT_TOP, NIGHT_BOT, NIGHT_TINT),
];

const FAR_HILL: Rgb = Rgb(118, 156, 138);
const MEADOW_TOP: Rgb = Rgb(106, 172, 74);
const MEADOW_BOT: Rgb = Rgb(56, 122, 42);
const BLADES: [Rgb; 4] = [Rgb(92, 170, 64), Rgb(74, 150, 54), Rgb(116, 184, 78), Rgb(62, 136, 48)];
const PETALS: [Rgb; 6] = [
    Rgb(250, 250, 250),
    Rgb(250, 214, 70),
    Rgb(236, 72, 100),
    Rgb(170, 120, 236),
    Rgb(250, 150, 190),
    Rgb(250, 140, 60),
];
const WINGS: [Rgb; 4] = [Rgb(250, 150, 40), Rgb(250, 250, 240), Rgb(250, 220, 60), Rgb(90, 160, 250)];
const WOOD: Rgb = Rgb(164, 122, 78);
const POST: Rgb = Rgb(132, 96, 60);
const TRUNK: Rgb = Rgb(104, 72, 46);
const LEAVES: Rgb = Rgb(52, 112, 52);
const LEAVES_LIT: Rgb = Rgb(84, 150, 70);

#[derive(Clone, Copy, PartialEq)]
enum State {
    Run,
    Stand,
    Sit,
    Nap,
    Sniff,
    Chase,
}

#[derive(Clone, Copy)]
struct Dog {
    /// Left edge of the sprite, in world pixels.
    x: f32,
    /// 1 facing right, -1 facing left.
    dir: f32,
    state: State,
    timer: u32,
    target: f32,
    /// Turn around when the current pause ends.
    turn: bool,
    hop: f32,
    vy: f32,
    anim: u32,
    coat: (Rgb, Rgb, Rgb),
    collar: Rgb,
}

struct Butterfly {
    x: f32,
    y: f32,
    tx: f32,
    ty: f32,
    /// Direction it flees in once the dog gives chase; 0 while wandering.
    flee: f32,
    speed: f32,
    life: u32,
    color: Rgb,
}

struct Blade {
    x: i32,
    y: i32,
    h: i32,
    color: Rgb,
    /// Petal color and whether it is a big (cross-shaped) bloom.
    flower: Option<(Rgb, bool)>,
}

struct Cloud {
    x: f32,
    y: f32,
    v: f32,
    puffs: Vec<(f32, f32, f32)>,
}

struct Tree {
    x: i32,
    trunk: i32,
    r: i32,
}

struct Star {
    x: i32,
    y: i32,
    phase: f32,
}

struct Firefly {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    phase: f32,
}

struct Zzz {
    x: f32,
    y: f32,
    age: u32,
}

/// What part of the world is on screen: top-left corner in world pixels and
/// screen pixels per world pixel.
#[derive(Clone, Copy)]
struct View {
    x: f32,
    y: f32,
    z: f32,
}

impl View {
    fn to_screen(self, x: f32, y: f32) -> (i32, i32) {
        (((x - self.x) * self.z).floor() as i32, ((y - self.y) * self.z).floor() as i32)
    }
}

struct Meadow {
    /// World size in pixels.
    w: i32,
    ph: i32,
    /// Screen size in half-block pixels.
    sw: i32,
    sph: i32,
    horizon: i32,
    fence: i32,
    ground: i32,
    px: Vec<Rgb>,
    out: Vec<Rgb>,
    far: Vec<i32>,
    near: Vec<i32>,
    seed: u32,
    trees: Vec<Tree>,
    clouds: Vec<Cloud>,
    stars: Vec<Star>,
    blades: Vec<Blade>,
    fireflies: Vec<Firefly>,
    zs: Vec<Zzz>,
    dog: Dog,
    butterfly: Option<Butterfly>,
    t: f32,
    tick: u32,
    sky_top: Rgb,
    sky_bot: Rgb,
    tint: Rgb,
    zoom: usize,
    /// Current rows in view, easing towards `ZOOMS[zoom]`.
    view_h: f32,
    /// World x the camera centres on, trailing the dog.
    cam_x: f32,
}

#[inline]
fn hash(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263) ^ seed;
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}

impl Meadow {
    fn new(sw: usize, sh: usize, rng: &mut Rng) -> Meadow {
        let (sw, sph) = (sw as i32, 2 * sh as i32);
        let ph = WORLD_H;
        let w = (WORLD_H * sw / sph.max(1)).max(1);
        // The dog runs close to the horizon so that zooming in on it still
        // shows some sky; the rest of the meadow is foreground.
        let horizon = ph * 62 / 100;
        let ground = horizon + 22;
        let fence = horizon + 8;
        let (a, b, c) = (rng.rangef(0.0, 6.3), rng.rangef(0.0, 6.3), rng.rangef(0.0, 6.3));
        let far = (0..w)
            .map(|x| {
                let x = x as f32;
                let v = 0.55 + 0.25 * (x * 0.02 + a).sin() + 0.2 * (x * 0.05 + b).sin();
                horizon - (ph as f32 * 0.09 * v) as i32
            })
            .collect();
        let near = (0..w)
            .map(|x| horizon - (ph as f32 * 0.03 * (0.5 + 0.5 * (x as f32 * 0.03 + c).sin())) as i32)
            .collect();

        let mut trees = Vec::new();
        let count = (w / 70).clamp(1, 5);
        for i in 0..count {
            let slot = w / count;
            trees.push(Tree {
                x: i * slot + rng.range(slot / 4, slot * 3 / 4),
                trunk: ph / 14 + rng.range(0, 4),
                r: ph / 13 + rng.range(0, 5),
            });
        }

        let clouds = (0..(w / 40).max(1)).map(|_| Self::cloud(rng, w, horizon, true)).collect();
        let stars = (0..(w * horizon / 90).max(1))
            .map(|_| Star {
                x: rng.range(0, w),
                y: rng.range(0, (horizon - 2).max(1)),
                phase: rng.rangef(0.0, 6.3),
            })
            .collect();

        let mut blades: Vec<Blade> = (0..(w * (ph - fence) / 8))
            .map(|_| {
                let y = rng.range(fence + 1, ph + 2);
                // No flowers right in front of the dog, where they would cover it.
                let flower = rng.chance(0.1) && !(ground - 2..ground + 8).contains(&y);
                // Nearer blades stand taller.
                let near = 1.0 + 1.5 * (y - fence) as f32 / (ph - fence) as f32;
                let h = if flower { rng.range(2, 6) } else { rng.range(1, 4) };
                Blade {
                    x: rng.range(0, w),
                    y,
                    h: (h as f32 * near).round() as i32,
                    color: *rng.pick(&BLADES),
                    flower: flower.then(|| (*rng.pick(&PETALS), rng.chance(0.4))),
                }
            })
            .collect();
        blades.sort_by_key(|b| b.y);

        let fireflies = (0..(w / 14).max(1))
            .map(|_| Firefly {
                x: rng.rangef(0.0, w as f32),
                y: rng.rangef((fence - ph / 8) as f32, ground as f32),
                vx: 0.0,
                vy: 0.0,
                phase: rng.rangef(0.0, 6.3),
            })
            .collect();

        let dog = Dog {
            x: rng.rangef(0.0, (w - SPRITE_W).max(1) as f32),
            dir: if rng.chance(0.5) { 1.0 } else { -1.0 },
            state: State::Stand,
            timer: 20,
            target: 0.0,
            turn: false,
            hop: 0.0,
            vy: 0.0,
            anim: 0,
            coat: *rng.pick(&COATS),
            collar: *rng.pick(&COLLARS),
        };
        let cam_x = dog.x + SPRITE_W as f32 / 2.0;

        Meadow {
            w,
            ph,
            sw,
            sph,
            horizon,
            fence,
            ground,
            px: vec![Rgb::BLACK; (w * ph) as usize],
            out: vec![Rgb::BLACK; (sw * sph).max(0) as usize],
            far,
            near,
            seed: rng.next_u64() as u32,
            trees,
            clouds,
            stars,
            blades,
            fireflies,
            zs: Vec::new(),
            dog,
            butterfly: None,
            // Start somewhere in the morning or early afternoon.
            t: rng.rangef(0.3, 0.5),
            tick: 0,
            sky_top: DAY_TOP,
            sky_bot: DAY_BOT,
            tint: Rgb::WHITE,
            zoom: DEFAULT_ZOOM,
            view_h: ZOOMS[DEFAULT_ZOOM],
            cam_x,
        }
    }

    fn cloud(rng: &mut Rng, w: i32, horizon: i32, anywhere: bool) -> Cloud {
        let n = rng.range(3, 6);
        let r = WORLD_H as f32 / 24.0;
        let mut puffs = Vec::new();
        let mut x = 0.0;
        for i in 0..n {
            let big = i > 0 && i < n - 1;
            let pr = r * if big { rng.rangef(1.2, 1.8) } else { rng.rangef(0.8, 1.1) };
            puffs.push((x, -pr * 0.3, pr));
            x += pr * rng.rangef(0.9, 1.3);
        }
        Cloud {
            x: if anywhere { rng.rangef(0.0, w as f32) } else { -x - r },
            y: rng.rangef(r * 2.0, (horizon as f32 * 0.45).max(r * 2.0 + 1.0)),
            v: rng.rangef(0.03, 0.09),
            puffs,
        }
    }

    /// Horizontal range the dog's sprite can occupy.
    fn limits(&self) -> (f32, f32) {
        let lo = 1.0;
        let hi = (self.w - SPRITE_W - 1) as f32;
        (lo, hi.max(lo))
    }

    fn daylight(&self) -> f32 {
        ((self.tint.luma() as f32 - NIGHT_TINT.luma() as f32) / (255.0 - NIGHT_TINT.luma() as f32))
            .clamp(0.0, 1.0)
    }

    #[inline]
    fn shade(&self, c: Rgb) -> Rgb {
        let f = |a: u8, t: u8| (a as u32 * t as u32 / 255) as u8;
        Rgb(f(c.0, self.tint.0), f(c.1, self.tint.1), f(c.2, self.tint.2))
    }

    #[inline]
    fn put(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && x < self.w && y < self.ph {
            self.px[(y * self.w + x) as usize] = c;
        }
    }

    #[inline]
    fn blend(&mut self, x: i32, y: i32, c: Rgb, a: f32) {
        if x >= 0 && y >= 0 && x < self.w && y < self.ph {
            let p = &mut self.px[(y * self.w + x) as usize];
            *p = p.lerp(c, a);
        }
    }

    /// Like `blend`, but on the screen buffer.
    #[inline]
    fn blend_out(&mut self, x: i32, y: i32, c: Rgb, a: f32) {
        if x >= 0 && y >= 0 && x < self.sw && y < self.sph {
            let p = &mut self.out[(y * self.sw + x) as usize];
            *p = p.lerp(c, a);
        }
    }

    // ----- simulation -----

    fn update(&mut self, rng: &mut Rng) {
        self.tick = self.tick.wrapping_add(1);
        self.t = (self.t + 1.0 / DAY_LEN).fract();
        self.update_sky();
        for i in 0..self.clouds.len() {
            let cl = &mut self.clouds[i];
            cl.x += cl.v;
            if cl.x > self.w as f32 + 2.0 {
                self.clouds[i] = Self::cloud(rng, self.w, self.horizon, false);
            }
        }
        let top = (self.fence - self.ph / 8) as f32;
        let bot = (self.ground as f32).max(top);
        for f in self.fireflies.iter_mut() {
            f.vx = (f.vx + rng.rangef(-0.04, 0.04)).clamp(-0.3, 0.3);
            f.vy = (f.vy + rng.rangef(-0.03, 0.03)).clamp(-0.15, 0.15);
            f.x = (f.x + f.vx).rem_euclid(self.w as f32);
            f.y += f.vy;
            if f.y < top || f.y > bot {
                f.vy = -f.vy;
                f.y = f.y.clamp(top, bot);
            }
        }
        self.update_butterfly(rng);
        self.update_dog(rng);
        for z in self.zs.iter_mut() {
            z.age += 1;
            z.y -= 0.18;
            z.x += 0.12 * (z.age as f32 * 0.15).sin();
        }
        self.zs.retain(|z| z.age < 70 && z.y > 0.0);
        // Camera: ease the zoom, settling exactly on the level.
        let goal = ZOOMS[self.zoom];
        self.view_h += (goal - self.view_h) * 0.15;
        if (goal - self.view_h).abs() < 0.05 {
            self.view_h = goal;
        }
        // Only pan once the dog leaves the middle of the frame (every pan
        // repaints the whole screen), and never let it out of frame.
        let center = self.dog.x + SPRITE_W as f32 / 2.0;
        let slack = ((self.view_size().0 - SPRITE_W as f32) / 2.0 - CAM_MARGIN).max(0.0);
        let target = self.cam_x.clamp(center - slack * 0.5, center + slack * 0.5);
        self.cam_x += (target - self.cam_x) * 0.15;
        self.cam_x = self.cam_x.clamp(center - slack, center + slack);
    }

    fn update_sky(&mut self) {
        let t = self.t;
        for win in KEYS.windows(2) {
            let (t0, a0, b0, c0) = win[0];
            let (t1, a1, b1, c1) = win[1];
            if t >= t0 && t <= t1 {
                let f = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
                self.sky_top = a0.lerp(a1, f);
                self.sky_bot = b0.lerp(b1, f);
                self.tint = c0.lerp(c1, f);
                return;
            }
        }
    }

    fn update_butterfly(&mut self, rng: &mut Rng) {
        let (w, tick) = (self.w as f32, self.tick as f32);
        let (lo_y, hi_y) = ((self.horizon - self.ph / 6) as f32, (self.ground - 6) as f32);
        match &mut self.butterfly {
            Some(b) => {
                if b.flee != 0.0 {
                    b.x += b.flee * b.speed;
                    b.y += (tick * 0.4).sin() * 0.5 - 0.12;
                    b.y = b.y.max(2.0);
                } else {
                    b.x += (b.tx - b.x).clamp(-0.5, 0.5);
                    b.y += (b.ty - b.y).clamp(-0.3, 0.3) + (tick * 0.5).sin() * 0.4;
                    if (b.tx - b.x).abs() < 2.0 && (b.ty - b.y).abs() < 2.0 {
                        b.tx = rng.rangef(0.0, w);
                        b.ty = rng.rangef(lo_y, hi_y.max(lo_y + 1.0));
                    }
                    b.life = b.life.saturating_sub(1);
                    if b.life == 0 {
                        // Wander off through the nearest side.
                        b.flee = if b.x < w / 2.0 { -1.0 } else { 1.0 };
                        b.speed = 0.5;
                    }
                }
                if b.x < -6.0 || b.x > w + 6.0 {
                    self.butterfly = None;
                }
            }
            None => {
                if self.daylight() > 0.7 && rng.chance(0.004) {
                    let left = rng.chance(0.5);
                    let hi_y = hi_y.max(lo_y + 1.0);
                    // Enter near the dog so the camera has a chance to see it.
                    let near = self.dog.x + SPRITE_W as f32 / 2.0 + if left { -60.0 } else { 60.0 };
                    self.butterfly = Some(Butterfly {
                        x: near.clamp(-3.0, w + 3.0),
                        y: rng.rangef(lo_y, hi_y),
                        tx: rng.rangef(w * 0.2, w * 0.8),
                        ty: rng.rangef(lo_y, hi_y),
                        flee: 0.0,
                        speed: 0.8,
                        life: rng.range(250, 450) as u32,
                        color: *rng.pick(&WINGS),
                    });
                }
            }
        }
    }

    fn start_run(&self, d: &mut Dog, rng: &mut Rng) {
        let (lo, hi) = self.limits();
        let edge = |dir: f32| if dir > 0.0 { hi } else { lo };
        if (edge(d.dir) - d.x).abs() < 6.0 {
            d.dir = -d.dir;
        }
        let e = edge(d.dir);
        d.target = if rng.chance(0.5) { e } else { d.x + (e - d.x) * rng.rangef(0.3, 0.8) };
        if (d.target - d.x).abs() < 8.0 {
            d.target = e;
        }
        d.state = State::Run;
    }

    fn rest(&self, d: &mut Dog, rng: &mut Rng, at_edge: bool) {
        d.turn = at_edge;
        let nap = if self.daylight() < 0.5 { 0.6 } else { 0.15 };
        let r = rng.f32();
        (d.state, d.timer) = if r < nap {
            (State::Nap, rng.range(220, 420) as u32)
        } else if r < nap + (1.0 - nap) * 0.4 {
            (State::Sit, rng.range(90, 200) as u32)
        } else if r < nap + (1.0 - nap) * 0.8 {
            (State::Sniff, rng.range(40, 100) as u32)
        } else {
            (State::Stand, rng.range(30, 60) as u32)
        };
    }

    fn update_dog(&mut self, rng: &mut Rng) {
        let mut d = self.dog;
        let (lo, hi) = self.limits();
        d.anim = d.anim.wrapping_add(1);
        if d.hop > 0.0 || d.vy > 0.0 {
            d.hop += d.vy;
            d.vy -= 0.3;
            if d.hop <= 0.0 {
                d.hop = 0.0;
                d.vy = 0.0;
            }
        }
        match d.state {
            State::Run => {
                d.x += d.dir * 1.1;
                if (d.target - d.x) * d.dir <= 0.0 {
                    d.x = d.target;
                    let at_edge = d.x <= lo + 0.5 || d.x >= hi - 0.5;
                    if at_edge && !rng.chance(0.35) {
                        (d.state, d.timer, d.turn) = (State::Stand, rng.range(10, 25) as u32, true);
                    } else {
                        self.rest(&mut d, rng, at_edge);
                    }
                }
            }
            State::Chase => {
                d.x = (d.x + d.dir * 1.5).clamp(lo, hi);
                let head = d.x + if d.dir > 0.0 { 19.0 } else { 2.0 };
                match &mut self.butterfly {
                    Some(b) => {
                        if d.hop == 0.0 && (b.x - head).abs() < 5.0 {
                            // Leap at it; it startles and speeds away.
                            d.vy = 2.4;
                            d.hop = 0.1;
                            b.speed = 1.8;
                        }
                        if d.x <= lo || d.x >= hi {
                            (d.state, d.timer, d.turn) = (State::Stand, 40, true);
                        }
                    }
                    None => (d.state, d.timer) = (State::Stand, 30),
                }
            }
            _ => {
                if d.state == State::Nap && d.timer % 30 == 0 {
                    let hx = if d.dir > 0.0 { 17.0 } else { 4.0 };
                    self.zs.push(Zzz { x: d.x + hx, y: (self.ground - LIE.len() as i32) as f32, age: 0 });
                }
                if d.timer > 0 {
                    d.timer -= 1;
                } else {
                    match d.state {
                        // Waking up: sit for a moment first.
                        State::Nap => (d.state, d.timer) = (State::Sit, rng.range(30, 60) as u32),
                        State::Stand => {
                            if d.turn {
                                d.dir = -d.dir;
                                d.turn = false;
                            }
                            self.start_run(&mut d, rng);
                        }
                        _ => (d.state, d.timer) = (State::Stand, rng.range(10, 30) as u32),
                    }
                }
            }
        }
        // A wandering butterfly close by is impossible to resist.
        if matches!(d.state, State::Stand | State::Sit | State::Sniff) && self.daylight() > 0.5 {
            if let Some(b) = &mut self.butterfly {
                let center = d.x + SPRITE_W as f32 / 2.0;
                if b.flee == 0.0 && (b.x - center).abs() < 35.0 && b.x > 0.0 && b.x < self.w as f32 {
                    d.dir = if b.x >= center { 1.0 } else { -1.0 };
                    d.state = State::Chase;
                    d.turn = false;
                    b.flee = d.dir;
                    b.speed = 0.85;
                }
            }
        }
        self.dog = d;
    }

    /// Width and height of the view in world pixels. Zooming in stops short
    /// of the point where the whole dog, leaps included, would not fit.
    fn view_size(&self) -> (f32, f32) {
        let aspect = self.sw as f32 / self.sph as f32;
        let fit_w = (SPRITE_W as f32 + 2.0 * CAM_MARGIN) / aspect;
        let fit_h = DOG_H + MAX_HOP + 2.0 * CAM_MARGIN;
        let vh = self.view_h.max(fit_w).max(fit_h).min(self.ph as f32);
        (vh * aspect, vh)
    }

    fn view(&self) -> View {
        let (vw, vh) = self.view_size();
        let z = self.sph as f32 / vh;
        // Snap to whole screen pixels so the picture only shifts in visible
        // steps instead of every cell's colour creeping with sub-pixel moves.
        let snap = |v: f32| (v * z).round() / z;
        let x = snap((self.cam_x - vw / 2.0).clamp(0.0, (self.w as f32 - vw).max(0.0)));
        // The dog's ground line sits at the same height in every zoom level,
        // unless the camera has to tilt up to keep a leaping dog in frame.
        let top = self.ground as f32 - self.dog.hop - DOG_H - CAM_MARGIN;
        let y = (self.ground as f32 - 0.78 * vh).min(top).clamp(0.0, (self.ph as f32 - vh).max(0.0));
        View { x, y: snap(y), z }
    }

    // ----- drawing -----

    fn draw(&mut self) {
        let (w, ph) = (self.w, self.ph);
        // Sky.
        for y in 0..self.horizon {
            let c = self.sky_top.lerp(self.sky_bot, y as f32 / self.horizon as f32);
            self.px[(y * w) as usize..((y + 1) * w) as usize].fill(c);
        }
        self.draw_sun_moon();
        self.draw_clouds();
        // Hills and meadow.
        let far_c = self.shade(FAR_HILL);
        for x in 0..w {
            let (f, n) = (self.far[x as usize], self.near[x as usize]);
            for y in f.max(0)..n.min(ph) {
                self.put(x, y, far_c);
            }
            for y in n.max(0)..ph {
                // Flat rows (no per-pixel texture): identical neighbouring
                // cells need no colour escapes when the camera pans.
                let t = ((y - n) as f32 / (ph - n).max(1) as f32).powf(0.8);
                let c = self.shade(MEADOW_TOP.lerp(MEADOW_BOT, t));
                self.put(x, y, c);
            }
        }
        self.draw_trees();
        self.draw_fence();
        let split = self.blades.partition_point(|b| b.y < self.ground);
        self.draw_blades(0, split);
        self.draw_dog();
        self.draw_blades(split, self.blades.len());
        self.draw_butterfly();
    }

    fn draw_sun_moon(&mut self) {
        let (w, hz) = (self.w as f32, self.horizon as f32);
        let r = self.ph as f32 / 16.0;
        // Sun from 0.19 to 0.81, moon for the rest of the day.
        let (u, sun) = if (0.19..0.81).contains(&self.t) {
            ((self.t - 0.19) / 0.62, true)
        } else {
            ((self.t - 0.81).rem_euclid(1.0) / 0.38, false)
        };
        let arc = (PI * u).sin();
        let cx = u * (w + 4.0 * r) - 2.0 * r;
        let cy = hz + 1.5 * r - arc * (hz * 0.85);
        let color = if sun {
            Rgb(255, 150, 60).lerp(Rgb(255, 238, 160), (arc * 1.6).min(1.0))
        } else {
            Rgb(236, 232, 212)
        };
        let glow_r = r * if sun { 2.6 } else { 1.8 };
        for y in (cy - glow_r) as i32..=(cy + glow_r) as i32 {
            if y >= self.horizon {
                break;
            }
            for x in (cx - glow_r) as i32..=(cx + glow_r) as i32 {
                let (dx, dy) = (x as f32 - cx, y as f32 - cy);
                let d = (dx * dx + dy * dy).sqrt();
                if d <= r {
                    // Crescent: carve out a shifted disc.
                    let (ex, ey) = (dx - r * 0.55, dy + r * 0.2);
                    if sun || ex * ex + ey * ey > r * r * 0.8 {
                        self.put(x, y, color);
                    }
                } else if d < glow_r {
                    let a = (1.0 - (d - r) / (glow_r - r)) * if sun { 0.4 } else { 0.18 };
                    self.blend(x, y, color, a);
                }
            }
        }
    }

    fn draw_clouds(&mut self) {
        let night = 1.0 - self.daylight();
        let white = self.shade(Rgb(252, 252, 255)).lerp(self.sky_top, 0.4 * night);
        let under = white.lerp(self.sky_bot, 0.25);
        for i in 0..self.clouds.len() {
            let (x0, y0) = (self.clouds[i].x, self.clouds[i].y);
            for p in 0..self.clouds[i].puffs.len() {
                let (px, py, r) = self.clouds[i].puffs[p];
                let (cx, cy) = (x0 + px, y0 + py);
                for y in (cy - r) as i32..=(cy + r) as i32 {
                    if y >= self.horizon {
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

    fn draw_trees(&mut self) {
        for i in 0..self.trees.len() {
            let Tree { x, trunk, r } = self.trees[i];
            let base = self.near[x.clamp(0, self.w - 1) as usize] + 1;
            let tw = (r / 3).max(1);
            let trunk_c = self.shade(TRUNK);
            for y in base - trunk - r / 2..=base {
                for dx in 0..tw {
                    self.put(x - tw / 2 + dx, y, trunk_c);
                }
            }
            let (cx, cy) = (x as f32, (base - trunk - r / 2) as f32);
            let rf = r as f32;
            let blobs = [(0.0, 0.0, 1.0), (-0.75, 0.35, 0.75), (0.75, 0.35, 0.75), (0.0, -0.55, 0.75)];
            let (dark, lit) = (self.shade(LEAVES), self.shade(LEAVES_LIT));
            for y in (cy - 1.4 * rf) as i32..=(cy + 1.2 * rf) as i32 {
                for xx in (cx - 1.6 * rf) as i32..=(cx + 1.6 * rf) as i32 {
                    let (dx, dy) = (xx as f32 - cx, y as f32 - cy);
                    let inside = blobs.iter().any(|&(bx, by, br)| {
                        let (ex, ey) = (dx - bx * rf, dy - by * rf);
                        ex * ex + ey * ey <= (br * rf) * (br * rf)
                    });
                    if inside {
                        let speckle = hash(xx, y, self.seed ^ 0x5eed) % 5 == 0;
                        let c = if dx + dy < -0.3 * rf && !speckle { lit } else if speckle { dark.scale(0.8) } else { dark };
                        self.put(xx, y, c);
                    }
                }
            }
        }
    }

    fn draw_fence(&mut self) {
        let bottom = self.fence;
        let top = bottom - 6;
        let (rail, post) = (self.shade(WOOD), self.shade(POST));
        for x in 0..self.w {
            self.put(x, top + 1, rail);
            self.put(x, top + 3, rail);
        }
        for x in (4..self.w).step_by(9) {
            for y in top..=bottom {
                self.put(x, y, post);
            }
        }
    }

    fn draw_blades(&mut self, from: usize, to: usize) {
        let tick = self.tick as f32;
        let wind = 0.9 + 0.7 * (tick * 0.004).sin();
        let center = self.shade(Rgb(250, 210, 60));
        for i in from..to {
            let (x, y, h, color, flower) = {
                let b = &self.blades[i];
                (b.x, b.y, b.h, b.color, b.flower)
            };
            let sway = (tick * 0.06 + x as f32 * 0.35 + y as f32 * 0.2).sin() * wind * h as f32 / 3.0;
            let color = self.shade(color);
            for j in 0..h {
                let f = if h > 1 { j as f32 / (h - 1) as f32 } else { 1.0 };
                self.put(x + (sway * f).round() as i32, y - j, color);
            }
            if let Some((petal, big)) = flower {
                let (tx, ty) = (x + sway.round() as i32, y - h);
                let petal = self.shade(petal);
                if big {
                    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                        self.put(tx + dx, ty + dy, petal);
                    }
                    self.put(tx, ty, center);
                } else {
                    self.put(tx, ty, petal);
                }
            }
        }
    }

    fn sprite(&self) -> (Sprite, i32) {
        let d = &self.dog;
        let a = d.anim;
        match d.state {
            State::Run | State::Chase => {
                let f = (a / 2 % 4) as usize;
                (RUN[f], if f == 0 && d.hop == 0.0 { -1 } else { 0 })
            }
            State::Stand => (WAG[(a / 3 % 4) as usize], 0),
            State::Sit => (if a / 4 % 2 == 0 { SIT } else { SIT_PANT }, 0),
            State::Sniff => (if a / 5 % 2 == 0 { SNIFF } else { SNIFF_WAG }, 0),
            State::Nap => (LIE, 0),
        }
    }

    fn draw_dog(&mut self) {
        let d = self.dog;
        let (sprite, bob) = self.sprite();
        let (coat, dark, cream) = d.coat;
        let x0 = d.x.round() as i32;
        // Shadow, fainter while airborne.
        let shadow = 0.3 / (1.0 + d.hop * 0.15);
        for x in x0 + 3..x0 + 19 {
            self.blend(x, self.ground + 1, Rgb::BLACK, shadow);
        }
        let bottom = self.ground - d.hop.round() as i32 + bob;
        let top = bottom - sprite.len() as i32 + 1;
        for (ry, row) in sprite.iter().enumerate() {
            for (rx, ch) in row.bytes().enumerate() {
                let c = match ch {
                    b'B' => coat,
                    b'D' => dark,
                    b'w' => cream,
                    b'k' => EYE,
                    b'c' => d.collar,
                    b'p' => TONGUE,
                    _ => continue,
                };
                let c = self.shade(c);
                let sx = if d.dir > 0.0 { rx as i32 } else { SPRITE_W - 1 - rx as i32 };
                self.put(x0 + sx, top + ry as i32, c);
            }
        }
    }

    fn draw_butterfly(&mut self) {
        let Some(b) = &self.butterfly else { return };
        let (x, y, color) = (b.x.round() as i32, b.y.round() as i32, b.color);
        let wing = self.shade(color);
        let body = self.shade(Rgb(50, 36, 30));
        let wy = if self.tick / 3 % 2 == 0 { y - 1 } else { y };
        self.put(x - 1, wy, wing);
        self.put(x + 1, wy, wing);
        self.put(x, y, body);
    }

    /// Resample the visible part of the world onto the screen: box-filtered
    /// when zoomed out, nearest pixel when zoomed in.
    fn project(&mut self, v: View) {
        let inv = 1.0 / v.z;
        let (w, ph) = (self.w, self.ph);
        for sy in 0..self.sph {
            let y0 = v.y + sy as f32 * inv;
            let y1 = y0 + inv;
            for sx in 0..self.sw {
                let x0 = v.x + sx as f32 * inv;
                let x1 = x0 + inv;
                let c = if v.z >= 1.0 {
                    let x = (((x0 + x1) * 0.5) as i32).clamp(0, w - 1);
                    let y = (((y0 + y1) * 0.5) as i32).clamp(0, ph - 1);
                    self.px[(y * w + x) as usize]
                } else {
                    let (mut r, mut g, mut b, mut sum) = (0.0, 0.0, 0.0, 0.0);
                    for y in (y0 as i32).max(0)..(y1.ceil() as i32).min(ph) {
                        let wy = (y1.min(y as f32 + 1.0) - y0.max(y as f32)).max(0.0);
                        for x in (x0 as i32).max(0)..(x1.ceil() as i32).min(w) {
                            let wt = wy * (x1.min(x as f32 + 1.0) - x0.max(x as f32)).max(0.0);
                            let p = self.px[(y * w + x) as usize];
                            r += p.0 as f32 * wt;
                            g += p.1 as f32 * wt;
                            b += p.2 as f32 * wt;
                            sum += wt;
                        }
                    }
                    let k = if sum > 0.0 { 1.0 / sum } else { 0.0 };
                    Rgb((r * k + 0.5) as u8, (g * k + 0.5) as u8, (b * k + 0.5) as u8)
                };
                // Ignore changes too small to see (swaying grass averaged
                // into a pixel, the slow dusk fade): each one would cost a
                // full cell repaint.
                let o = &mut self.out[(sy * self.sw + sx) as usize];
                let diff = o.0.abs_diff(c.0).max(o.1.abs_diff(c.1)).max(o.2.abs_diff(c.2));
                if diff > 3 {
                    *o = c;
                }
            }
        }
    }

    /// Point lights go straight onto the screen so zooming out cannot blur
    /// them away.
    fn draw_lights(&mut self, v: View) {
        let tick = self.tick as f32;
        let night = 1.0 - self.daylight();
        // Stars only come out once the sky is properly dark.
        let dark = ((night - 0.5) * 2.0).clamp(0.0, 1.0);
        if dark > 0.0 {
            for i in 0..self.stars.len() {
                let st = &self.stars[i];
                let a = dark * (0.55 + 0.45 * (tick * 0.07 + st.phase).sin());
                let (wx, wy) = (st.x, st.y);
                // Skip stars hidden behind clouds or the sun and moon's glow.
                let behind = self.px[(wy * self.w + wx) as usize].luma() > self.sky_top.lerp(self.sky_bot, 0.9).luma();
                if !behind {
                    let (x, y) = v.to_screen(wx as f32 + 0.5, wy as f32 + 0.5);
                    self.blend_out(x, y, Rgb(230, 230, 255), a);
                }
            }
        }
        if night > 0.05 {
            let glow = Rgb(210, 255, 110);
            for i in 0..self.fireflies.len() {
                let f = &self.fireflies[i];
                let b = night * (tick * 0.05 + f.phase).sin().max(0.0).powi(3);
                let (x, y) = v.to_screen(f.x, f.y);
                self.blend_out(x, y, glow, b);
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    self.blend_out(x + dx, y + dy, glow, b * 0.25);
                }
            }
        }
    }

    fn draw_zs(&self, c: &mut Canvas, v: View) {
        for z in &self.zs {
            let (x, y) = v.to_screen(z.x, z.y);
            let Some(bg) = c.get(x, y >> 1).map(|cell| cell.bg) else { continue };
            let fade = if z.age > 50 { (70 - z.age) as f32 / 20.0 } else { 1.0 };
            let ch = if z.age < 25 { 'z' } else { 'Z' };
            c.put(x, y >> 1, ch, bg.lerp(Rgb(235, 240, 255), fade));
        }
    }
}

impl Animation for Meadow {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        self.update(rng);
        if self.sw < 1 || self.sph < 1 {
            return;
        }
        self.draw();
        let v = self.view();
        self.project(v);
        self.draw_lights(v);
        c.blit_pixels(&self.out);
        self.draw_zs(c, v);
    }

    fn has_camera(&self) -> bool {
        true
    }

    fn key(&mut self, key: Key) {
        match key {
            Key::Up => self.zoom = (self.zoom + 1).min(ZOOMS.len() - 1),
            Key::Down => self.zoom = self.zoom.saturating_sub(1),
            _ => {}
        }
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    Box::new(Meadow::new(w, h, rng))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fully zoomed in, the whole dog stays on screen through runs, leaps and naps.
    #[test]
    fn camera_keeps_the_dog_in_frame() {
        for (sw, sh) in [(80, 24), (200, 50), (40, 30), (24, 40), (120, 12)] {
            let mut rng = Rng::new(sw as u64 * 7 + sh as u64);
            let mut m = Meadow::new(sw, sh, &mut rng);
            m.zoom = ZOOMS.len() - 1;
            let mut max_hop: f32 = 0.0;
            for _ in 0..20_000 {
                m.update(&mut rng);
                let v = m.view();
                let (vw, vh) = m.view_size();
                let d = m.dog;
                let (sprite, bob) = m.sprite();
                let top = (m.ground - d.hop.round() as i32 + bob) as f32 - sprite.len() as f32 + 1.0;
                max_hop = max_hop.max(d.hop);
                assert!(d.x >= v.x && d.x + SPRITE_W as f32 <= v.x + vw, "{sw}x{sh}: dog cut horizontally");
                assert!(top >= v.y && (m.ground + 1) as f32 <= v.y + vh, "{sw}x{sh}: dog cut vertically");
            }
            assert!(max_hop <= MAX_HOP, "leap of {max_hop} exceeds MAX_HOP");
        }
    }
}
