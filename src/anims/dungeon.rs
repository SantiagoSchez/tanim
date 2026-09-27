//! A roguelike dungeon that builds itself and is then explored.
//!
//! Each level is generated in plain view, one step at a time: either rooms
//! from a binary space partition joined by dug-out corridors with doors, or
//! caves grown from noise by a cellular automaton. Stairs, monsters and loot
//! pop in, fog falls, and a hero sets off: seeing by line of sight (rooms
//! light up whole when entered), fighting whatever notices it, drinking
//! potions when hurt, picking up loot and exploring the frontier until it
//! takes the stairs down to a harder level. Should it die, a new hero starts
//! again from the top.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;
use std::collections::VecDeque;

const BG: Rgb = Rgb(0, 0, 0);
const HUD_BG: Rgb = Rgb(12, 12, 20);
const HUD_FG: Rgb = Rgb(200, 204, 220);
const STONE: Rgb = Rgb(78, 76, 96);
const STONE_HI: Rgb = Rgb(118, 116, 138);
const STONE_LO: Rgb = Rgb(44, 42, 58);
const FLOOR: Rgb = Rgb(46, 38, 32);
const FLOOR_HI: Rgb = Rgb(60, 50, 42);
const CORRIDOR: Rgb = Rgb(34, 32, 38);
const WOOD: Rgb = Rgb(124, 76, 38);
const WOOD_DARK: Rgb = Rgb(74, 44, 22);
/// How far the hero sees, in tiles.
const SIGHT: i32 = 7;
/// Frames between the hero's actions, and between awake monsters' moves.
const HERO_DELAY: u32 = 3;
const MOB_DELAY: u32 = 5;
/// Frames a level may take before the hero heads for the stairs regardless.
const PATIENCE: u32 = 2400;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tile {
    Rock,
    Floor,
    Corridor,
    Door,
    OpenDoor,
    Up,
    Down,
}

impl Tile {
    fn walkable(self) -> bool {
        self != Tile::Rock
    }
    fn opaque(self) -> bool {
        matches!(self, Tile::Rock | Tile::Door)
    }
}

/// 5x5 sprites, resampled to the tile size. Palette letters: see `ink`.
type Sprite = [&'static str; 5];

const HERO: Sprite = [".hhh.", ".hsh.", "bbbbw", ".bbbw", ".b.b."];
const TOMB: Sprite = ["..h..", ".hhh.", "..h..", ".hhh.", "hhhhh"];

#[derive(Clone, Copy)]
struct Kind {
    name: &'static str,
    sprite: Sprite,
    hp: i32,
    atk: i32,
    def: i32,
    /// Frames between moves while chasing.
    delay: u32,
    depth: u32,
}

const KINDS: [Kind; 6] = [
    Kind { name: "rat", sprite: [".....", ".....", ".nnn.", "nnnnk", "n...n"], hp: 4, atk: 2, def: 0, delay: 5, depth: 1 },
    Kind { name: "bat", sprite: ["p...p", "pp.pp", ".pep.", "..p..", "....."], hp: 3, atk: 1, def: 0, delay: 3, depth: 1 },
    Kind { name: "slime", sprite: [".....", ".ggg.", "gegeg", "ggggg", "....."], hp: 6, atk: 2, def: 0, delay: 7, depth: 1 },
    Kind { name: "skeleton", sprite: [".www.", ".wkw.", "..w..", ".www.", ".w.w."], hp: 9, atk: 3, def: 1, delay: 5, depth: 2 },
    Kind { name: "orc", sprite: [".ooo.", "ororo", ".ooo.", "ooooo", ".o.o."], hp: 14, atk: 4, def: 1, delay: 6, depth: 3 },
    Kind { name: "dragon", sprite: ["r...r", "rrrr.", ".rrry", "rrrrr", "r.r.r"], hp: 30, atk: 7, def: 2, delay: 6, depth: 5 },
];

#[derive(Clone, Copy, PartialEq)]
enum Loot {
    Gold,
    Gem,
    Potion,
    Sword,
    Shield,
}

impl Loot {
    fn sprite(self) -> Sprite {
        match self {
            Loot::Gold => [".....", "..y..", ".yyy.", "yyyyy", "....."],
            Loot::Gem => [".....", ".cc..", "ccccc", ".ccc.", "..c.."],
            Loot::Potion => ["..t..", ".e.e.", ".rrr.", ".rrr.", "....."],
            Loot::Sword => ["....w", "...w.", ".tw..", "..t..", ".t..."],
            Loot::Shield => [".....", ".hhh.", ".hyh.", ".hhh.", "..h.."],
        }
    }
}

/// Palette for sprite letters.
fn ink(ch: u8) -> Option<Rgb> {
    Some(match ch {
        b'h' => Rgb(170, 176, 190),
        b's' => Rgb(236, 196, 160),
        b'b' => Rgb(60, 110, 220),
        b'w' => Rgb(236, 236, 240),
        b'k' => Rgb(20, 16, 20),
        b'y' => Rgb(250, 210, 60),
        b'r' => Rgb(220, 50, 40),
        b'g' => Rgb(80, 200, 90),
        b'p' => Rgb(150, 90, 200),
        b'o' => Rgb(80, 130, 60),
        b'e' => Rgb(250, 250, 250),
        b'n' => Rgb(140, 100, 70),
        b'c' => Rgb(80, 230, 240),
        b't' => Rgb(170, 120, 60),
        _ => return None,
    })
}

struct Room {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Room {
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
    /// Inside the room or on the wall ring around it.
    fn lights(&self, x: i32, y: i32) -> bool {
        x >= self.x - 1 && y >= self.y - 1 && x <= self.x + self.w && y <= self.y + self.h
    }
}

struct Mob {
    x: i32,
    y: i32,
    kind: usize,
    hp: i32,
    atk: i32,
    def: i32,
    awake: bool,
    shown: bool,
    timer: u32,
    hurt: u32,
}

struct Item {
    x: i32,
    y: i32,
    loot: Loot,
    shown: bool,
    taken: bool,
}

/// One step of the generation replay: tile changes, maybe revealing an
/// entity, and how many frames it takes.
struct Step {
    tiles: Vec<(usize, Tile)>,
    reveal: Option<Reveal>,
    cost: f32,
}

#[derive(Clone, Copy)]
enum Reveal {
    Mob(usize),
    Item(usize),
    Torch(usize),
}

struct Spark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: u32,
    color: Rgb,
}

#[derive(PartialEq)]
enum Phase {
    Building,
    Exploring,
    Descending(u32),
    Dead(u32),
}

struct Hero {
    x: i32,
    y: i32,
    hp: i32,
    max: i32,
    atk: i32,
    def: i32,
    gold: u32,
    potions: u32,
    hurt: u32,
    timer: u32,
}

struct Dungeon {
    w: usize,
    h: usize,
    ph: usize,
    /// Tile size in pixels and map size in tiles.
    t: i32,
    mw: i32,
    mh: i32,
    ox: i32,
    oy: i32,
    map: Vec<Tile>,
    /// What the replay has built so far (the real map is already complete).
    shown: Vec<Tile>,
    known: Vec<bool>,
    seen: Vec<bool>,
    rooms: Vec<Room>,
    torches: Vec<(usize, bool)>,
    mobs: Vec<Mob>,
    items: Vec<Item>,
    steps: VecDeque<Step>,
    budget: f32,
    phase: Phase,
    hero: Hero,
    depth: u32,
    cave: bool,
    sparks: Vec<Spark>,
    px: Vec<Rgb>,
    frame: u32,
    level_frames: u32,
    message: String,
    message_age: u32,
    seed: u32,
    /// Space was pressed: build a fresh level on the next tick.
    regen: bool,
}

fn hash(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77) ^ seed;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 12)
}

