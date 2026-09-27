//! A fish tank in half-block pixels: light rays through deep water, sandy
//! dunes with pebbles, shells, rocks and a little castle, swaying seaweed,
//! schools of pixel-art fish, a passing shark, a crab and rising bubbles.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;

/// Pixel-art rows facing right; `.` is transparent. Fish use `B` body,
/// `D` dark fins, `L` belly, `S` stripes or spots, `F` tail and `w`/`k` eye.
type Sprite = &'static [&'static str];

struct Species {
    /// Tail spread and tail flicked, alternated while swimming.
    frames: [Sprite; 2],
    /// Color variants for `B`, `D`, `L`, `S` and `F`.
    palettes: &'static [[Rgb; 5]],
    /// Only swims in roomy tanks.
    big: bool,
}

const TETRA: Species = Species {
    frames: [
        &[
            "....DD...",
            "F..BBBBB.",
            "FFSSSSSkB",
            "F..LLLLL.",
            ".....D...",
        ],
        &[
            "....DD...",
            ".F.BBBBB.",
            ".FSSSSSkB",
            ".F.LLLLL.",
            ".....D...",
        ],
    ],
    palettes: &[
        [Rgb(176, 196, 214), Rgb(130, 150, 170), Rgb(226, 52, 62), Rgb(60, 206, 244), Rgb(196, 206, 220)],
        [Rgb(236, 196, 70), Rgb(196, 150, 40), Rgb(250, 230, 150), Rgb(250, 140, 40), Rgb(240, 180, 60)],
        [Rgb(130, 214, 120), Rgb(80, 160, 80), Rgb(210, 240, 190), Rgb(40, 130, 70), Rgb(150, 220, 130)],
        [Rgb(250, 150, 190), Rgb(210, 100, 150), Rgb(255, 214, 230), Rgb(190, 60, 130), Rgb(250, 170, 200)],
    ],
    big: false,
};

const CLOWN: Species = Species {
    frames: [
        &[
            ".....DDD....",
            "F...BSBBBSB.",
            "FF.BBSBBBSkB",
            "FFBSBSBBBSBB",
            "F...BSBBBSB.",
            ".....D...D..",
        ],
        &[
            ".....DDD....",
            ".F..BSBBBSB.",
            ".FFBBSBBBSkB",
            ".FFSBSBBBSBB",
            ".F..BSBBBSB.",
            ".....D...D..",
        ],
    ],
    palettes: &[
        [Rgb(250, 122, 30), Rgb(40, 24, 20), Rgb(250, 122, 30), Rgb(250, 250, 250), Rgb(240, 104, 26)],
        [Rgb(160, 30, 36), Rgb(60, 14, 16), Rgb(160, 30, 36), Rgb(250, 236, 220), Rgb(140, 24, 30)],
        [Rgb(250, 190, 40), Rgb(200, 110, 20), Rgb(250, 190, 40), Rgb(250, 250, 240), Rgb(250, 160, 30)],
    ],
    big: false,
};

const ANGEL: Species = Species {
    frames: [
        &[
            "....D......",
            "....DD.....",
            "....DDB....",
            "...DBBBB...",
            "F..SBSBBB..",
            "FFBSBSBBBkB",
            "FFBSBSBBBBB",
            "F..SBSBBL..",
            "...DBSBL...",
            "....DDL....",
            "....DD.....",
            "....D......",
        ],
        &[
            "....D......",
            "....DD.....",
            "....DDB....",
            "...DBBBB...",
            ".F.SBSBBB..",
            ".FBSBSBBBkB",
            ".FBSBSBBBBB",
            ".F.SBSBBL..",
            "...DBSBL...",
            "....DDL....",
            "....DD.....",
            "....D......",
        ],
    ],
    palettes: &[
        [Rgb(226, 230, 236), Rgb(150, 156, 170), Rgb(250, 250, 250), Rgb(40, 40, 50), Rgb(200, 206, 216)],
        [Rgb(250, 200, 60), Rgb(220, 150, 40), Rgb(250, 236, 150), Rgb(230, 120, 30), Rgb(250, 190, 60)],
        [Rgb(90, 90, 110), Rgb(40, 40, 56), Rgb(150, 150, 170), Rgb(20, 20, 30), Rgb(80, 80, 100)],
    ],
    big: false,
};

