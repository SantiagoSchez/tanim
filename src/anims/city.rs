//! Night skyline: three parallax layers of procedural buildings with windows
//! flickering on and off, a moon, twinkling stars, planes and street traffic.
//! Buildings are laid out in cells; everything finer is half-block pixels.

use super::Animation;
use crate::canvas::{Canvas, Rgb, HALF};
use crate::rng::Rng;
use std::collections::VecDeque;

const SKY_TOP: Rgb = Rgb(4, 5, 18);
const SKY_LOW: Rgb = Rgb(48, 26, 64);
const ROAD: Rgb = Rgb(20, 20, 26);
const WARM: [Rgb; 3] = [Rgb(255, 205, 110), Rgb(255, 170, 80), Rgb(250, 230, 170)];
const COOL: [Rgb; 2] = [Rgb(170, 215, 255), Rgb(140, 250, 230)];

#[derive(Clone, Copy)]
enum Roof {
    Flat,
    Antenna,
    Stepped,
    Spire,
    Slant,
}

struct Building {
    x: i32,
    w: i32,
    h: i32,
    roof: Roof,
    /// Window grid: 0 = dark, otherwise index + 1 into the tint palette.
    windows: Vec<u8>,
    wcols: i32,
    spacing: i32,
    blink: u32,
}

struct Layer {
    speed: f32,
    offset: f32,
    body: Rgb,
    dim: f32,
    hmin: f32,
    hmax: f32,
    gap: (i32, i32),
    lit: f32,
    buildings: VecDeque<Building>,
    next_x: i32,
}

struct Car {
    x: f32,
    v: f32,
    lane: i32,
    color: Rgb,
}

struct Plane {
    x: f32,
    y: i32,
    v: f32,
}

struct Star {
    x: i32,
    /// Pixel row.
    y: i32,
    phase: u32,
    /// 0 faint dot, 1 bright dot, 2 small cross, 3 dot that flares into a cross.
    kind: u8,
}

struct City {
    w: i32,
    h: i32,
    ground: i32,
    layers: Vec<Layer>,
    stars: Vec<Star>,
    cars: Vec<Car>,
    plane: Option<Plane>,
    moon: (i32, i32, i32),
    frame: u32,
}

impl Layer {
    fn window_rows(b: &Building) -> i32 {
        (b.h - 2).max(0)
    }

    fn make(&mut self, rng: &mut Rng, avail: i32) -> Building {
        let w = rng.range(4, 13);
        let h = ((avail as f32) * rng.rangef(self.hmin, self.hmax)).round().max(2.0) as i32;
        let roof = match rng.below(9) {
            0 | 1 | 2 => Roof::Flat,
            3 | 4 => Roof::Antenna,
            5 | 6 => Roof::Stepped,
            7 => Roof::Spire,
            _ => Roof::Slant,
        };
        let spacing = if rng.chance(0.3) { 3 } else { 2 };
        let wcols = ((w - 1) / spacing).max(0);
        let mut b = Building {
            x: self.next_x,
            w,
            h,
            roof,
            windows: Vec::new(),
            wcols,
            spacing,
            blink: rng.below(30) as u32,
        };
        let rows = Self::window_rows(&b);
        let cool = rng.chance(0.3);
        b.windows = (0..wcols * rows)
            .map(|_| {
                if rng.chance(self.lit) {
                    if cool { 4 + rng.below(2) as u8 } else { 1 + rng.below(3) as u8 }
                } else {
                    0
                }
            })
            .collect();
        self.next_x += w + rng.range(self.gap.0, self.gap.1);
        b
    }

    fn fill(&mut self, rng: &mut Rng, screen_w: i32, avail: i32) {
        let left = self.offset as i32;
        while self.buildings.front().is_some_and(|b| b.x + b.w + 4 < left) {
            self.buildings.pop_front();
        }
        while self.next_x < left + screen_w + 4 {
            let b = self.make(rng, avail);
            self.buildings.push_back(b);
        }
    }