impl Dungeon {
    fn new(w: usize, h: usize, rng: &mut Rng) -> Dungeon {
        let ph = 2 * h;
        // One HUD text row on top; tiles fill the rest.
        let area = ph.saturating_sub(2) as i32;
        let t = (area / 15).clamp(3, 6);
        let (mw, mh) = (w as i32 / t, area / t);
        let mut d = Dungeon {
            w,
            h,
            ph,
            t,
            mw,
            mh,
            ox: (w as i32 - mw * t) / 2,
            oy: 2 + (area - mh * t) / 2,
            map: Vec::new(),
            shown: Vec::new(),
            known: Vec::new(),
            seen: Vec::new(),
            rooms: Vec::new(),
            torches: Vec::new(),
            mobs: Vec::new(),
            items: Vec::new(),
            steps: VecDeque::new(),
            budget: 0.0,
            phase: Phase::Building,
            hero: Hero::fresh(),
            depth: 1,
            cave: false,
            sparks: Vec::new(),
            px: vec![BG; w * ph],
            frame: 0,
            level_frames: 0,
            message: String::new(),
            message_age: 0,
            seed: rng.next_u64() as u32,
            regen: false,
        };
        d.generate(rng);
        d
    }

    fn big_enough(&self) -> bool {
        self.mw >= 10 && self.mh >= 7
    }