const PUFFER: Species = Species {
    frames: [
        &[
            "....DDD...",
            "..BBBBBBB.",
            "F.BSBBSBwk",
            "FBBBBBBBBB",
            "FBSBBSBBBB",
            "F.LLLLLLL.",
            "...LLLLL..",
            "....D.D...",
        ],
        &[
            "....DDD...",
            "..BBBBBBB.",
            ".FBSBBSBwk",
            ".FBBBBBBBB",
            ".FSBBSBBBB",
            ".FLLLLLLL.",
            "...LLLLL..",
            "....D.D...",
        ],
    ],
    palettes: &[
        [Rgb(230, 200, 80), Rgb(170, 140, 50), Rgb(250, 244, 210), Rgb(120, 90, 40), Rgb(210, 180, 70)],
        [Rgb(140, 190, 120), Rgb(90, 130, 80), Rgb(236, 240, 220), Rgb(50, 80, 50), Rgb(120, 170, 100)],
        [Rgb(236, 150, 90), Rgb(180, 100, 60), Rgb(252, 230, 210), Rgb(130, 70, 40), Rgb(220, 130, 80)],
    ],
    big: false,
};

const TANG: Species = Species {
    frames: [
        &[
            "......DDDD....",
            "F...DDBBBBD...",
            "FF.BBSSSSBBB..",
            "FFBBSBBBBSBkB.",
            "FFBBSSBBBBBBBB",
            "FF.BBBSSBBBB..",
            "F...DDBBBBD...",
            "......DDD.....",
        ],
        &[
            "......DDDD....",
            ".F..DDBBBBD...",
            ".FFBBSSSSBBB..",
            ".FFBSBBBBSBkB.",
            ".FFBSSBBBBBBBB",
            ".FFBBBSSBBBB..",
            ".F..DDBBBBD...",
            "......DDD.....",
        ],
    ],
    palettes: &[
        [Rgb(40, 110, 230), Rgb(30, 60, 150), Rgb(40, 110, 230), Rgb(16, 20, 40), Rgb(250, 210, 40)],
        [Rgb(250, 220, 40), Rgb(220, 180, 30), Rgb(250, 220, 40), Rgb(250, 240, 150), Rgb(250, 250, 230)],
        [Rgb(150, 90, 210), Rgb(90, 50, 150), Rgb(150, 90, 210), Rgb(250, 200, 60), Rgb(250, 200, 60)],
    ],
    big: false,
};

const GOLDFISH: Species = Species {
    frames: [
        &[
            "......DD......",
            "FF...BBBBB....",
            "FFF.BBBBBBBB..",
            ".FFBBBBBBBwkB.",
            "..FBBBBBBBBBBB",
            ".FFBBLLLLLBBB.",
            "FFF.LLLLLLL...",
            "FF....D..D....",
        ],
        &[
            "......DD......",
            ".....BBBBB....",
            "F...BBBBBBBB..",
            "FFFBBBBBBBwkB.",
            ".FFBBBBBBBBBBB",
            "FFFBBLLLLLBBB.",
            "F...LLLLLLL...",
            "......D..D....",
        ],
    ],
    palettes: &[
        [Rgb(250, 130, 30), Rgb(230, 90, 20), Rgb(250, 190, 90), Rgb(0, 0, 0), Rgb(250, 150, 60)],
        [Rgb(236, 236, 230), Rgb(230, 80, 40), Rgb(250, 250, 250), Rgb(0, 0, 0), Rgb(240, 110, 70)],
        [Rgb(240, 60, 40), Rgb(190, 30, 30), Rgb(250, 150, 110), Rgb(0, 0, 0), Rgb(250, 100, 80)],
    ],
    big: false,
};

