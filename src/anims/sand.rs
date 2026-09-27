//! Falling sand: wandering emitters pour grains whose hue drifts over time,
//! building layered dunes. When the box fills up, the floor opens, the sand
//! drains away and a new round begins.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;

const EMPTY: u32 = 0;

struct Emitter {
    x: f32,
    vx: f32,
    hue: f32,
}

#[derive(PartialEq)]
enum Phase {
    Pour,
    /// Floor open; the value is the current half-width of the hole.
    Drain(f32),
}

struct Sand {
    w: usize,
    h: usize,
    /// Packed 0x01RRGGBB per grain, `EMPTY` for air.
    grid: Vec<u32>,
    px: Vec<Rgb>,
    bg: Vec<Rgb>,
    emitters: Vec<Emitter>,
    phase: Phase,
    grains: usize,
    rate: usize,
    frame: u32,
}

fn pack(c: Rgb) -> u32 {
    0x0100_0000 | (c.0 as u32) << 16 | (c.1 as u32) << 8 | c.2 as u32
}

fn unpack(v: u32) -> Rgb {
    Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

impl Sand {
    fn restart(&mut self, rng: &mut Rng) {
        self.grid.fill(EMPTY);
        self.grains = 0;
        self.phase = Phase::Pour;
        self.frame = 0;
        let n = rng.range(1, 4) as usize;
        let base = rng.f32();
        self.emitters = (0..n)
            .map(|i| Emitter {
                x: rng.rangef(0.0, self.w as f32),
                vx: rng.rangef(-0.5, 0.5),
                hue: base + i as f32 * rng.rangef(0.08, 0.3),
            })
            .collect();
    }

    fn pour(&mut self, rng: &mut Rng) {
        let w = self.w as f32;
        let per = (self.rate / self.emitters.len()).max(1);
        for e in &mut self.emitters {
            // Wander: occasional new heading, bounce off the walls.
            if rng.chance(0.02) {
                e.vx = rng.rangef(-0.8, 0.8);
            }
            e.x += e.vx;
            if e.x < 0.0 {
                e.x = 0.0;
                e.vx = e.vx.abs();
            } else if e.x > w - 1.0 {
                e.x = (w - 1.0).max(0.0);
                e.vx = -e.vx.abs();
            }
            e.hue += 0.0015;
            for _ in 0..per {
                let x = (e.x + rng.rangef(-1.5, 1.5)).clamp(0.0, w - 1.0) as usize;
                let y = rng.below(2.min(self.h));
                let i = y * self.w + x;
                if self.grid[i] == EMPTY {
                    let c = Rgb::hsv(
                        e.hue + rng.rangef(-0.02, 0.02),
                        rng.rangef(0.45, 0.65),
                        rng.rangef(0.78, 0.95),
                    );
                    self.grid[i] = pack(c);
                    self.grains += 1;
                }
            }
        }
    }

    /// True when some column is solid sand from near the top to the floor
    /// (falling streams always have gaps, a pile does not).
    fn pile_reaches_top(&self) -> bool {
        let (w, h) = (self.w, self.h);
        let top = 3.min(h - 1);
        (0..w).any(|x| (top..h).all(|y| self.grid[y * w + x] != EMPTY))
    }

    /// One gravity pass, bottom-up so a grain moves at most once.
    fn fall(&mut self, rng: &mut Rng, hole: Option<(usize, usize)>) {
        let (w, h) = (self.w, self.h);
        for y in (0..h).rev() {
            let ltr = rng.chance(0.5);
            for k in 0..w {
                let x = if ltr { k } else { w - 1 - k };
                let i = y * w + x;
                let g = self.grid[i];
                if g == EMPTY {
                    continue;
                }
                if y + 1 == h {
                    if let Some((a, b)) = hole {
                        if x >= a && x < b {
                            self.grid[i] = EMPTY;
                            self.grains -= 1;
                        }
                    }
                    continue;
                }
                let below = i + w;
                if self.grid[below] == EMPTY {
                    self.grid[below] = g;
                    self.grid[i] = EMPTY;
                    continue;
                }
                let first: i32 = if rng.chance(0.5) { -1 } else { 1 };
                for dx in [first, -first] {
                    let nx = x as i32 + dx;
                    if nx < 0 || nx >= w as i32 {
                        continue;
                    }
                    let d = below as i32 + dx;
                    if self.grid[d as usize] == EMPTY {
                        self.grid[d as usize] = g;
                        self.grid[i] = EMPTY;
                        break;
                    }
                }
            }
        }
    }
}

impl Animation for Sand {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        let (w, h) = (self.w, self.h);
        self.frame += 1;
        let cells = w * h;
        let hole = match self.phase {
            Phase::Pour => {
                self.pour(rng);
                // Full enough, or the pile reaches the spout: open the floor.
                let top_blocked = self.frame % 16 == 0 && self.pile_reaches_top();
                if self.grains * 100 > cells * 58 || top_blocked {
                    self.phase = Phase::Drain(0.0);
                }
                None
            }
            Phase::Drain(r) => {
                let r = (r + 0.08).min(w as f32 * 0.5);
                self.phase = Phase::Drain(r);
                if self.grains == 0 {
                    self.restart(rng);
                    None
                } else {
                    let mid = w as f32 * 0.5;
                    Some(((mid - r).max(0.0) as usize, ((mid + r).ceil() as usize).min(w)))
                }
            }
        };
        self.fall(rng, hole);
        self.fall(rng, hole);

        for i in 0..cells {
            let g = self.grid[i];
            self.px[i] = if g == EMPTY { self.bg[i / w] } else { unpack(g) };
        }
        // Show the open floor as a dark slit.
        if let Some((a, b)) = hole {
            for x in a..b {
                self.px[(h - 1) * w + x] = Rgb::BLACK;
            }
        }
        c.blit_pixels(&self.px);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    let ph = h * 2;
    let bg = (0..ph)
        .map(|y| Rgb(14, 12, 30).lerp(Rgb(40, 28, 52), y as f32 / ph.max(1) as f32))
        .collect();
    let mut s = Sand {
        w,
        h: ph,
        grid: vec![EMPTY; w * ph],
        px: vec![Rgb::BLACK; w * ph],
        bg,
        emitters: Vec::new(),
        phase: Phase::Pour,
        grains: 0,
        rate: (w * ph / 2500).clamp(1, 16),
        frame: 0,
    };
    s.restart(rng);
    Box::new(s)
}