    #[inline]
    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.mw && y < self.mh).then(|| (y * self.mw + x) as usize)
    }

    fn tile(&self, x: i32, y: i32) -> Tile {
        self.idx(x, y).map_or(Tile::Rock, |i| self.map[i])
    }

    fn say(&mut self, msg: String) {
        self.message = msg;
        self.message_age = 0;
    }

    // ----- generation -----

    fn generate(&mut self, rng: &mut Rng) {
        let n = (self.mw * self.mh).max(0) as usize;
        self.map = vec![Tile::Rock; n];
        self.shown = vec![Tile::Rock; n];
        self.known = vec![false; n];
        self.seen = vec![false; n];
        self.rooms.clear();
        self.torches.clear();
        self.mobs.clear();
        self.items.clear();
        self.steps.clear();
        self.sparks.clear();
        self.budget = 0.0;
        self.level_frames = 0;
        self.phase = Phase::Building;
        if !self.big_enough() {
            return;
        }
        self.cave = rng.chance(0.4);
        if self.cave {
            self.caves(rng);
            self.say(format!("Depth {}: caves take shape", self.depth));
        } else {
            self.rooms_and_corridors(rng);
            self.say(format!("Depth {}: digging rooms and corridors", self.depth));
        }
        self.populate(rng);
    }

    fn rooms_and_corridors(&mut self, rng: &mut Rng) {
        // Split the map into leaves, a room in each, siblings joined up.
        let mut leaves = Vec::new();
        let mut links = Vec::new();
        split(0, 0, self.mw, self.mh, rng, &mut leaves, &mut links, 0);
        for &(lx, ly, lw, lh) in &leaves {
            let w = rng.range(3, (lw - 2).max(4));
            let h = rng.range(3, (lh - 2).max(4));
            let x = lx + 1 + rng.range(0, (lw - w - 1).max(1));
            let y = ly + 1 + rng.range(0, (lh - h - 1).max(1));
            let (w, h) = (w.min(self.mw - 1 - x), h.min(self.mh - 1 - y));
            if w >= 2 && h >= 2 {
                self.rooms.push(Room { x, y, w, h });
            }
        }
        for r in 0..self.rooms.len() {
            let Room { x, y, w, h } = self.rooms[r];
            for yy in y..y + h {
                let row: Vec<(usize, Tile)> = (x..x + w).filter_map(|xx| self.idx(xx, yy)).map(|i| (i, Tile::Floor)).collect();
                for &(i, t) in &row {
                    self.map[i] = t;
                }
                self.steps.push_back(Step { tiles: row, reveal: None, cost: 1.0 });
            }
        }
        // Corridors between rooms whose leaves were siblings, plus a couple
        // of extra loops so the map is not a pure tree.
        let mut pairs: Vec<(usize, usize)> = links
            .iter()
            .filter_map(|&(a, b)| Some((self.room_near(a)?, self.room_near(b)?)))
            .filter(|(a, b)| a != b)
            .collect();
        for _ in 0..rng.range(1, 3) {
            if self.rooms.len() > 2 {
                let a = rng.below(self.rooms.len());
                pairs.push((a, (a + 1 + rng.below(self.rooms.len() - 1)) % self.rooms.len()));
            }
        }
        for (a, b) in pairs {
            let (ax, ay) = self.room_center(a, rng);
            let (bx, by) = self.room_center(b, rng);
            let mut path = Vec::new();
            let horizontal_first = rng.chance(0.5);
            let (mut x, mut y) = (ax, ay);
            let walk = |x: &mut i32, y: &mut i32, tx: i32, ty: i32, path: &mut Vec<(i32, i32)>| {
                while (*x, *y) != (tx, ty) {
                    *x += (tx - *x).signum();
                    *y += (ty - *y).signum();
                    path.push((*x, *y));
                }
            };
            if horizontal_first {
                walk(&mut x, &mut y, bx, ay, &mut path);
                walk(&mut x, &mut y, bx, by, &mut path);
            } else {
                walk(&mut x, &mut y, ax, by, &mut path);
                walk(&mut x, &mut y, bx, by, &mut path);
            }
            for (x, y) in path {
                let Some(i) = self.idx(x, y) else { continue };
                if self.map[i] == Tile::Rock {
                    self.map[i] = Tile::Corridor;
                    self.steps.push_back(Step { tiles: vec![(i, Tile::Corridor)], reveal: None, cost: 0.35 });
                }
            }
        }
        // Doors where a corridor squeezes through a room's wall ring.
        for i in 0..self.map.len() {
            let (x, y) = ((i as i32) % self.mw, (i as i32) / self.mw);
            if self.map[i] != Tile::Corridor || !self.rooms.iter().any(|r| r.lights(x, y) && !r.contains(x, y)) {
                continue;
            }
            let open = |dx: i32, dy: i32| self.tile(x + dx, y + dy).walkable();
            let pass = (open(-1, 0) && open(1, 0) && !open(0, -1) && !open(0, 1))
                || (open(0, -1) && open(0, 1) && !open(-1, 0) && !open(1, 0));
            if pass && rng.chance(0.75) {
                self.map[i] = Tile::Door;
                self.steps.push_back(Step { tiles: vec![(i, Tile::Door)], reveal: None, cost: 2.0 });
            }
        }
        // Torches on the walls above some rooms.
        for r in 0..self.rooms.len() {
            if rng.chance(0.6) {
                let Room { x, y, w, .. } = self.rooms[r];
                if let Some(i) = self.idx(x + rng.range(0, w), y - 1) {
                    if self.map[i] == Tile::Rock {
                        self.torches.push((i, false));
                        let k = self.torches.len() - 1;
                        self.steps.push_back(Step { tiles: vec![], reveal: Some(Reveal::Torch(k)), cost: 1.5 });
                    }
                }
            }
        }
    }

    fn room_near(&self, leaf: (i32, i32, i32, i32)) -> Option<usize> {
        let (lx, ly, lw, lh) = leaf;
        let (cx, cy) = (lx + lw / 2, ly + lh / 2);
        (0..self.rooms.len()).min_by_key(|&r| {
            let rm = &self.rooms[r];
            (rm.x + rm.w / 2 - cx).abs() + (rm.y + rm.h / 2 - cy).abs()
        })
    }

    fn room_center(&self, r: usize, rng: &mut Rng) -> (i32, i32) {
        let rm = &self.rooms[r];
        (rm.x + rng.range(0, rm.w), rm.y + rng.range(0, rm.h))
    }

    fn caves(&mut self, rng: &mut Rng) {
        let (mw, mh) = (self.mw, self.mh);
        let mut rock: Vec<bool> = (0..mw * mh)
            .map(|i| {
                let (x, y) = (i % mw, i / mw);
                x == 0 || y == 0 || x == mw - 1 || y == mh - 1 || rng.chance(0.45)
            })
            .collect();
        let snapshot = |rock: &[bool], steps: &mut VecDeque<Step>, prev: &mut Vec<bool>, cost: f32| {
            let tiles = (0..rock.len())
                .filter(|&i| rock[i] != prev[i])
                .map(|i| (i, if rock[i] { Tile::Rock } else { Tile::Floor }))
                .collect();
            steps.push_back(Step { tiles, reveal: None, cost });
            prev.clone_from_slice(rock);
        };
        let mut prev = vec![true; rock.len()];
        snapshot(&rock, &mut self.steps, &mut prev, 14.0);
        for _ in 0..5 {
            rock = (0..mw * mh)
                .map(|i| {
                    let (x, y) = (i % mw, i / mw);
                    if x == 0 || y == 0 || x == mw - 1 || y == mh - 1 {
                        return true;
                    }
                    let walls = (-1..=1)
                        .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
                        .filter(|&(dx, dy)| rock[((y + dy) * mw + x + dx) as usize])
                        .count();
                    walls >= 5
                })
                .collect();
            snapshot(&rock, &mut self.steps, &mut prev, 14.0);
        }
        // Keep only the largest open region.
        let mut region = vec![usize::MAX; rock.len()];
        let mut best = (0, 0);
        for start in 0..rock.len() {
            if rock[start] || region[start] != usize::MAX {
                continue;
            }
            let mut size = 0;
            let mut queue = VecDeque::from([start]);
            region[start] = start;
            while let Some(i) = queue.pop_front() {
                size += 1;
                let (x, y) = (i as i32 % mw, i as i32 / mw);
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    if let Some(j) = self.idx(x + dx, y + dy) {
                        if !rock[j] && region[j] == usize::MAX {
                            region[j] = start;
                            queue.push_back(j);
                        }
                    }
                }
            }
            if size > best.0 {
                best = (size, start);
            }
        }
        for i in 0..rock.len() {
            if !rock[i] && region[i] != best.1 {
                rock[i] = true;
            }
        }
        snapshot(&rock, &mut self.steps, &mut prev, 10.0);
        for i in 0..rock.len() {
            self.map[i] = if rock[i] { Tile::Rock } else { Tile::Floor };
        }
    }

    /// Stairs, monsters and loot, each popping in during the replay.
    fn populate(&mut self, rng: &mut Rng) {
        let floors: Vec<usize> = (0..self.map.len()).filter(|&i| self.map[i] == Tile::Floor).collect();
        if floors.len() < 4 {
            return;
        }
        let up = floors[rng.below(floors.len())];
        let dist = self.distances(up, |t| t.walkable());
        let down = *floors.iter().max_by_key(|&&i| dist[i]).unwrap_or(&up);
        self.map[up] = Tile::Up;
        self.map[down] = Tile::Down;
        self.steps.push_back(Step { tiles: vec![(up, Tile::Up)], reveal: None, cost: 6.0 });
        self.steps.push_back(Step { tiles: vec![(down, Tile::Down)], reveal: None, cost: 6.0 });
        let (ux, uy) = (up as i32 % self.mw, up as i32 / self.mw);
        self.hero.x = ux;
        self.hero.y = uy;

        let mut taken = vec![false; self.map.len()];
        taken[up] = true;
        taken[down] = true;
        let mut free_spot = |rng: &mut Rng, min_dist: u32| {
            for _ in 0..50 {
                let i = floors[rng.below(floors.len())];
                if !taken[i] && dist[i] != u32::MAX && dist[i] >= min_dist {
                    taken[i] = true;
                    return Some(i);
                }
            }
            None
        };
        let kinds: Vec<usize> = (0..KINDS.len()).filter(|&k| KINDS[k].depth <= self.depth).collect();
        let count = floors.len() / 45 + self.depth as usize;
        let tough = 1.0 + 0.12 * (self.depth as f32 - 1.0);
        for _ in 0..count.min(40) {
            let Some(i) = free_spot(rng, 6) else { break };
            // Favour the strongest kinds allowed at this depth.
            let k = kinds[rng.below(kinds.len()).max(rng.below(kinds.len()))];
            let kind = KINDS[k];
            self.mobs.push(Mob {
                x: i as i32 % self.mw,
                y: i as i32 / self.mw,
                kind: k,
                hp: (kind.hp as f32 * tough) as i32,
                atk: kind.atk + (self.depth as i32 - 1) / 3,
                def: kind.def,
                awake: false,
                shown: false,
                timer: rng.below(10) as u32,
                hurt: 0,
            });
            self.steps.push_back(Step { tiles: vec![], reveal: Some(Reveal::Mob(self.mobs.len() - 1)), cost: 3.0 });
        }
        for _ in 0..(floors.len() / 60 + 2).min(30) {
            let Some(i) = free_spot(rng, 2) else { break };
            let loot = match rng.below(20) {
                0..=6 => Loot::Gold,
                7..=11 => Loot::Potion,
                12..=13 => Loot::Gem,
                14..=16 => Loot::Sword,
                _ => Loot::Shield,
            };
            self.items.push(Item { x: i as i32 % self.mw, y: i as i32 / self.mw, loot, shown: false, taken: false });
            self.steps.push_back(Step { tiles: vec![], reveal: Some(Reveal::Item(self.items.len() - 1)), cost: 3.0 });
        }
        self.steps.push_back(Step { tiles: vec![], reveal: None, cost: 30.0 });
    }

    /// Breadth-first distances from `from` over tiles passing `ok`.
    fn distances(&self, from: usize, ok: impl Fn(Tile) -> bool) -> Vec<u32> {
        let mut dist = vec![u32::MAX; self.map.len()];
        let mut queue = VecDeque::from([from]);
        dist[from] = 0;
        while let Some(i) = queue.pop_front() {
            let (x, y) = (i as i32 % self.mw, i as i32 / self.mw);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                if let Some(j) = self.idx(x + dx, y + dy) {
                    if dist[j] == u32::MAX && ok(self.map[j]) {
                        dist[j] = dist[i] + 1;
                        queue.push_back(j);
                    }
                }
            }
        }
        dist
    }

    // ----- exploring -----

    fn look(&mut self) {
        self.seen.fill(false);
        let (hx, hy) = (self.hero.x, self.hero.y);
        for dy in -SIGHT..=SIGHT {
            for dx in -SIGHT..=SIGHT {
                if dx * dx + dy * dy > SIGHT * SIGHT {
                    continue;
                }
                let (tx, ty) = (hx + dx, hy + dy);
                let Some(ti) = self.idx(tx, ty) else { continue };
                // Walk the line towards the tile; stop at the first opaque one.
                let steps = dx.abs().max(dy.abs());
                let mut clear = true;
                for s in 1..steps {
                    let x = hx + (dx * s + dx.signum() * steps / 2) / steps.max(1);
                    let y = hy + (dy * s + dy.signum() * steps / 2) / steps.max(1);
                    if self.tile(x, y).opaque() {
                        clear = false;
                        break;
                    }
                }
                if clear {
                    self.seen[ti] = true;
                }
            }
        }
        // A lit room shows whole as soon as the hero is inside it.
        if let Some(room) = self.rooms.iter().find(|r| r.contains(hx, hy)) {
            for y in room.y - 1..=room.y + room.h {
                for x in room.x - 1..=room.x + room.w {
                    if let Some(i) = self.idx(x, y) {
                        self.seen[i] = true;
                    }
                }
            }
        }
        for i in 0..self.seen.len() {
            if self.seen[i] {
                self.known[i] = true;
            }
        }
    }

    fn mob_at(&self, x: i32, y: i32) -> Option<usize> {
        self.mobs.iter().position(|m| m.hp > 0 && m.x == x && m.y == y)
    }

    fn occupied(&self, x: i32, y: i32) -> bool {
        (self.hero.x == x && self.hero.y == y) || self.mob_at(x, y).is_some()
    }

    fn burst(&mut self, x: i32, y: i32, color: Rgb, n: usize, rng: &mut Rng) {
        let (cx, cy) = ((self.ox + x * self.t) as f32 + self.t as f32 / 2.0, (self.oy + y * self.t) as f32 + self.t as f32 / 2.0);
        for _ in 0..n {
            let a = rng.rangef(0.0, std::f32::consts::TAU);
            let v = rng.rangef(0.3, 1.2);
            self.sparks.push(Spark { x: cx, y: cy, vx: a.cos() * v, vy: a.sin() * v - 0.3, life: rng.range(6, 14) as u32, color });
        }
    }

    fn hero_act(&mut self, rng: &mut Rng) {
        let (hx, hy) = (self.hero.x, self.hero.y);
        // Drink first when badly hurt, even mid-fight.
        if self.hero.hp * 100 < self.hero.max * 45 && self.hero.potions > 0 {
            self.hero.potions -= 1;
            self.hero.hp = (self.hero.hp + 10).min(self.hero.max);
            self.burst(hx, hy, Rgb(90, 240, 120), 10, rng);
            self.say("The hero drinks a potion".into());
            return;
        }
        // Fight anything next to us.
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            if let Some(m) = self.mob_at(hx + dx, hy + dy) {
                if self.seen[self.idx(hx + dx, hy + dy).unwrap()] {
                    self.hero_hits(m, rng);
                    return;
                }
            }
        }
        let late = self.level_frames > PATIENCE;
        if late && self.level_frames > PATIENCE + 1200 {
            self.phase = Phase::Descending(0);
            return;
        }
        let here = self.idx(hx, hy).unwrap();
        if self.map[here] == Tile::Down && (late || !self.worth_staying()) {
            self.phase = Phase::Descending(0);
            self.say(format!("Down to depth {}...", self.depth + 1));
            return;
        }
        // Where to next: a nearby monster, loot, the unknown, or the stairs.
        let next = {
            let goals: [&dyn Fn(usize) -> bool; 4] = [
                &|i| !late && self.seen[i] && self.mob_at(i as i32 % self.mw, i as i32 / self.mw).is_some(),
                &|i| !late && self.known[i] && self.items.iter().any(|it| !it.taken && self.idx(it.x, it.y) == Some(i)),
                &|i| !late && self.frontier(i),
                &|i| self.known[i] && self.map[i] == Tile::Down,
            ];
            goals.iter().enumerate().find_map(|(g, goal)| self.first_step(here, *goal, if g == 0 { 6 } else { u32::MAX }))
        };
        let Some(next) = next else {
            // Nowhere left to go (e.g. stairs walled off): move on anyway.
            self.phase = Phase::Descending(0);
            return;
        };
        let (nx, ny) = (next as i32 % self.mw, next as i32 / self.mw);
        if let Some(m) = self.mob_at(nx, ny) {
            self.hero_hits(m, rng);
        } else if self.map[next] == Tile::Door {
            self.map[next] = Tile::OpenDoor;
        } else {
            self.hero.x = nx;
            self.hero.y = ny;
            self.pick_up(rng);
        }
    }

    fn worth_staying(&self) -> bool {
        (0..self.map.len()).any(|i| self.frontier(i)) || self.items.iter().any(|it| !it.taken && self.known[self.idx(it.x, it.y).unwrap()])
    }

    /// A known floor tile next to one never seen.
    fn frontier(&self, i: usize) -> bool {
        if !self.known[i] || !self.map[i].walkable() {
            return false;
        }
        let (x, y) = (i as i32 % self.mw, i as i32 / self.mw);
        [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| self.idx(x + dx, y + dy).is_some_and(|j| !self.known[j]))
    }

    /// First move along the shortest known path to a tile passing `goal`.
    fn first_step(&self, from: usize, goal: &dyn Fn(usize) -> bool, limit: u32) -> Option<usize> {
        let mut parent = vec![usize::MAX; self.map.len()];
        let mut dist = vec![u32::MAX; self.map.len()];
        let mut queue = VecDeque::from([from]);
        dist[from] = 0;
        while let Some(i) = queue.pop_front() {
            if i != from && goal(i) {
                let mut step = i;
                while parent[step] != from {
                    step = parent[step];
                }
                return Some(step);
            }
            if dist[i] >= limit {
                continue;
            }
            let (x, y) = (i as i32 % self.mw, i as i32 / self.mw);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                if let Some(j) = self.idx(x + dx, y + dy) {
                    if dist[j] == u32::MAX && self.known[j] && self.map[j].walkable() {
                        dist[j] = dist[i] + 1;
                        parent[j] = i;
                        queue.push_back(j);
                    }
                }
            }
        }
        None
    }

    fn pick_up(&mut self, rng: &mut Rng) {
        let (x, y) = (self.hero.x, self.hero.y);
        let Some(k) = self.items.iter().position(|it| !it.taken && it.x == x && it.y == y) else { return };
        self.items[k].taken = true;
        let msg = match self.items[k].loot {
            Loot::Gold => {
                let g = rng.range(5, 16) as u32 * self.depth;
                self.hero.gold += g;
                format!("Picked up {g} gold")
            }
            Loot::Gem => {
                self.hero.gold += 50 * self.depth;
                "Found a gem!".into()
            }
            Loot::Potion => {
                self.hero.potions += 1;
                "Found a potion".into()
            }
            Loot::Sword => {
                self.hero.atk += 2;
                "A better sword: +2 attack".into()
            }
            Loot::Shield => {
                self.hero.def += 1;
                "A sturdier shield: +1 defence".into()
            }
        };
        self.burst(x, y, Rgb(250, 230, 120), 8, rng);
        self.say(msg);
    }

    fn hero_hits(&mut self, m: usize, rng: &mut Rng) {
        let dmg = (self.hero.atk - self.mobs[m].def + rng.range(0, 3)).max(1);
        let (x, y) = (self.mobs[m].x, self.mobs[m].y);
        self.mobs[m].hp -= dmg;
        self.mobs[m].hurt = 4;
        self.mobs[m].awake = true;
        self.burst(x, y, Rgb(240, 60, 50), 5, rng);
        if self.mobs[m].hp <= 0 {
            let name = KINDS[self.mobs[m].kind].name;
            self.burst(x, y, Rgb(150, 150, 160), 12, rng);
            self.say(format!("The hero slays the {name}"));
            if rng.chance(0.3) && self.items.iter().all(|it| it.taken || (it.x, it.y) != (x, y)) {
                self.items.push(Item { x, y, loot: Loot::Gold, shown: true, taken: false });
            }
        }
    }

    fn mobs_act(&mut self, rng: &mut Rng) {
        for m in 0..self.mobs.len() {
            if self.mobs[m].hp <= 0 {
                continue;
            }
            self.mobs[m].hurt = self.mobs[m].hurt.saturating_sub(1);
            let kind = KINDS[self.mobs[m].kind];
            let delay = if self.mobs[m].awake { kind.delay } else { MOB_DELAY * 2 };
            self.mobs[m].timer += 1;
            if self.mobs[m].timer < delay {
                continue;
            }
            self.mobs[m].timer = 0;
            let (x, y) = (self.mobs[m].x, self.mobs[m].y);
            let (hx, hy) = (self.hero.x, self.hero.y);
            let visible = self.idx(x, y).is_some_and(|i| self.seen[i]);
            if visible && (x - hx).abs() + (y - hy).abs() <= SIGHT {
                self.mobs[m].awake = true;
            }
            if (x - hx).abs() + (y - hy).abs() == 1 && self.mobs[m].awake {
                let dmg = (self.mobs[m].atk - self.hero.def + rng.range(0, 2)).max(1);
                self.hero.hp -= dmg;
                self.hero.hurt = 4;
                self.burst(hx, hy, Rgb(255, 40, 40), 5, rng);
                self.say(format!("The {} hits the hero", kind.name));
                if self.hero.hp <= 0 {
                    self.phase = Phase::Dead(0);
                    self.say(format!("The hero falls on depth {}", self.depth));
                    return;
                }
                continue;
            }
            let erratic = kind.name == "bat" && rng.chance(0.4);
            let (dx, dy) = if self.mobs[m].awake && !erratic {
                // Step to the neighbour closest to the hero.
                *[(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .filter(|&&(dx, dy)| self.tile(x + dx, y + dy).walkable() && !self.occupied(x + dx, y + dy))
                    .min_by_key(|&&(dx, dy)| (x + dx - hx).abs() + (y + dy - hy).abs())
                    .unwrap_or(&(0, 0))
            } else if rng.chance(0.5) {
                *rng.pick(&[(1, 0), (-1, 0), (0, 1), (0, -1)])
            } else {
                (0, 0)
            };
            let (nx, ny) = (x + dx, y + dy);
            if (dx, dy) != (0, 0) && self.tile(nx, ny).walkable() && !self.occupied(nx, ny) {
                if self.tile(nx, ny) == Tile::Door {
                    if let Some(i) = self.idx(nx, ny) {
                        self.map[i] = Tile::OpenDoor;
                    }
                } else {
                    self.mobs[m].x = nx;
                    self.mobs[m].y = ny;
                }
            }
        }
    }

    fn update(&mut self, rng: &mut Rng) {
        if self.regen {
            self.regen = false;
            self.generate(rng);
        }
        self.frame = self.frame.wrapping_add(1);
        self.message_age += 1;
        for s in self.sparks.iter_mut() {
            s.x += s.vx;
            s.y += s.vy;
            s.vy += 0.08;
            s.life -= 1;
        }
        self.sparks.retain(|s| s.life > 0);
        if !self.big_enough() {
            return;
        }
        match self.phase {
            Phase::Building => {
                self.budget += 1.0;
                while let Some(step) = self.steps.front() {
                    if step.cost > self.budget {
                        break;
                    }
                    self.budget -= step.cost;
                    let step = self.steps.pop_front().unwrap();
                    for (i, t) in step.tiles {
                        self.shown[i] = t;
                    }
                    match step.reveal {
                        Some(Reveal::Mob(k)) => self.mobs[k].shown = true,
                        Some(Reveal::Item(k)) => self.items[k].shown = true,
                        Some(Reveal::Torch(k)) => self.torches[k].1 = true,
                        None => {}
                    }
                }
                if self.steps.is_empty() {
                    self.phase = Phase::Exploring;
                    self.shown.clone_from(&self.map);
                    self.look();
                    self.say(format!("A hero enters depth {}", self.depth));
                }
            }
            Phase::Exploring => {
                self.level_frames += 1;
                if self.frame % 60 == 0 && self.hero.hp < self.hero.max {
                    self.hero.hp += 1;
                }
                self.hero.hurt = self.hero.hurt.saturating_sub(1);
                self.hero.timer += 1;
                if self.hero.timer >= HERO_DELAY {
                    self.hero.timer = 0;
                    self.hero_act(rng);
                    self.look();
                }
                if self.phase == Phase::Exploring {
                    self.mobs_act(rng);
                }
            }
            Phase::Descending(t) => {
                if t >= 25 {
                    self.depth += 1;
                    self.hero.max += 2;
                    self.hero.hp = (self.hero.hp + 4).min(self.hero.max);
                    self.generate(rng);
                } else {
                    self.phase = Phase::Descending(t + 1);
                }
            }
            Phase::Dead(t) => {
                if t >= 70 {
                    self.depth = 1;
                    self.hero = Hero::fresh();
                    self.generate(rng);
                } else {
                    self.phase = Phase::Dead(t + 1);
                }
            }
        }
    }

    // ----- drawing -----

    fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        for py in y.max(0)..(y + h).min(self.ph as i32) {
            for px in x.max(0)..(x + w).min(self.w as i32) {
                self.px[py as usize * self.w + px as usize] = c;
            }
        }
    }

    /// Draw a 5x5 sprite into a tile. Shrinking weighs every source pixel a
    /// target pixel covers, so small tiles keep each sprite's silhouette and
    /// color instead of a few arbitrary pixels.
    fn sprite(&mut self, tx: i32, ty: i32, sprite: &Sprite, light: f32, flash: bool) {
        let t = self.t;
        let (x0, y0) = (self.ox + tx * t, self.oy + ty * t);
        let k = 5.0 / t as f32;
        for py in 0..t {
            for px in 0..t {
                let (sx0, sy0) = (px as f32 * k, py as f32 * k);
                let (mut sum, mut cover) = ([0.0f32; 3], 0.0);
                for sy in sy0 as usize..((sy0 + k).ceil() as usize).min(5) {
                    let wy = ((sy + 1) as f32).min(sy0 + k) - (sy as f32).max(sy0);
                    for sx in sx0 as usize..((sx0 + k).ceil() as usize).min(5) {
                        let wx = ((sx + 1) as f32).min(sx0 + k) - (sx as f32).max(sx0);
                        if let Some(c) = ink(sprite[sy].as_bytes()[sx]) {
                            let wgt = wx * wy;
                            cover += wgt;
                            sum[0] += c.0 as f32 * wgt;
                            sum[1] += c.1 as f32 * wgt;
                            sum[2] += c.2 as f32 * wgt;
                        }
                    }
                }
                if cover >= 0.4 * k * k {
                    let c = Rgb((sum[0] / cover) as u8, (sum[1] / cover) as u8, (sum[2] / cover) as u8);
                    let c = if flash { Rgb(255, 80, 70) } else { c.scale(light) };
                    self.fill(x0 + px, y0 + py, 1, 1, c);
                }
            }
        }
    }

    /// How brightly tile `i` shows: lit by the hero, remembered, or unseen.
    fn light(&self, i: usize) -> Option<f32> {
        if self.phase == Phase::Building {
            return Some(1.0);
        }
        if !self.known[i] {
            return None;
        }
        if !self.seen[i] {
            return Some(0.3);
        }
        let (x, y) = ((i as i32 % self.mw - self.hero.x) as f32, (i as i32 / self.mw - self.hero.y) as f32);
        let d = (x * x + y * y).sqrt() / SIGHT as f32;
        let flicker = 0.96 + 0.04 * (self.frame as f32 * 0.7).sin();
        Some(((1.0 - 0.55 * d * d) * flicker).clamp(0.35, 1.0))
    }

    fn draw_tile(&mut self, tx: i32, ty: i32) {
        let i = (ty * self.mw + tx) as usize;
        let Some(light) = self.light(i) else { return };
        let dim = light < 0.31;
        let tint = |c: Rgb| {
            let c = c.scale(light);
            if dim {
                c.lerp(Rgb(20, 26, 48), 0.35)
            } else {
                c
            }
        };
        let t = self.t;
        let (x0, y0) = (self.ox + tx * t, self.oy + ty * t);
        let tile = if self.phase == Phase::Building { self.shown[i] } else { self.map[i] };
        let h = hash(tx, ty, self.seed);
        match tile {
            Tile::Rock => {
                let near = |dx: i32, dy: i32| {
                    self.idx(tx + dx, ty + dy).is_some_and(|j| {
                        let t = if self.phase == Phase::Building { self.shown[j] } else { self.map[j] };
                        t != Tile::Rock
                    })
                };
                let wall = (-1..=1).any(|dy| (-1..=1).any(|dx| near(dx, dy)));
                if !wall {
                    return;
                }
                // Stone blocks with a lit top edge, a shaded bottom and mortar.
                self.fill(x0, y0, t, t, tint(STONE));
                self.fill(x0, y0, t, 1, tint(STONE_HI));
                self.fill(x0, y0 + t - 1, t, 1, tint(STONE_LO));
                if t >= 4 {
                    let mid = if ty % 2 == 0 { t / 2 } else { 0 };
                    self.fill(x0 + mid, y0 + 1, 1, t - 2, tint(STONE_LO));
                }
            }
            Tile::Floor | Tile::Up | Tile::Down => {
                self.fill(x0, y0, t, t, tint(FLOOR));
                self.fill(x0 + (h % t as u32) as i32, y0 + (h / 7 % t as u32) as i32, 1, 1, tint(FLOOR_HI));
                if tile != Tile::Floor {
                    // Steps: diagonal treads, pale going up and dark going down.
                    let (a, b) = if tile == Tile::Up { (Rgb(200, 190, 170), Rgb(120, 110, 100)) } else { (Rgb(70, 60, 55), Rgb(10, 8, 8)) };
                    for k in 0..t {
                        self.fill(x0 + k, y0 + k, t - k, 1, tint(if k % 2 == 0 { a } else { b }));
                    }
                }
            }
            Tile::Corridor => {
                self.fill(x0, y0, t, t, tint(CORRIDOR));
            }
            Tile::Door | Tile::OpenDoor => {
                self.fill(x0, y0, t, t, tint(CORRIDOR));
                if tile == Tile::Door {
                    self.fill(x0, y0, t, t, tint(WOOD));
                    self.fill(x0 + t / 2, y0, 1, t, tint(WOOD_DARK));
                    if t >= 4 {
                        self.fill(x0 + t - 2, y0 + t / 2, 1, 1, tint(Rgb(250, 210, 80)));
                    }
                } else {
                    self.fill(x0, y0, 1, t, tint(WOOD));
                    self.fill(x0 + t - 1, y0, 1, t, tint(WOOD));
                }
            }
        }
    }

    fn draw(&mut self) {
        self.px.fill(BG);
        let (w, t) = (self.w as i32, self.t);
        self.fill(0, 0, w, 2, HUD_BG);
        if !self.big_enough() {
            return;
        }
        for ty in 0..self.mh {
            for tx in 0..self.mw {
                self.draw_tile(tx, ty);
            }
        }
        for k in 0..self.torches.len() {
            let (i, lit) = self.torches[k];
            if !lit || self.light(i).is_none() {
                continue;
            }
            let (tx, ty) = (i as i32 % self.mw, i as i32 / self.mw);
            let (x0, y0) = (self.ox + tx * t + t / 2, self.oy + ty * t + t / 2);
            let flame = if (self.frame / 3 + k as u32) % 2 == 0 { Rgb(255, 200, 60) } else { Rgb(255, 120, 30) };
            self.fill(x0, y0 - 1, 1, 1, flame);
            self.fill(x0, y0, 1, 1, Rgb(255, 150, 40));
            self.fill(x0, y0 + 1, 1, 1, WOOD_DARK);
        }
        for k in 0..self.items.len() {
            let it = &self.items[k];
            let Some(i) = self.idx(it.x, it.y) else { continue };
            if !it.shown || it.taken {
                continue;
            }
            let (x, y, s) = (it.x, it.y, it.loot.sprite());
            if let Some(l) = self.light(i) {
                self.sprite(x, y, &s, l, false);
            }
        }
        let building = self.phase == Phase::Building;
        for k in 0..self.mobs.len() {
            let m = &self.mobs[k];
            let Some(i) = self.idx(m.x, m.y) else { continue };
            if !m.shown || m.hp <= 0 || !(building || self.seen[i]) {
                continue;
            }
            let (x, y, s, flash) = (m.x, m.y, KINDS[m.kind].sprite, m.hurt > 0 && m.hurt % 2 == 0);
            let l = self.light(i).unwrap_or(1.0);
            self.sprite(x, y, &s, l, flash);
        }
        match self.phase {
            Phase::Exploring | Phase::Descending(_) => {
                let (x, y, flash) = (self.hero.x, self.hero.y, self.hero.hurt > 0 && self.hero.hurt % 2 == 0);
                self.sprite(x, y, &HERO, 1.0, flash);
            }
            Phase::Dead(_) => {
                let (x, y) = (self.hero.x, self.hero.y);
                self.sprite(x, y, &TOMB, 1.0, false);
            }
            Phase::Building => {}
        }
        for k in 0..self.sparks.len() {
            let s = &self.sparks[k];
            let (x, y, c) = (s.x as i32, s.y as i32, s.color.scale(0.4 + s.life as f32 / 14.0));
            if y >= 2 {
                self.fill(x, y, 1, 1, c);
            }
        }
        // Fade to black on the way down, red when the hero dies.
        let fade = match self.phase {
            Phase::Descending(t) => 1.0 - t as f32 / 25.0,
            Phase::Dead(t) => 1.0 - (t as f32 / 70.0).powi(2),
            _ => 1.0,
        };
        if fade < 1.0 {
            let red = matches!(self.phase, Phase::Dead(_));
            for p in self.px[2 * self.w..].iter_mut() {
                *p = if red { p.lerp(Rgb(90, 0, 0), 1.0 - fade).scale(fade.max(0.3)) } else { p.scale(fade) };
            }
        }
    }

    fn hud(&self) -> String {
        let h = &self.hero;
        let mut s = format!(" Depth {}  HP {}/{}  Gold {}  Atk {} Def {}", self.depth, h.hp.max(0), h.max, h.gold, h.atk, h.def);
        if h.potions > 0 {
            s += &format!("  Potions {}", h.potions);
        }
        s
    }
}