const GROUPER: Species = Species {
    frames: [
        &[
            "..........DDDD......",
            "F......DDDDDDDDD....",
            "FF...BBBBBBBBBBBBB..",
            "FFF.BBSBBBSBBBBBwkB.",
            "FFFBBBBBBBBBSBBBBBBB",
            "FFFBBSBBSBBBBBBBBBBD",
            "FFF.BBBBBBSBBBBBBBB.",
            "FF...LLLLLLLLLLLLL..",
            "F.......LLLLLLLL....",
            "........DD...DD.....",
        ],
        &[
            "..........DDDD......",
            ".......DDDDDDDDD....",
            "F....BBBBBBBBBBBBB..",
            "FF..BBSBBBSBBBBBwkB.",
            "FFFBBBBBBBBBSBBBBBBB",
            "FFFBBSBBSBBBBBBBBBBD",
            "FF..BBBBBBSBBBBBBBB.",
            "F....LLLLLLLLLLLLL..",
            "........LLLLLLLL....",
            "........DD...DD.....",
        ],
    ],
    palettes: &[
        [Rgb(120, 110, 90), Rgb(80, 72, 60), Rgb(196, 186, 160), Rgb(70, 60, 50), Rgb(100, 92, 76)],
        [Rgb(90, 130, 160), Rgb(50, 80, 110), Rgb(190, 214, 226), Rgb(40, 60, 90), Rgb(80, 116, 146)],
        [Rgb(200, 80, 60), Rgb(140, 50, 40), Rgb(240, 180, 150), Rgb(250, 220, 200), Rgb(180, 70, 50)],
    ],
    big: true,
};

const SPECIES: [&Species; 7] = [&TETRA, &CLOWN, &ANGEL, &PUFFER, &TANG, &GOLDFISH, &GROUPER];

const SHARK: Species = Species {
    frames: [
        &[
            "..................DD..............",
            ".................DDD..............",
            "DD..............DDDD..............",
            "DDD............DDDDD..............",
            ".DDD.......BBBBBBBBBBBBBBB........",
            "..DDDBBBBBBBBBBBBBBBBBBSBSBBBB....",
            "...DDBBBBBBBBBBBBBBBBBBSBSBBkBBBB.",
            "...DDLLLLLLLLLLLLLLLLLLLLLLLLLBBBB",
            "..DDD...LLLLLLLLLLLLLLLLLLLLSSSL..",
            ".DDD.........DDDD....LLLLLLL......",
            "DD..............DDD...............",
        ],
        &[
            "..................DD..............",
            ".................DDD..............",
            "................DDDD..............",
            ".DD............DDDDD..............",
            ".DDD.......BBBBBBBBBBBBBBB........",
            "..DDDBBBBBBBBBBBBBBBBBBSBSBBBB....",
            "...DDBBBBBBBBBBBBBBBBBBSBSBBkBBBB.",
            "..DDDLLLLLLLLLLLLLLLLLLLLLLLLLBBBB",
            ".DDD....LLLLLLLLLLLLLLLLLLLLSSSL..",
            ".DD..........DDDD....LLLLLLL......",
            "................DDD...............",
        ],
    ],
    palettes: &[[Rgb(126, 138, 156), Rgb(96, 106, 124), Rgb(222, 226, 232), Rgb(70, 76, 90), Rgb(0, 0, 0)]],
    big: true,
};

/// `R` shell, `r` legs and joints, `k` eyes; claws open and closed.
const CRAB: [Sprite; 2] = [
    &[
        "R.R.......R.R",
        "RRR.k...k.RRR",
        ".R..R...R..R.",
        "..RRRRRRRRR..",
        ".RRRRRRRRRRR.",
        "r.r.r...r.r.r",
    ],
    &[
        ".RR.......RR.",
        "RRR.k...k.RRR",
        ".R..R...R..R.",
        "..RRRRRRRRR..",
        ".RRRRRRRRRRR.",
        ".r.r.....r.r.",
    ],
];
const CRAB_W: i32 = 13;

/// `G` stone, `g` mortar, `r`/`R` roof, `k` openings.
const CASTLE: Sprite = &[
    "..r.........r..",
    ".rrR.......rrR.",
    ".rrR.......rrR.",
    "rrrRR.....rrrRR",
    "GGGGG.....GGGGG",
    "GGkGG.....GGkGG",
    "GGkGGG.G.GGGkGG",
    "GGGGGGGGGGGGGGG",
    "gGGgGGgGGGgGGgG",
    "GGGGGGkkkGGGGGG",
    "GGgGGkkkkkGGgGG",
    "GGGGGkkkkkGGGGG",
    "gGGGGkkkkkGGGGg",
];