    fn flicker(&mut self, rng: &mut Rng) {
        let n = self.buildings.len();
        if n == 0 {
            return;
        }
        for _ in 0..2 {
            let b = &mut self.buildings[rng.below(n)];
            if b.windows.is_empty() {
                continue;
            }
            let i = rng.below(b.windows.len());
            b.windows[i] = if b.windows[i] != 0 {
                0
            } else if rng.chance(0.3) {
                4 + rng.below(2) as u8
            } else {
                1 + rng.below(3) as u8
            };
        }
    }
}

fn tint(v: u8) -> Rgb {
    match v {
        1..=3 => WARM[v as usize - 1],
        _ => COOL[(v as usize - 4).min(1)],
    }
}

impl City {
    fn new(w: usize, h: usize, rng: &mut Rng) -> City {
        let (w, h) = (w as i32, h as i32);
        let ground = if h >= 10 { h - 3 } else { h };
        let spec = [
            (0.12, Rgb(34, 30, 62), 0.35, 0.35, 0.8, (-2, 2), 0.25),
            (0.3, Rgb(22, 20, 42), 0.6, 0.22, 0.62, (-1, 3), 0.35),
            (0.6, Rgb(11, 11, 20), 1.0, 0.12, 0.48, (1, 5), 0.45),
        ];
        let mut layers: Vec<Layer> = spec
            .iter()
            .map(|&(speed, body, dim, hmin, hmax, gap, lit)| Layer {
                speed,
                offset: rng.rangef(0.0, 500.0),
                body,
                dim,
                hmin,
                hmax,
                gap,
                lit,
                buildings: VecDeque::new(),
                next_x: 0,
            })
            .collect();
        for l in layers.iter_mut() {
            l.next_x = l.offset as i32 - rng.range(0, 8);
            l.fill(rng, w, ground);
        }
        let stars = (0..(w * ground / 40).max(1))
            .map(|_| Star {
                x: rng.range(0, w),
                y: rng.range(0, (ground * 4 / 3).max(1)),
                phase: rng.below(200) as u32,
                kind: *rng.pick(&[0, 0, 1, 2, 3]),
            })
            .collect();
        let r = (h / 6).clamp(2, 6);
        let moon = (w - w / 5 - r, (h / 5).max(r / 2 + 1), r);
        City { w, h, ground, layers, stars, cars: Vec::new(), plane: None, moon, frame: 0 }
    }

    fn sky(&self, y: i32) -> Rgb {
        SKY_TOP.lerp(SKY_LOW, (y as f32 / self.ground.max(1) as f32).powf(1.6))
    }

    fn draw_building(&self, c: &mut Canvas, l: &Layer, b: &Building) {
        let sx = b.x - l.offset as i32;
        let top = self.ground - b.h;
        let body = l.body;
        c.fill_rect(sx, top, b.w, b.h, ' ', body, body);
        match b.roof {
            Roof::Flat => {}
            Roof::Antenna => {
                let ax = sx + b.w / 2;
                let len = (b.h / 6).clamp(1, 4);
                for py in 2 * (top - len)..2 * top {
                    c.pixel(ax, py, body.add(Rgb(20, 20, 20)));
                }
                let on = (self.frame + b.blink) % 20 < 5;
                let red = if on { Rgb(255, 40, 40) } else { Rgb(80, 20, 25) };
                c.pixel(ax, 2 * (top - len) - 1, red.scale(l.dim.max(0.5)));
            }
            Roof::Stepped => {
                c.fill_rect(sx + 1, top - 1, b.w - 2, 1, ' ', body, body);
                c.fill_rect(sx + 2, top - 2, b.w - 4, 1, ' ', body, body);
            }
            Roof::Spire => {
                let ax = sx + b.w / 2;
                c.fill_rect(ax - 1, top - 1, 3, 1, ' ', body, body);
                for py in 2 * (top - 5) + 1..2 * (top - 1) {
                    c.pixel(ax, py, body);
                }
            }
            Roof::Slant => {
                // A gable stepping in one pixel every two rows.
                for k in 0..2 * (b.w / 2) {
                    let inset = (k + 1) / 2;
                    for x in sx + inset..sx + b.w - inset {
                        c.pixel(x, 2 * top - 1 - k, body);
                    }
                }
            }
        }
        let rows = Layer::window_rows(b);
        for r in 0..rows {
            for k in 0..b.wcols {
                let v = b.windows[(r * b.wcols + k) as usize];
                let x = sx + 1 + k * b.spacing;
                let y = top + 1 + r;
                if v != 0 {
                    c.put(x, y, HALF, tint(v).scale(l.dim));
                } else if l.dim >= 1.0 {
                    c.put(x, y, HALF, body.add(Rgb(10, 10, 16)));
                }
            }
        }
    }