impl Hero {
    fn fresh() -> Hero {
        Hero { x: 0, y: 0, hp: 20, max: 20, atk: 3, def: 0, gold: 0, potions: 0, hurt: 0, timer: 0 }
    }
}

/// Recursively split a region into leaves, recording each sibling pair.
#[allow(clippy::too_many_arguments)]
fn split(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    rng: &mut Rng,
    leaves: &mut Vec<(i32, i32, i32, i32)>,
    links: &mut Vec<((i32, i32, i32, i32), (i32, i32, i32, i32))>,
    depth: u32,
) {
    let (min_w, min_h) = (8, 6);
    let can_v = w >= 2 * min_w;
    let can_h = h >= 2 * min_h;
    if depth > 5 || (!can_v && !can_h) || (depth > 1 && rng.chance(0.15)) {
        leaves.push((x, y, w, h));
        return;
    }
    let vertical = if can_v && can_h { w as f32 / h as f32 > 1.3 || (w >= h && rng.chance(0.5)) } else { can_v };
    if vertical {
        let cut = rng.range(min_w, w - min_w + 1);
        links.push(((x, y, cut, h), (x + cut, y, w - cut, h)));
        split(x, y, cut, h, rng, leaves, links, depth + 1);
        split(x + cut, y, w - cut, h, rng, leaves, links, depth + 1);
    } else {
        let cut = rng.range(min_h, h - min_h + 1);
        links.push(((x, y, w, cut), (x, y + cut, w, h - cut)));
        split(x, y, w, cut, rng, leaves, links, depth + 1);
        split(x, y + cut, w, h - cut, rng, leaves, links, depth + 1);
    }
}

impl Animation for Dungeon {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        self.update(rng);
        if self.w == 0 || self.h == 0 {
            return;
        }
        self.draw();
        c.blit_pixels(&self.px);
        let hud = self.hud();
        c.text(0, 0, &hud, HUD_FG);
        if self.message_age < 90 && !self.message.is_empty() {
            let x = self.w as i32 - self.message.chars().count() as i32 - 1;
            if x > hud.chars().count() as i32 + 2 {
                let fade = if self.message_age > 60 { (90 - self.message_age) as f32 / 30.0 } else { 1.0 };
                c.text(x, 0, &self.message, HUD_BG.lerp(Rgb(250, 220, 140), fade));
            }
        }
    }

    /// Space throws the level away and builds a new one at the same depth.
    fn key(&mut self, key: crate::Key) {
        if key == crate::Key::Space {
            self.regen = true;
        }
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    Box::new(Dungeon::new(w, h, rng))
}