const WATER_TOP: Rgb = Rgb(26, 104, 160);
const WATER_MID: Rgb = Rgb(10, 54, 104);
const WATER_BOT: Rgb = Rgb(4, 20, 52);
const RAY: Rgb = Rgb(110, 190, 225);
const SURFACE: Rgb = Rgb(160, 220, 245);
const SAND: Rgb = Rgb(206, 182, 128);
const SAND_DARK: Rgb = Rgb(150, 126, 84);
const BUBBLE: Rgb = Rgb(180, 226, 248);
const EYE: Rgb = Rgb(16, 16, 24);
const EYE_WHITE: Rgb = Rgb(245, 245, 245);
const CRAB_SHELL: Rgb = Rgb(226, 78, 48);
const CRAB_LEG: Rgb = Rgb(180, 54, 36);
const STONE: Rgb = Rgb(150, 142, 150);
const ROOF: Rgb = Rgb(196, 90, 70);
const WEED_TIP: Rgb = Rgb(150, 226, 110);
const WEED_COLORS: [Rgb; 4] = [Rgb(30, 120, 60), Rgb(56, 140, 50), Rgb(24, 100, 80), Rgb(90, 120, 40)];
const PEBBLES: [Rgb; 6] = [
    Rgb(120, 116, 110),
    Rgb(170, 160, 150),
    Rgb(110, 84, 60),
    Rgb(230, 226, 214),
    Rgb(100, 120, 140),
    Rgb(170, 100, 80),
];

struct Fish {
    sp: &'static Species,
    colors: [Rgb; 5],
    dir: f32,
    /// Left edge and top of the sprite, in pixels.
    x: f32,
    y: f32,
    speed: f32,
    phase: f32,
}

struct Bubble {
    x: f32,
    y: f32,
    age: u32,
    phase: f32,
}

struct Weed {
    x: i32,
    height: i32,
    phase: f32,
    color: Rgb,
    /// Broad kelp with side leaves rather than a thin strand.
    kelp: bool,
}

struct Crab {
    x: f32,
    dir: f32,
    pause: u32,
}

struct Aquarium {
    w: i32,
    /// Height in pixels, twice the rows.
    ph: i32,
    px: Vec<Rgb>,
    /// Water color per pixel row.
    water: Vec<Rgb>,
    /// Sand, rocks and ornaments, drawn over the water every frame.
    scenery: Vec<Option<Rgb>>,
    sand_top: Vec<i32>,
    rays: Vec<f32>,
    fish: Vec<Fish>,
    shark: Option<Fish>,
    bubbles: Vec<Bubble>,
    weeds: Vec<Weed>,
    vents: Vec<(i32, i32)>,
    crab: Option<Crab>,
    t: u32,
}

#[inline]
fn hash(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263) ^ seed;
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}

/// Call `f(x, y, ch)` for every opaque pixel, mirrored when `flip`.
fn each_pixel(s: Sprite, flip: bool, mut f: impl FnMut(i32, i32, u8)) {
    let sw = s.first().map_or(0, |r| r.len()) as i32;
    for (ry, row) in s.iter().enumerate() {
        for (rx, ch) in row.bytes().enumerate() {
            if ch != b'.' {
                let x = if flip { sw - 1 - rx as i32 } else { rx as i32 };
                f(x, ry as i32, ch);
            }
        }
    }
}