    fn draw_moon(&self, c: &mut Canvas) {
        let (mx, my, r) = self.moon;
        let (cx, cy) = (mx as f32, my as f32 * 2.0);
        let rf = r as f32 * 2.0;
        let r2 = rf * rf;
        for py in (cy - rf) as i32 - 1..=(cy + rf) as i32 + 1 {
            for x in mx - 2 * r - 1..=mx + 2 * r + 1 {
                let dx = x as f32 - cx;
                let dy = py as f32 - cy;
                let d2 = dx * dx + dy * dy;
                if d2 <= r2 {
                    let crater = ((dx * 0.7).sin() * (dy * 0.9).cos() * 0.5 + 0.5) * 0.12;
                    let limb = 1.0 - 0.25 * d2 / r2;
                    c.pixel(x, py, Rgb(245, 238, 205).scale(limb - crater));
                } else if d2 <= r2 * 1.9 && c.get(x, py >> 1).is_some_and(|cell| cell.ch == ' ') {
                    let glow = 1.0 - (d2 / r2 - 1.0) / 0.9;
                    let y = py >> 1;
                    c.paint_bg(x, y, self.sky(y).lerp(Rgb(90, 80, 110), glow * 0.35));
                }
            }
        }
    }

    fn draw_street(&self, c: &mut Canvas) {
        if self.ground >= self.h {
            return;
        }
        let g = self.ground;
        let near = &self.layers[2];
        // Sidewalk over a curb, then the road.
        c.fill_rect(0, g, self.w, 1, HALF, Rgb(30, 30, 38), Rgb(50, 50, 60));
        c.fill_rect(0, g + 1, self.w, self.h - g - 1, ' ', ROAD, ROAD);
        let off = near.offset as i32 * 2;
        if g + 2 < self.h {
            for x in 0..self.w {
                if (x + off).rem_euclid(8) < 4 {
                    c.pixel(x, 2 * g + 3, Rgb(120, 112, 72));
                }
            }
        }
        for car in &self.cars {
            let x = car.x as i32;
            // Body on the lower pixel of the lane's row, cabin above it.
            let py = 2 * (g + 1 + car.lane).min(self.h - 1) + 1;
            let (front, back) = if car.v > 0.0 { (x + 3, x) } else { (x, x + 3) };
            let dir = if car.v > 0.0 { 1 } else { -1 };
            for i in 1..6 {
                let a = 0.5 - i as f32 * 0.08;
                blend(c, front + dir * i, py, Rgb(120, 110, 70), a);
                blend(c, front + dir * i, py - 1, Rgb(120, 110, 70), a * 0.5);
            }
            for i in 0..4 {
                c.pixel(x + i, py, car.color);
            }
            let glass = car.color.lerp(Rgb(150, 180, 210), 0.35).scale(0.6);
            c.pixel(x + 1, py - 1, glass);
            c.pixel(x + 2, py - 1, glass);
            c.pixel(front, py, Rgb(255, 245, 190));
            c.pixel(back, py, Rgb(230, 40, 40));
        }
    }