fn size(s: Sprite) -> (i32, i32) {
    (s.first().map_or(0, |r| r.len()) as i32, s.len() as i32)
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    let (w, ph) = (w as i32, 2 * h as i32);
    let n = (w * ph).max(0) as usize;
    let sand_h = (ph as f32 / 7.0).max(2.0);
    let base = ph as f32 - sand_h;
    let (p1, p2) = (rng.rangef(0.0, 6.28), rng.rangef(0.0, 6.28));
    let sand_top: Vec<i32> = (0..w)
        .map(|x| {
            let x = x as f32;
            let y = base + (x * 0.05 + p1).sin() * 1.6 + (x * 0.13 + p2).sin() * 0.7;
            (y.round() as i32).min(ph - 1).max(1)
        })
        .collect();
    let mut a = Aquarium {
        w,
        ph,
        px: vec![Rgb::BLACK; n],
        water: (0..ph)
            .map(|y| Rgb::gradient(&[WATER_TOP, WATER_MID, WATER_BOT], y as f32 / ph.max(1) as f32))
            .collect(),
        scenery: vec![None; n],
        sand_top,
        rays: vec![0.0; (w + ph / 3 + 1).max(0) as usize],
        fish: Vec::new(),
        shark: None,
        bubbles: Vec::new(),
        weeds: Vec::new(),
        vents: Vec::new(),
        crab: None,
        t: 0,
    };
    a.build_scenery(rng);
    for _ in 0..(w / 9).max(1) {
        let kelp = rng.chance(0.4);
        a.weeds.push(Weed {
            x: rng.range(0, w),
            height: rng.range(4, (ph / 2).max(5)),
            phase: rng.rangef(0.0, 6.28),
            color: *rng.pick(&WEED_COLORS),
            kelp,
        });
    }
    if w >= 20 && h >= 8 {
        a.crab = Some(Crab { x: rng.rangef(0.0, (w - CRAB_W) as f32), dir: 1.0, pause: 0 });
    }
    let count = (w as usize * h / 260).clamp(2, 40);
    for _ in 0..count {
        let mut f = a.spawn_fish(rng);
        f.x = rng.rangef(0.0, w as f32);
        a.fish.push(f);
    }
    Box::new(a)
}

impl Aquarium {
    fn floor_at(&self, x: i32) -> i32 {
        if self.w <= 0 {
            return self.ph;
        }
        self.sand_top[x.clamp(0, self.w - 1) as usize]
    }

    /// Highest sand surface under `x..x + width`.
    fn floor_under(&self, x: i32, width: i32) -> i32 {
        (x..x + width).map(|xx| self.floor_at(xx)).min().unwrap_or(self.ph)
    }

    fn set_scenery(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && x < self.w && y < self.ph {
            self.scenery[(y * self.w + x) as usize] = Some(c);
        }
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

    /// Sand, rocks, a castle and small ornaments. Static, so drawn once.
    fn build_scenery(&mut self, rng: &mut Rng) {
        let (w, ph) = (self.w, self.ph);
        let seed = rng.next_u64() as u32;
        let big = w >= 60 && ph >= 40;

        // Rocks and the castle stand half buried, so they go in first.
        for _ in 0..(w / 45).max(1) {
            let (rx, ry) = (rng.rangef(2.5, 6.5), rng.rangef(2.0, 4.5));
            let cx = rng.rangef(0.0, w as f32);
            let cy = self.floor_at(cx as i32) as f32 + 1.0;
            let tone = rng.rangef(0.8, 1.15);
            for y in (cy - ry) as i32..=(cy + ry) as i32 {
                for x in (cx - rx) as i32..=(cx + rx) as i32 {
                    let (dx, dy) = ((x as f32 - cx) / rx, (y as f32 - cy) / ry);
                    if dx * dx + dy * dy <= 1.0 {
                        let lit = 1.0 - (dx + dy) * 0.25;
                        let speck = if hash(x, y, seed) % 6 == 0 { 0.85 } else { 1.0 };
                        self.set_scenery(x, y, Rgb(104, 104, 114).scale(tone * lit * speck));
                    }
                }
            }
        }
        if big {
            let (cw, chh) = size(CASTLE);
            let x0 = rng.range(4, (w - cw - 4).max(5));
            let y0 = self.floor_under(x0, cw) + 2 - chh;
            each_pixel(CASTLE, false, |x, y, ch| {
                let c = match ch {
                    b'G' => STONE,
                    b'g' => STONE.scale(0.72),
                    b'r' => ROOF,
                    b'R' => ROOF.scale(0.7),
                    _ => Rgb(20, 24, 40),
                };
                self.set_scenery(x0 + x, y0 + y, c);
            });
            // Bubbles stream out of the door.
            self.vents.push((x0 + cw / 2, y0 + chh - 3));
        }

        for x in 0..w {
            let top = self.floor_at(x);
            for y in top..ph {
                let depth = (y - top) as f32 / (ph - top).max(1) as f32;
                let mut c = if y == top { SAND.lerp(Rgb::WHITE, 0.15) } else { SAND.lerp(SAND_DARK, depth * 0.7) };
                match hash(x, y, seed) % 9 {
                    0 => c = c.scale(0.86),
                    1 => c = c.lerp(Rgb::WHITE, 0.12),
                    _ => {}
                }
                // Keep what already pokes out of the sand in front of it.
                let i = (y * w + x) as usize;
                if self.scenery[i].is_none() || y > top + 1 {
                    self.scenery[i] = Some(c);
                }
            }
        }

        for _ in 0..w / 5 {
            let x = rng.range(0, w);
            let y = self.floor_at(x) + rng.range(0, 3);
            let c = *rng.pick(&PEBBLES);
            self.set_scenery(x, y, c);
            if rng.chance(0.4) {
                self.set_scenery(x + 1, y, c.scale(0.8));
            }
        }
        for _ in 0..w / 30 {
            let x = rng.range(0, w - 3);
            let y = self.floor_under(x, 3) - 1;
            let c = *rng.pick(&[Rgb(246, 214, 200), Rgb(250, 236, 220), Rgb(236, 180, 170)]);
            // A scallop: fanned top edge over a solid base.
            self.set_scenery(x, y, c);
            self.set_scenery(x + 2, y, c);
            for dx in 0..3 {
                self.set_scenery(x + dx, y + 1, c.scale(0.85));
            }
            self.set_scenery(x + 1, y, c.lerp(Rgb::WHITE, 0.4));
        }
        for _ in 0..w / 40 {
            let x = rng.range(1, w - 1);
            let y = self.floor_under(x - 1, 3);
            let c = *rng.pick(&[Rgb(240, 120, 60), Rgb(236, 90, 110), Rgb(250, 170, 60)]);
            for (dx, dy) in [(0, -1), (-1, 0), (0, 0), (1, 0), (-1, 1), (1, 1)] {
                self.set_scenery(x + dx, y + dy, c);
            }
            self.set_scenery(x, y, c.lerp(Rgb::WHITE, 0.3));
        }
        for _ in 0..(w / 40).max(1) {
            let x = rng.range(0, w);
            self.vents.push((x, self.floor_at(x) - 1));
        }
    }

    fn spawn(&self, rng: &mut Rng, sp: &'static Species) -> Fish {
        let (fw, fh) = size(sp.frames[0]);
        let dir = if rng.chance(0.5) { 1.0 } else { -1.0 };
        let floor = self.sand_top.iter().copied().min().unwrap_or(self.ph) as f32;
        let y = rng.rangef(3.0, (floor - fh as f32 - 1.0).max(3.0));
        let x = if dir > 0.0 { -(fw as f32) - rng.rangef(0.0, 20.0) } else { self.w as f32 + rng.rangef(0.0, 20.0) };
        let size_speed = if fh > 8 { 0.6 } else { 1.0 };
        Fish {
            sp,
            colors: *rng.pick(sp.palettes),
            dir,
            x,
            y,
            speed: rng.rangef(0.08, 0.45) * size_speed,
            phase: rng.rangef(0.0, 6.28),
        }
    }

    fn spawn_fish(&self, rng: &mut Rng) -> Fish {
        // Small tanks only get the small fish.
        let roomy = self.w >= 60 && self.ph >= 40;
        let sp = loop {
            let sp = *rng.pick(&SPECIES);
            if roomy || !sp.big {
                break sp;
            }
        };
        self.spawn(rng, sp)
    }

    fn draw_fish(&mut self, f: &Fish, t: f32) {
        // Faster fish beat their tails faster.
        let frame = ((t * (0.05 + f.speed * 0.5) + f.phase) as i32 & 1) as usize;
        let x0 = f.x.round() as i32;
        let y0 = (f.y + (t * 0.07 + f.phase).sin() * 0.8).round() as i32;
        let colors = f.colors;
        each_pixel(f.sp.frames[frame], f.dir < 0.0, |x, y, ch| {
            let c = match ch {
                b'B' => colors[0],
                b'D' => colors[1],
                b'L' => colors[2],
                b'S' => colors[3],
                b'F' => colors[4],
                b'w' => EYE_WHITE,
                _ => EYE,
            };
            self.put(x0 + x, y0 + y, c);
        });
    }

    fn draw_water(&mut self, t: f32) {
        let (w, ph) = (self.w, self.ph);
        // Slow diagonal light rays that fade with depth.
        for (u, r) in self.rays.iter_mut().enumerate() {
            let u = u as f32;
            let s = ((u * 0.1 + t * 0.011).sin() + (u * 0.043 - t * 0.006).sin()) * 0.5;
            *r = s.max(0.0).powi(3) * 0.36;
        }
        let ray_depth = ph as f32 * 0.7;
        for y in 0..ph {
            let base = self.water[y as usize];
            let fall = (1.0 - y as f32 / ray_depth).max(0.0);
            let row = (y * w) as usize;
            for x in 0..w {
                let i = row + x as usize;
                self.px[i] = match self.scenery[i] {
                    Some(c) => c,
                    // Coarse steps: a cell is only resent when its level changes.
                    None if fall > 0.0 => {
                        base.lerp(RAY, (self.rays[(x + y / 3) as usize] * fall * 14.0).round() / 14.0)
                    }
                    None => base,
                };
            }
        }
        // Rippling surface in three shades, so only wave edges change.
        for x in 0..w {
            let v = (x as f32 * 0.35 + t * 0.1).sin() + (x as f32 * 0.13 - t * 0.06).sin();
            let a = if v > 0.6 { 0.7 } else if v > -0.4 { 0.45 } else { 0.25 };
            self.blend(x, 0, SURFACE, a);
            if v > 1.2 {
                self.blend(x, 1, SURFACE, 0.3);
            }
        }
    }

    fn draw_weeds(&mut self, t: f32) {
        for i in 0..self.weeds.len() {
            let Weed { x: wx, height, phase, color, kelp } = self.weeds[i];
            let base = self.floor_at(wx) + 1;
            let mut prev = wx;
            for j in 0..height {
                let k = j as f32 / height as f32;
                let sway = (t * 0.045 + phase + j as f32 * 0.16).sin() * k * 3.2;
                let x = wx + sway.round() as i32;
                let y = base - j;
                let c = color.lerp(WEED_TIP, k * 0.55);
                // Keep the strand joined where it leans more than a pixel.
                for xx in prev.min(x) + 1..prev.max(x) {
                    self.put(xx, y, c);
                }
                self.put(x, y, c);
                if kelp {
                    if k < 0.75 {
                        self.put(x + 1, y, c.scale(0.8));
                    }
                    if j % 4 == 2 && k < 0.9 {
                        let side = if j % 8 == 2 { -1 } else { 2 };
                        let leaf = c.lerp(WEED_TIP, 0.3);
                        self.put(x + side, y, leaf);
                        self.put(x + side + side.signum(), y - 1, leaf);
                    }
                }
                prev = x;
            }
        }
    }

    fn update_crab(&mut self, rng: &mut Rng) {
        let w = self.w;
        let Some(cr) = &mut self.crab else { return };
        if cr.pause > 0 {
            cr.pause -= 1;
        } else {
            cr.x += cr.dir * 0.12;
            if cr.x < 0.0 || cr.x > (w - CRAB_W) as f32 {
                cr.dir = -cr.dir;
                cr.x = cr.x.min((w - CRAB_W) as f32).max(0.0);
            }
            if rng.chance(0.006) {
                cr.pause = rng.range(20, 100) as u32;
            }
        }
    }

    fn draw_crab(&mut self) {
        let Some(cr) = &self.crab else { return };
        let walking = cr.pause == 0;
        let x0 = cr.x.round() as i32;
        let frame = if walking && (self.t / 6) % 2 == 0 { 0 } else { 1 };
        let y0 = self.floor_under(x0, CRAB_W) - size(CRAB[frame]).1;
        each_pixel(CRAB[frame], false, |x, y, ch| {
            let c = match ch {
                b'R' => CRAB_SHELL,
                b'r' => CRAB_LEG,
                _ => EYE,
            };
            self.put(x0 + x, y0 + y, c);
        });
    }

    fn draw_bubbles(&mut self, t: f32) {
        for i in 0..self.bubbles.len() {
            let Bubble { x, y, age, phase } = self.bubbles[i];
            let x = (x + (t * 0.09 + phase).sin() * 0.7).round() as i32;
            let y = y.round() as i32;
            let hi = BUBBLE.lerp(Rgb::WHITE, 0.6);
            match age {
                0..=29 => self.blend(x, y, BUBBLE, 0.8),
                30..=89 => {
                    self.blend(x, y, hi, 0.9);
                    self.blend(x + 1, y, BUBBLE, 0.55);
                    self.blend(x, y + 1, BUBBLE, 0.55);
                    self.blend(x + 1, y + 1, BUBBLE, 0.4);
                }
                _ => {
                    // A ring with a see-through middle and a glint.
                    for (dx, dy) in [(0, -1), (1, 0), (-1, 1), (0, 1), (1, 1), (-1, 0)] {
                        self.blend(x + dx, y + dy, BUBBLE, 0.65);
                    }
                    self.blend(x - 1, y - 1, hi, 0.5);
                    self.blend(x + 1, y - 1, BUBBLE, 0.4);
                    self.blend(x, y, BUBBLE, 0.12);
                }
            }
        }
    }
}

impl Animation for Aquarium {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        let (w, ph) = (self.w, self.ph);
        if w <= 0 || ph <= 0 {
            return;
        }
        self.t += 1;
        let t = self.t as f32;