    fn update(&mut self, rng: &mut Rng) {
        self.frame = self.frame.wrapping_add(1);
        let (w, ground) = (self.w, self.ground);
        for l in self.layers.iter_mut() {
            l.offset += l.speed;
            l.fill(rng, w, ground);
            l.flicker(rng);
        }
        if self.ground + 1 < self.h {
            for car in self.cars.iter_mut() {
                car.x += car.v;
            }
            self.cars.retain(|c| c.x > -8.0 && c.x < w as f32 + 8.0);
            if self.cars.len() < 4 && rng.chance(0.03) {
                let lanes = (self.h - self.ground - 1).min(2);
                let lane = rng.range(0, lanes);
                let right = lane == 0 || lanes == 1 && rng.chance(0.5);
                let v = rng.rangef(0.8, 1.6) * if right { 1.0 } else { -1.0 };
                let x = if right { -6.0 } else { w as f32 + 2.0 };
                let color = Rgb::hsv(rng.f32(), 0.5, rng.rangef(0.5, 0.8));
                self.cars.push(Car { x, v, lane, color });
            }
        }
        match &mut self.plane {
            Some(p) => {
                p.x += p.v;
                if p.x < -6.0 || p.x > w as f32 + 6.0 {
                    self.plane = None;
                }
            }
            None => {
                if rng.chance(0.004) {
                    let right = rng.chance(0.5);
                    self.plane = Some(Plane {
                        x: if right { -4.0 } else { w as f32 + 4.0 },
                        y: rng.range(1, (ground / 3).max(2)),
                        v: if right { 0.5 } else { -0.5 },
                    });
                }
            }
        }
    }

    fn draw(&self, c: &mut Canvas) {
        for y in 0..self.h {
            let s = self.sky(y);
            c.fill_rect(0, y, self.w, 1, ' ', s, s);
        }
        for s in &self.stars {
            let t = (self.frame + s.phase) % 200;
            let v = if t < 12 { 1.0 } else { 0.45 + 0.2 * ((t as f32) * 0.2).sin() };
            let col = Rgb(220, 220, 255).scale(v);
            let sky = self.sky(s.y >> 1);
            c.pixel(s.x, s.y, if s.kind == 0 { sky.lerp(col, 0.7) } else { col });
            if s.kind == 2 || s.kind == 3 && t < 12 {
                // Steady arms on crosses keep the twinkle to a single cell.
                let arm = if t < 12 { sky.lerp(col, 0.4) } else { sky.lerp(Rgb(220, 220, 255), 0.22) };
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    c.pixel(s.x + dx, s.y + dy, arm);
                }
            }
        }
        self.draw_moon(c);
        if let Some(p) = &self.plane {
            // A two-pixel fuselage with a tail fin, strobing white at the nose
            // and red at the tail.
            let x = p.x as i32;
            let py = 2 * p.y + 1;
            let blink = self.frame % 10 < 3;
            let (nose, tail) = if p.v > 0.0 { (x + 2, x - 1) } else { (x - 1, x + 2) };
            c.pixel(x, py, Rgb(120, 120, 140));
            c.pixel(x + 1, py, Rgb(120, 120, 140));
            c.pixel(if p.v > 0.0 { x } else { x + 1 }, py - 1, Rgb(90, 90, 110));
            if blink {
                c.pixel(nose, py, Rgb(255, 255, 255));
            } else {
                c.pixel(tail, py, Rgb(255, 60, 60));
            }
        }
        for l in &self.layers {
            for b in &l.buildings {
                let sx = b.x - l.offset as i32;
                if sx + b.w >= 0 && sx < self.w {
                    self.draw_building(c, l, b);
                }
            }
        }
        self.draw_street(c);
    }
}

/// Mix `col` into one pixel by `a`, keeping whatever is drawn there.
fn blend(c: &mut Canvas, x: i32, py: i32, col: Rgb, a: f32) {
    let Some(cell) = c.get(x, py >> 1) else { return };
    let cur = if cell.ch == HALF && py & 1 == 0 { cell.fg } else { cell.bg };
    c.pixel(x, py, cur.lerp(col, a));
}

impl Animation for City {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        self.update(rng);
        self.draw(c);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    Box::new(City::new(w, h, rng))
}