        // Fish swim across, blowing the odd bubble, and respawn off screen.
        for i in 0..self.fish.len() {
            let f = &mut self.fish[i];
            f.x += f.dir * f.speed;
            let (fw, fh) = size(f.sp.frames[0]);
            let gone = (f.dir > 0.0 && f.x > w as f32 + 1.0) || (f.dir < 0.0 && f.x < -(fw as f32) - 1.0);
            if rng.chance(0.006) {
                let mouth = if f.dir > 0.0 { f.x + fw as f32 } else { f.x - 1.0 };
                let (bx, by) = (mouth, f.y + (fh / 2) as f32);
                self.bubbles.push(Bubble { x: bx, y: by, age: 0, phase: rng.rangef(0.0, 6.28) });
            }
            if gone {
                self.fish[i] = self.spawn_fish(rng);
            }
        }
        if self.shark.is_none() && w >= 30 && ph >= 24 && rng.chance(0.0015) {
            let mut s = self.spawn(rng, &SHARK);
            s.speed = 0.18;
            self.shark = Some(s);
        }
        if let Some(s) = &mut self.shark {
            s.x += s.dir * s.speed;
            let sw = size(SHARK.frames[0]).0 as f32;
            if (s.dir > 0.0 && s.x > w as f32 + 1.0) || (s.dir < 0.0 && s.x < -sw - 1.0) {
                self.shark = None;
            }
        }
        self.update_crab(rng);

        // Bubbles from vents, rising and growing until they reach the surface.
        for i in 0..self.vents.len() {
            let (vx, vy) = self.vents[i];
            if (self.t / 40 + vx as u32) % 3 == 0 && rng.chance(0.08) {
                self.bubbles.push(Bubble { x: vx as f32, y: vy as f32, age: 0, phase: rng.rangef(0.0, 6.28) });
            }
        }
        self.bubbles.retain_mut(|b| {
            b.age += 1;
            b.y -= 0.28;
            b.y >= 2.0
        });

        self.draw_water(t);
        self.draw_weeds(t);
        self.draw_crab();
        let fish = std::mem::take(&mut self.fish);
        for f in &fish {
            self.draw_fish(f, t);
        }
        self.fish = fish;
        if let Some(s) = self.shark.take() {
            self.draw_fish(&s, t);
            self.shark = Some(s);
        }
        self.draw_bubbles(t);
        c.blit_pixels(&self.px);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprites_are_rectangular() {
        let fish = SPECIES.iter().chain([&&SHARK]).flat_map(|s| s.frames);
        for s in fish.chain(CRAB).chain([CASTLE]) {
            assert!(s.iter().all(|r| r.len() == s[0].len()), "ragged sprite: {s:?}");
        }
        for sp in SPECIES.iter().chain([&&SHARK]) {
            assert_eq!(size(sp.frames[0]), size(sp.frames[1]));
        }
    }
}
