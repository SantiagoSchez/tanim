//! The 3D Maze screensaver of Windows 95 and NT: a first-person walk through
//! a brick maze with a wooden floor and a tiled ceiling, past spinning
//! polyhedra that turn the world upside down when touched and rats scurrying
//! along the corridors, to the smiley face waiting at the exit.
//!
//! The view is a raycaster at half-block pixel resolution: one ray per column
//! steps through the tile grid to the first wall, whose distance sets the
//! height of its slice, and every floor and ceiling pixel is cast back onto
//! its plane from its row. Textures are generated at start-up with mipmaps,
//! picked by how many texels fall on a pixel, so distant walls and floor stay
//! calm instead of shimmering, and they shade in a few flat steps rather than
//! smoothly, which keeps the output small while everything moves. The depth
//! buffer of that pass hides the objects behind walls: the polyhedra are real
//! meshes rasterized as flat-shaded triangles, the smiley is a disc spinning
//! on its vertical axis and the rats are billboards seen from the side, the
//! front or the back.
//!
//! Each round carves a perfect maze of 8 to 14 cells a side, starts in a
//! dead end facing the START sign and explores depth first: straight on when
//! it can, into dead ends and back, until it meets the smiley in the cell
//! farthest from the start. Touching a polyhedron rolls the view half a turn;
//! while rolling, the frame is rendered on a larger square and rotated.
//! Space shows or hides a map of the explored part in the corner.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Texture size in texels, and mipmap levels (64 down to 2).
const TEX: usize = 64;
const LEVELS: usize = 6;
/// Eye height; walls are one tile tall.
const EYE: f32 = 0.5;
/// Tiles per frame while walking; frames per about-turn and per roll.
const SPEED: f32 = 0.075;
const SPIN: f32 = 18.0;
const ROLL: f32 = 54.0;
const FADE: u32 = 20;
/// Frames spent in front of the START sign before setting off.
const HOLD: u32 = 45;
/// Distance at which the camera touches the smiley or a polyhedron.
const TOUCH: f32 = 0.5;
/// East, south, west, north: y grows downwards, so direction `k` has the
/// angle `k * PI / 2`.
const DIRS: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

/// The START sign, drawn on the wall at the back of the first dead end.
const SIGN_W: usize = 36;
const SIGN_H: usize = 13;
const FONT: [[u8; 7]; 4] = [
    [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
    [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
    [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
];
const START: [usize; 5] = [0, 1, 2, 3, 1];

const YELLOW: Rgb = Rgb(255, 216, 40);
const FUR: Rgb = Rgb(126, 116, 110);
const PINK: Rgb = Rgb(222, 150, 150);

/// The maze as a tile grid: cell `(cx, cy)` is tile `(2cx + 1, 2cy + 1)` and
/// the tiles between cells are open where there is a passage.
struct Grid {
    n: i32,
    t: i32,
    wall: Vec<bool>,
}

impl Grid {
    /// A perfect maze by recursive backtracking.
    fn carve(n: i32, rng: &mut Rng) -> Grid {
        let t = 2 * n + 1;
        let mut g = Grid { n, t, wall: vec![true; (t * t) as usize] };
        let mut seen = vec![false; (n * n) as usize];
        let first = (rng.range(0, n), rng.range(0, n));
        seen[(first.1 * n + first.0) as usize] = true;
        g.open(2 * first.0 + 1, 2 * first.1 + 1);
        let mut stack = vec![first];
        while let Some(&(cx, cy)) = stack.last() {
            let mut opts = [0; 4];
            let mut k = 0;
            for (d, &(dx, dy)) in DIRS.iter().enumerate() {
                let (nx, ny) = (cx + dx, cy + dy);
                if nx >= 0 && ny >= 0 && nx < n && ny < n && !seen[(ny * n + nx) as usize] {
                    opts[k] = d;
                    k += 1;
                }
            }
            if k == 0 {
                stack.pop();
                continue;
            }
            let (dx, dy) = DIRS[opts[rng.below(k)]];
            let (nx, ny) = (cx + dx, cy + dy);
            seen[(ny * n + nx) as usize] = true;
            g.open(2 * cx + 1 + dx, 2 * cy + 1 + dy);
            g.open(2 * nx + 1, 2 * ny + 1);
            stack.push((nx, ny));
        }
        g
    }

    fn open(&mut self, x: i32, y: i32) {
        self.wall[(y * self.t + x) as usize] = false;
    }

    fn solid(&self, x: i32, y: i32) -> bool {
        x < 0 || y < 0 || x >= self.t || y >= self.t || self.wall[(y * self.t + x) as usize]
    }

    /// Whether a cell has a passage towards direction `d`.
    fn opens(&self, (cx, cy): (i32, i32), d: usize) -> bool {
        !self.solid(2 * cx + 1 + DIRS[d].0, 2 * cy + 1 + DIRS[d].1)
    }

    fn cell(&self, (cx, cy): (i32, i32)) -> usize {
        (cy * self.n + cx) as usize
    }

    fn exits(&self, c: (i32, i32)) -> impl Iterator<Item = usize> + '_ {
        (0..4).filter(move |&d| self.opens(c, d))
    }

    /// Steps from `from` to every cell.
    fn distances(&self, from: (i32, i32)) -> Vec<u32> {
        let mut dist = vec![u32::MAX; (self.n * self.n) as usize];
        dist[self.cell(from)] = 0;
        let mut queue = std::collections::VecDeque::from([from]);
        while let Some(c) = queue.pop_front() {
            for d in self.exits(c) {
                let nb = step(c, d);
                if dist[self.cell(nb)] == u32::MAX {
                    dist[self.cell(nb)] = dist[self.cell(c)] + 1;
                    queue.push_back(nb);
                }
            }
        }
        dist
    }
}

fn step((cx, cy): (i32, i32), d: usize) -> (i32, i32) {
    (cx + DIRS[d].0, cy + DIRS[d].1)
}

fn center((cx, cy): (i32, i32)) -> (f32, f32) {
    (2.0 * cx as f32 + 1.5, 2.0 * cy as f32 + 1.5)
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn fog(d: f32) -> f32 {
    1.0 / (1.0 + 0.008 * d * d)
}

/// Mipmap level for a pixel covering `tpp` texels.
fn level(tpp: f32) -> usize {
    if tpp <= 1.0 {
        0
    } else {
        (tpp.log2() as usize).min(LEVELS - 1)
    }
}

/// A square texture tiling every world unit, with its mipmaps.
struct Tex(Vec<Vec<Rgb>>);

impl Tex {
    fn new(base: Vec<Rgb>) -> Tex {
        let mut levels = vec![base];
        for l in 1..LEVELS {
            let s = TEX >> l;
            let p = &levels[l - 1];
            let mut v = Vec::with_capacity(s * s);
            for y in 0..s {
                for x in 0..s {
                    let mut acc = [0u32; 3];
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let c = p[(2 * y + dy) * 2 * s + 2 * x + dx];
                        acc[0] += c.0 as u32;
                        acc[1] += c.1 as u32;
                        acc[2] += c.2 as u32;
                    }
                    v.push(Rgb((acc[0] / 4) as u8, (acc[1] / 4) as u8, (acc[2] / 4) as u8));
                }
            }
            levels.push(v);
        }
        Tex(levels)
    }

    #[inline]
    fn at(&self, l: usize, u: f32, v: f32) -> Rgb {
        let s = TEX >> l;
        let m = s as i32 - 1;
        let x = ((u * s as f32).floor() as i32 & m) as usize;
        let y = ((v * s as f32).floor() as i32 & m) as usize;
        self.0[l][y * s + x]
    }
}

/// Smooth value noise over the texture, tiling, with `nx` by `ny` lattice
/// points (fewer along an axis stretches it that way).
struct Noise {
    nx: usize,
    ny: usize,
    v: Vec<f32>,
}

impl Noise {
    fn new(nx: usize, ny: usize, rng: &mut Rng) -> Noise {
        Noise { nx, ny, v: (0..nx * ny).map(|_| rng.f32()).collect() }
    }

    fn at(&self, x: usize, y: usize) -> f32 {
        let fx = x as f32 * self.nx as f32 / TEX as f32;
        let fy = y as f32 * self.ny as f32 / TEX as f32;
        let (x0, y0) = (fx as usize, fy as usize);
        let (tx, ty) = (smooth(fx - x0 as f32), smooth(fy - y0 as f32));
        let g = |i: usize, j: usize| self.v[(j % self.ny) * self.nx + i % self.nx];
        let top = g(x0, y0) + (g(x0 + 1, y0) - g(x0, y0)) * tx;
        let bot = g(x0, y0 + 1) + (g(x0 + 1, y0 + 1) - g(x0, y0 + 1)) * tx;
        top + (bot - top) * ty
    }
}

fn texture(f: impl Fn(usize, usize) -> Rgb) -> Tex {
    Tex::new((0..TEX * TEX).map(|i| f(i % TEX, i / TEX)).collect())
}

// The textures vary in a few flat steps rather than smoothly: while walking
// nearly every pixel moves, and flat patches are what keeps cells unchanged.

/// Red bricks in four courses of two, offset every other course.
fn bricks(rng: &mut Rng) -> Tex {
    let noise = Noise::new(16, 16, rng);
    let grit = Noise::new(32, 32, rng);
    let tones: Vec<f32> = (0..8).map(|_| rng.rangef(0.82, 1.12)).collect();
    texture(|x, y| {
        let (row, yy) = (y / 16, y % 16);
        let xs = (x + 16 * (row % 2)) % TEX;
        let (col, xx) = (xs / 32, xs % 32);
        if yy >= 14 || xx < 2 {
            return Rgb(168, 160, 146).scale(0.94 + 0.06 * (grit.at(x, y) * 2.0).floor());
        }
        let bevel = match (yy, xx) {
            (0, _) => 1.14,
            (13, _) => 0.78,
            (_, 2) => 1.06,
            (_, 31) => 0.86,
            _ => 1.0,
        };
        let k = tones[row * 2 + col] * (0.92 + 0.06 * (noise.at(x, y) * 3.0).floor()) * bevel;
        Rgb(156, 62, 42).scale(k)
    })
}

/// Wooden planks running east to west, with staggered joints.
fn planks(rng: &mut Rng) -> Tex {
    let grain = Noise::new(4, 32, rng);
    let fine = Noise::new(16, 64, rng);
    let tones: Vec<f32> = (0..4).map(|_| rng.rangef(0.84, 1.1)).collect();
    let joints: Vec<usize> = (0..4).map(|_| rng.below(TEX)).collect();
    texture(|x, y| {
        let (plank, yy) = (y / 16, y % 16);
        if yy == 15 || x == joints[plank] {
            return Rgb(52, 32, 18);
        }
        let ring = ((yy as f32 + 6.0 * grain.at(x, y)) * 1.1).sin() * 0.5 + 0.5;
        let k = tones[plank] * (0.88 + 0.06 * (ring * 2.0 + fine.at(x, y)).floor());
        Rgb(146, 96, 54).scale(k)
    })
}

/// Four pale acoustic ceiling tiles per world unit, pierced by pinholes.
fn ceiling(rng: &mut Rng) -> Tex {
    let noise = Noise::new(32, 32, rng);
    let holes: Vec<bool> = (0..TEX * TEX).map(|_| rng.chance(0.06)).collect();
    texture(|x, y| {
        let (xx, yy) = (x % 32, y % 32);
        if xx == 0 || yy == 0 {
            return Rgb(122, 120, 114);
        }
        if xx == 31 || yy == 31 {
            return Rgb(160, 158, 150);
        }
        let base = Rgb(206, 204, 194).scale(0.95 + 0.04 * (noise.at(x, y) * 2.0).floor());
        if xx == 1 || yy == 1 {
            base.scale(1.08)
        } else if holes[y * TEX + x] {
            base.scale(0.82)
        } else {
            base
        }
    })
}

fn sign() -> Vec<Rgb> {
    let mut px = vec![Rgb(236, 228, 206); SIGN_W * SIGN_H];
    for y in 0..SIGN_H {
        for x in 0..SIGN_W {
            if x == 0 || y == 0 || x == SIGN_W - 1 || y == SIGN_H - 1 {
                px[y * SIGN_W + x] = Rgb(120, 24, 20);
            }
        }
    }
    for (i, &g) in START.iter().enumerate() {
        for (row, bits) in FONT[g].iter().enumerate() {
            for col in 0..5 {
                if bits >> (4 - col) & 1 == 1 {
                    px[(3 + row) * SIGN_W + 4 + i * 6 + col] = Rgb(200, 30, 26);
                }
            }
        }
    }
    px
}

/// A convex solid with triangular faces, vertices on the unit sphere.
struct Mesh {
    v: Vec<[f32; 3]>,
    f: Vec<[usize; 3]>,
}

impl Mesh {
    /// The faces are the triples of vertices all at the shortest edge
    /// length from each other, which holds for the deltahedra used here.
    fn new(v: &[[f32; 3]]) -> Mesh {
        let v: Vec<[f32; 3]> = v.iter().map(|&p| norm(p)).collect();
        let dist = |a: usize, b: usize| len(sub(v[a], v[b]));
        let mut edge = f32::MAX;
        for a in 0..v.len() {
            for b in a + 1..v.len() {
                edge = edge.min(dist(a, b));
            }
        }
        let near = |a: usize, b: usize| dist(a, b) < edge * 1.01;
        let mut f = Vec::new();
        for a in 0..v.len() {
            for b in a + 1..v.len() {
                for c in b + 1..v.len() {
                    if near(a, b) && near(b, c) && near(a, c) {
                        f.push([a, b, c]);
                    }
                }
            }
        }
        Mesh { v, f }
    }
}

fn solids() -> Vec<Mesh> {
    let p = (1.0 + 5f32.sqrt()) / 2.0;
    let mut ico = Vec::new();
    for a in [-1.0, 1.0] {
        for b in [-p, p] {
            ico.extend([[0.0, a, b], [a, b, 0.0], [b, 0.0, a]]);
        }
    }
    vec![
        Mesh::new(&[[1.0, 1.0, 1.0], [1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [-1.0, -1.0, 1.0]]),
        Mesh::new(&[
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ]),
        Mesh::new(&ico),
    ]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn len(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = len(a).max(1e-9);
    [a[0] / l, a[1] / l, a[2] / l]
}

struct Poly {
    x: f32,
    y: f32,
    mesh: usize,
    hue: f32,
    /// Spin about the vertical axis and about a tilted one, and their speeds.
    yaw: f32,
    tilt: f32,
    spin: (f32, f32),
    bob: f32,
    alive: bool,
}

struct Rat {
    from: (i32, i32),
    to: (i32, i32),
    /// Progress from `from` to `to`.
    t: f32,
    speed: f32,
    /// Leg cycle, and frames left sniffing about before moving on.
    gait: f32,
    wait: u32,
}

impl Rat {
    fn pos(&self) -> (f32, f32) {
        let (a, b) = (center(self.from), center(self.to));
        (a.0 + (b.0 - a.0) * self.t, a.1 + (b.1 - a.1) * self.t)
    }
}

/// A stretch of the camera's walk. Corners are taken on a quarter circle
/// around the wall's corner, from the edge of the cell to the next edge;
/// dead ends are turned around on the spot, in the middle of the cell.
#[derive(Clone, Copy)]
enum Seg {
    Line { x0: f32, y0: f32, x1: f32, y1: f32 },
    /// Around `(ox, oy)` at radius one half, from angle `t0` by `turn`,
    /// while the heading turns by as much from `a0`.
    Arc { ox: f32, oy: f32, t0: f32, a0: f32, turn: f32 },
    Spin { a0: f32, turn: f32 },
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    In,
    Walk,
    Out,
}

/// The camera for one frame, looking along `(dx, dy)` with `(rx, ry)` to its
/// right, projecting onto a `w x h` pixel buffer.
struct View {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    rx: f32,
    ry: f32,
    focal: f32,
    w: usize,
    h: usize,
}

impl View {
    /// World point to camera space: right, up and forward.
    fn cam(&self, wx: f32, wy: f32, wz: f32) -> (f32, f32, f32) {
        let (ox, oy) = (wx - self.x, wy - self.y);
        (ox * self.rx + oy * self.ry, wz - EYE, ox * self.dx + oy * self.dy)
    }

    fn screen(&self, r: f32, up: f32, f: f32) -> (f32, f32) {
        (self.w as f32 / 2.0 + self.focal * r / f, self.h as f32 / 2.0 - self.focal * up / f)
    }

    /// Pixel columns and rows of a box around `(sx, sy)`, clipped.
    fn span(&self, sx: f32, sy: f32, hw: f32, top: f32, bot: f32) -> (usize, usize, usize, usize) {
        let x0 = (sx - hw).floor().max(0.0) as usize;
        let x1 = ((sx + hw).ceil().max(0.0) as usize).min(self.w);
        let y0 = (sy - top).floor().max(0.0) as usize;
        let y1 = ((sy + bot).ceil().max(0.0) as usize).min(self.h);
        (x0, x1, y0, y1)
    }
}

struct Hit {
    d: f32,
    u: f32,
    xside: bool,
    tile: (i32, i32),
    face: usize,
}

pub struct Maze3d {
    w: usize,
    ph: usize,
    focal: f32,
    wall: Tex,
    floor: Tex,
    ceil: Tex,
    sign: Vec<Rgb>,
    solids: Vec<Mesh>,
    grid: Grid,
    x: f32,
    y: f32,
    a: f32,
    /// The cell last entered and the one being walked to.
    cell: (i32, i32),
    next: (i32, i32),
    path: std::collections::VecDeque<Seg>,
    /// Progress along the first segment of `path`.
    along: f32,
    stack: Vec<(i32, i32)>,
    visited: Vec<bool>,
    /// Tiles shown on the map, and the ones walked through.
    seen: Vec<bool>,
    trail: Vec<bool>,
    /// Wall tile and face carrying the START sign.
    sign_at: ((i32, i32), usize),
    exit: (i32, i32),
    smiley: bool,
    polys: Vec<Poly>,
    rats: Vec<Rat>,
    roll: f32,
    roll_from: f32,
    roll_to: f32,
    roll_t: f32,
    phase: Phase,
    timer: u32,
    t: u32,
    map: bool,
    px: Vec<Rgb>,
    zb: Vec<f32>,
    big: Vec<Rgb>,
    bigz: Vec<f32>,
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    Box::new(Maze3d::new(w, h, rng))
}

impl Maze3d {
    fn new(w: usize, h: usize, rng: &mut Rng) -> Maze3d {
        let ph = 2 * h;
        // Fit about 70 degrees across and 56 down, whichever is tighter.
        let focal = (w as f32 * 0.5 / 0.7).min(ph as f32 * 0.5 / 0.53).max(0.5);
        let mut m = Maze3d {
            w,
            ph,
            focal,
            wall: bricks(rng),
            floor: planks(rng),
            ceil: ceiling(rng),
            sign: sign(),
            solids: solids(),
            grid: Grid { n: 0, t: 0, wall: Vec::new() },
            x: 0.0,
            y: 0.0,
            a: 0.0,
            cell: (0, 0),
            next: (0, 0),
            path: Default::default(),
            along: 0.0,
            stack: Vec::new(),
            visited: Vec::new(),
            seen: Vec::new(),
            trail: Vec::new(),
            sign_at: ((0, 0), 0),
            exit: (0, 0),
            smiley: true,
            polys: Vec::new(),
            rats: Vec::new(),
            roll: 0.0,
            roll_from: 0.0,
            roll_to: 0.0,
            roll_t: 1.0,
            phase: Phase::In,
            timer: 0,
            t: 0,
            map: false,
            px: vec![Rgb::BLACK; w * ph],
            zb: vec![0.0; w * ph],
            big: Vec::new(),
            bigz: Vec::new(),
        };
        m.reset(rng);
        m
    }

    /// A new maze, with the camera in a dead end looking at the START sign.
    fn reset(&mut self, rng: &mut Rng) {
        let n = rng.range(8, 15);
        self.grid = Grid::carve(n, rng);
        let g = &self.grid;
        let cells: Vec<(i32, i32)> = (0..n * n).map(|i| (i % n, i / n)).collect();
        let ends: Vec<(i32, i32)> = cells.iter().copied().filter(|&c| g.exits(c).count() == 1).collect();
        let start = *rng.pick(&ends);
        let k = g.exits(start).next().unwrap_or(0);
        let dist = g.distances(start);
        self.exit = cells.iter().copied().max_by_key(|&c| dist[g.cell(c)]).unwrap_or(start);

        let (sx, sy) = (2 * start.0 + 1, 2 * start.1 + 1);
        let (dx, dy) = DIRS[k];
        self.x = (sx + dx) as f32 + 0.5;
        self.y = (sy + dy) as f32 + 0.5;
        self.a = ((k + 2) % 4) as f32 * FRAC_PI_2;
        self.sign_at = ((sx - dx, sy - dy), k);
        self.cell = start;
        self.next = step(start, k);
        self.path.clear();
        self.along = 0.0;
        self.stack = vec![start];
        self.visited = vec![false; (n * n) as usize];
        self.visited[g.cell(start)] = true;
        let tiles = (g.t * g.t) as usize;
        self.seen = vec![false; tiles];
        self.trail = vec![false; tiles];
        self.smiley = true;

        let first = step(start, k);
        self.polys.clear();
        let count = 2 + rng.below(3);
        for _ in 0..100 {
            if self.polys.len() >= count {
                break;
            }
            let c = *rng.pick(&cells);
            let (x, y) = center(c);
            if c == start || c == first || c == self.exit || self.polys.iter().any(|p| p.x == x && p.y == y) {
                continue;
            }
            self.polys.push(Poly {
                x,
                y,
                mesh: rng.below(self.solids.len()),
                hue: rng.f32(),
                yaw: rng.rangef(0.0, TAU),
                tilt: rng.rangef(0.0, TAU),
                spin: (rng.rangef(0.03, 0.06), rng.rangef(0.015, 0.04)),
                bob: rng.rangef(0.0, TAU),
                alive: true,
            });
        }

        self.rats.clear();
        for _ in 0..1 + rng.below(3) {
            let from = *rng.pick(&cells);
            let d: Vec<usize> = self.grid.exits(from).collect();
            if d.is_empty() {
                continue;
            }
            self.rats.push(Rat {
                from,
                to: step(from, *rng.pick(&d)),
                t: rng.f32(),
                speed: rng.rangef(0.028, 0.04),
                gait: 0.0,
                wait: 0,
            });
        }

        self.roll = 0.0;
        self.roll_from = 0.0;
        self.roll_to = 0.0;
        self.roll_t = 1.0;
        self.phase = Phase::In;
        self.timer = 0;
    }

    /// Turn round from the START sign and head for the first cell.
    fn set_off(&mut self, rng: &mut Rng) {
        let (x1, y1) = center(self.next);
        let (cx, cy) = center(self.cell);
        let turn = if rng.chance(0.5) { PI } else { -PI };
        self.path.push_back(Seg::Spin { a0: self.a, turn });
        let (x0, y0) = (self.x, self.y);
        self.path.push_back(Seg::Line { x0, y0, x1: x1 - (x1 - cx) / 4.0, y1: y1 - (y1 - cy) / 4.0 });
    }

    /// At the edge of `self.next`: step in, pick where to go from it depth
    /// first and lay out the way to the edge of that cell. False when there
    /// is nowhere left to go.
    fn enter(&mut self, rng: &mut Rng) -> bool {
        let c = self.next;
        self.cell = c;
        let i = self.grid.cell(c);
        self.visited[i] = true;
        let (cx, cy) = center(c);
        let (x, y) = (self.x, self.y);
        if c == self.exit {
            // Walk up to the smiley; touching it ends the round.
            if (cx - x).hypot(cy - y) < 1e-3 {
                return false;
            }
            self.path.push_back(Seg::Line { x0: x, y0: y, x1: cx, y1: cy });
            return true;
        }
        let g = &self.grid;
        let ahead = ((self.a / FRAC_PI_2).round() as i32).rem_euclid(4) as usize;
        let opts: Vec<usize> = g.exits(c).filter(|&d| !self.visited[g.cell(step(c, d))]).collect();
        let d = if opts.is_empty() {
            let Some(back) = self.stack.pop() else {
                return false;
            };
            DIRS.iter().position(|&d| d == (back.0 - c.0, back.1 - c.1)).unwrap_or(0)
        } else {
            self.stack.push(c);
            if opts.contains(&ahead) && rng.chance(0.5) { ahead } else { *rng.pick(&opts) }
        };
        self.next = step(c, d);
        let (dx, dy) = (DIRS[d].0 as f32, DIRS[d].1 as f32);
        let (fx, fy) = (cx + 1.5 * dx, cy + 1.5 * dy);
        match (d + 4 - ahead) % 4 {
            0 => self.path.push_back(Seg::Line { x0: x, y0: y, x1: fx, y1: fy }),
            2 => {
                let turn = if rng.chance(0.5) { PI } else { -PI };
                self.path.push_back(Seg::Line { x0: x, y0: y, x1: cx, y1: cy });
                self.path.push_back(Seg::Spin { a0: self.a, turn });
                self.path.push_back(Seg::Line { x0: cx, y0: cy, x1: fx, y1: fy });
            }
            r => {
                let turn = if r == 1 { FRAC_PI_2 } else { -FRAC_PI_2 };
                let (ox, oy) = (x + 0.5 * dx, y + 0.5 * dy);
                let t0 = (y - oy).atan2(x - ox);
                self.path.push_back(Seg::Arc { ox, oy, t0, a0: self.a, turn });
                self.path.push_back(Seg::Line { x0: cx + 0.5 * dx, y0: cy + 0.5 * dy, x1: fx, y1: fy });
            }
        }
        true
    }

    fn walk(&mut self, rng: &mut Rng) {
        let mut left = SPEED;
        loop {
            let Some(&seg) = self.path.front() else {
                if self.enter(rng) {
                    continue;
                }
                return;
            };
            // Move along a segment of length `len` with what is left of this
            // frame's step; false while it is not done.
            let mut advance = |along: &mut f32, len: f32| {
                let s = (*along + left / len).min(1.0);
                left -= (s - *along) * len;
                *along = s;
                s >= 1.0
            };
            let done = match seg {
                Seg::Line { x0, y0, x1, y1 } => {
                    let done = advance(&mut self.along, (x1 - x0).hypot(y1 - y0).max(1e-4));
                    self.x = x0 + (x1 - x0) * self.along;
                    self.y = y0 + (y1 - y0) * self.along;
                    done
                }
                Seg::Arc { ox, oy, t0, a0, turn } => {
                    let done = advance(&mut self.along, 0.5 * FRAC_PI_2);
                    let t = t0 + turn * self.along;
                    self.x = ox + 0.5 * t.cos();
                    self.y = oy + 0.5 * t.sin();
                    self.a = a0 + turn * self.along;
                    done
                }
                Seg::Spin { a0, turn } => {
                    self.along = (self.along + 1.0 / SPIN).min(1.0);
                    self.a = a0 + turn * smooth(self.along);
                    left = 0.0;
                    self.along >= 1.0
                }
            };
            if !done {
                return;
            }
            self.path.pop_front();
            self.along = 0.0;
            if left <= 0.0 {
                return;
            }
        }
    }

    fn touch(&mut self) {
        let near = |x: f32, y: f32| (x - self.x).hypot(y - self.y) < TOUCH;
        if self.smiley && {
            let (ex, ey) = center(self.exit);
            near(ex, ey)
        } {
            self.smiley = false;
            self.phase = Phase::Out;
            self.timer = 0;
        }
        for p in &mut self.polys {
            if p.alive && near(p.x, p.y) {
                p.alive = false;
                self.roll_from = self.roll;
                self.roll_to += PI;
                self.roll_t = 0.0;
            }
        }
    }

    fn move_rats(&mut self, rng: &mut Rng) {
        for r in &mut self.rats {
            if r.wait > 0 {
                r.wait -= 1;
                continue;
            }
            r.t += r.speed / 2.0;
            r.gait += 0.8;
            if r.t < 1.0 {
                continue;
            }
            r.t -= 1.0;
            let here = r.to;
            let mut opts: Vec<usize> = self.grid.exits(here).collect();
            if opts.len() > 1 {
                opts.retain(|&d| step(here, d) != r.from);
            }
            r.from = here;
            r.to = step(here, *rng.pick(&opts));
            if rng.chance(0.15) {
                r.t = 0.0;
                r.wait = rng.range(10, 50) as u32;
            }
        }
    }

    fn view(&self, w: usize, h: usize) -> View {
        let (dy, dx) = self.a.sin_cos();
        View { x: self.x, y: self.y, dx, dy, rx: -dy, ry: dx, focal: self.focal, w, h }
    }

    /// First wall along a ray whose forward component is 1, so the distance
    /// found is the depth.
    fn cast(&self, rdx: f32, rdy: f32) -> Hit {
        let (mut mx, mut my) = (self.x.floor() as i32, self.y.floor() as i32);
        let ddx = if rdx == 0.0 { 1e30 } else { (1.0 / rdx).abs() };
        let ddy = if rdy == 0.0 { 1e30 } else { (1.0 / rdy).abs() };
        let (sx, mut side_x) = if rdx < 0.0 { (-1, (self.x - mx as f32) * ddx) } else { (1, (mx as f32 + 1.0 - self.x) * ddx) };
        let (sy, mut side_y) = if rdy < 0.0 { (-1, (self.y - my as f32) * ddy) } else { (1, (my as f32 + 1.0 - self.y) * ddy) };
        let mut xside = true;
        for _ in 0..4 * self.grid.t + 4 {
            if side_x < side_y {
                side_x += ddx;
                mx += sx;
                xside = true;
            } else {
                side_y += ddy;
                my += sy;
                xside = false;
            }
            if self.grid.solid(mx, my) {
                break;
            }
        }
        let d = if xside { side_x - ddx } else { side_y - ddy }.max(1e-3);
        let (u, face) = if xside {
            let f = (self.y + d * rdy).fract();
            if sx > 0 { (f, 2) } else { (1.0 - f, 0) }
        } else {
            let f = (self.x + d * rdx).fract();
            if sy > 0 { (1.0 - f, 3) } else { (f, 1) }
        };
        Hit { d, u, xside, tile: (mx, my), face }
    }

    /// Walls, floor and ceiling, then the objects, into a `w x h` buffer.
    fn render(&self, buf: &mut [Rgb], zb: &mut [f32], w: usize, h: usize) {
        let v = self.view(w, h);
        let (cx, hz) = (w as f32 / 2.0, h as f32 / 2.0);
        let f = self.focal;
        for col in 0..w {
            let k = (col as f32 + 0.5 - cx) / f;
            let (rdx, rdy) = (v.dx + v.rx * k, v.dy + v.ry * k);
            let hit = self.cast(rdx, rdy);
            let lh = f / hit.d;
            let top = hz - lh * (1.0 - EYE);
            let bot = hz + lh * EYE;
            let lvl = level(TEX as f32 / lh);
            let shade = if hit.xside { 0.8 } else { 1.0 } * fog(hit.d);
            let sign = (hit.tile, hit.face) == self.sign_at;
            for row in 0..h {
                let yc = row as f32 + 0.5;
                let i = row * w + col;
                if yc >= top && yc < bot {
                    let t = (yc - top) / lh;
                    let su = (hit.u - 0.1) / 0.8;
                    let sv = (t - 0.34) / (0.8 * SIGN_H as f32 / SIGN_W as f32);
                    let c = if sign && (0.0..1.0).contains(&su) && (0.0..1.0).contains(&sv) {
                        self.sign[(sv * SIGN_H as f32) as usize * SIGN_W + (su * SIGN_W as f32) as usize]
                    } else {
                        self.wall.at(lvl, hit.u, t)
                    };
                    buf[i] = c.scale(shade);
                    zb[i] = hit.d;
                } else {
                    let (d, tex) = if yc > hz {
                        (f * EYE / (yc - hz), &self.floor)
                    } else {
                        (f * (1.0 - EYE) / (hz - yc).max(1e-3), &self.ceil)
                    };
                    let lvl = level(TEX as f32 * d / f * (d / EYE).sqrt());
                    buf[i] = tex.at(lvl, self.x + rdx * d, self.y + rdy * d).scale(fog(d));
                    zb[i] = d;
                }
            }
        }
        self.draw_polys(&v, buf, zb);
        self.draw_smiley(&v, buf, zb);
        self.draw_rats(&v, buf, zb);
    }

    fn draw_polys(&self, v: &View, buf: &mut [Rgb], zb: &mut [f32]) {
        const R: f32 = 0.26;
        let eye = [self.x, self.y, EYE];
        for p in self.polys.iter().filter(|p| p.alive) {
            let t = self.t as f32;
            let (sy, cy) = (p.yaw + t * p.spin.0).sin_cos();
            let (st, ct) = (p.tilt + t * p.spin.1).sin_cos();
            let zc = EYE + 0.04 * (t * 0.05 + p.bob).sin();
            let mesh = &self.solids[p.mesh];
            let mut world = Vec::with_capacity(mesh.v.len());
            let mut proj = Vec::with_capacity(mesh.v.len());
            for &[x, y, z] in &mesh.v {
                // Tumble about the x axis, then turn about the vertical.
                let (y, z) = (y * ct - z * st, y * st + z * ct);
                let (x, y) = (x * cy - y * sy, x * sy + y * cy);
                let q = [p.x + x * R, p.y + y * R, zc + z * R];
                let (r, up, f) = v.cam(q[0], q[1], q[2]);
                if f < 0.05 {
                    break;
                }
                let (sx, sy) = v.screen(r, up, f);
                world.push(q);
                proj.push((sx, sy, 1.0 / f));
            }
            if proj.len() < mesh.v.len() {
                continue;
            }
            let dim = fog(v.cam(p.x, p.y, zc).2);
            for &[a, b, c] in &mesh.f {
                let (wa, wb, wc) = (world[a], world[b], world[c]);
                let mut n = norm(cross(sub(wb, wa), sub(wc, wa)));
                let g = [(wa[0] + wb[0] + wc[0]) / 3.0, (wa[1] + wb[1] + wc[1]) / 3.0, (wa[2] + wb[2] + wc[2]) / 3.0];
                if dot(n, sub(g, [p.x, p.y, zc])) < 0.0 {
                    n = [-n[0], -n[1], -n[2]];
                }
                let e = norm(sub(eye, g));
                let lambert = dot(n, e);
                if lambert <= 0.0 {
                    continue;
                }
                let shade = 0.2 + 0.58 * lambert + 0.22 * n[2].max(0.0);
                let col = Rgb::hsv(p.hue, 0.55, shade.min(1.0)).add(Rgb::WHITE.scale(0.55 * lambert.powi(24)));
                tri(buf, zb, v.w, v.h, [proj[a], proj[b], proj[c]], col.scale(dim));
            }
        }
    }

    fn draw_smiley(&self, v: &View, buf: &mut [Rgb], zb: &mut [f32]) {
        const R: f32 = 0.3;
        if !self.smiley {
            return;
        }
        let (ex, ey) = center(self.exit);
        let (r, up, f) = v.cam(ex, ey, EYE);
        if f < 0.1 {
            return;
        }
        let (sx, sy) = v.screen(r, up, f);
        let rp = v.focal * R / f;
        let turn = (self.t as f32 * 0.07).cos();
        let hw = (rp * turn.abs()).max(0.6);
        let light = (0.55 + 0.45 * turn.abs()) * fog(f);
        let (x0, x1, y0, y1) = v.span(sx, sy, hw, rp, rp);
        for py in y0..y1 {
            for px in x0..x1 {
                let i = py * v.w + px;
                let u = (px as f32 + 0.5 - sx) / hw;
                let t = (py as f32 + 0.5 - sy) / rp;
                let rr = u * u + t * t;
                if rr > 1.0 || f >= zb[i] {
                    continue;
                }
                let eye = ((u.abs() - 0.34) / 0.12).powi(2) + ((t + 0.28) / 0.2).powi(2) < 1.0;
                let m = (u * u + (t + 0.05) * (t + 0.05)).sqrt();
                let mouth = t > 0.18 && (0.5..0.64).contains(&m);
                let c = if eye || mouth {
                    Rgb(40, 26, 8)
                } else if rr > 0.85 {
                    Rgb(206, 150, 10)
                } else {
                    YELLOW.lerp(Rgb::WHITE, 0.4 * (1.0 - ((u + 0.4).powi(2) + (t + 0.45).powi(2)) * 4.0).max(0.0))
                };
                buf[i] = c.scale(light);
                zb[i] = f;
            }
        }
    }

    fn draw_rats(&self, v: &View, buf: &mut [Rgb], zb: &mut [f32]) {
        // Size of the sprite's unit square in world units.
        const S: f32 = 0.32;
        for rat in &self.rats {
            let (x, y) = rat.pos();
            let (r, up, f) = v.cam(x, y, 0.0);
            if f < 0.2 {
                continue;
            }
            let (hx, hy) = ((rat.to.0 - rat.from.0) as f32, (rat.to.1 - rat.from.1) as f32);
            let along = hx * v.dx + hy * v.dy;
            let across = hx * v.rx + hy * v.ry;
            let pose = if along.abs() > 0.72 {
                if along > 0.0 { Pose::Back } else { Pose::Front }
            } else {
                Pose::Side(if across >= 0.0 { 1.0 } else { -1.0 })
            };
            let umax = if matches!(pose, Pose::Side(_)) { 1.0 } else { 0.5 };
            let (sx, gy) = v.screen(r, up, f);
            let unit = v.focal * S / f;
            let dim = fog(f);
            let (x0, x1, y0, y1) = v.span(sx, gy, umax * unit, unit, 0.0);
            for py in y0..y1 {
                for px in x0..x1 {
                    let i = py * v.w + px;
                    if f - 0.1 >= zb[i] {
                        continue;
                    }
                    let u = (px as f32 + 0.5 - sx) / unit;
                    let t = (gy - py as f32 - 0.5) / unit;
                    if let Some(c) = rat_pixel(pose, u, t, rat.gait) {
                        buf[i] = c.scale(dim);
                        zb[i] = f - 0.1;
                    }
                }
            }
        }
    }

    fn draw_map(&mut self) {
        let t = self.grid.t as usize;
        let (w, ph) = (self.w, self.ph);
        if t + 2 > w || t + 2 > ph {
            return;
        }
        let s = if 2 * t + 2 <= w / 2 && 2 * t + 2 <= ph / 2 { 2 } else { 1 };
        let size = t * s + 2;
        for y in 0..size {
            for x in 0..size {
                self.px[y * w + x] = Rgb(12, 10, 10);
            }
        }
        let put = |px: &mut Vec<Rgb>, x: f32, y: f32, c: Rgb| {
            let (mx, my) = ((x * s as f32) as usize + 1, (y * s as f32) as usize + 1);
            if mx < size - 1 && my < size - 1 {
                px[my * w + mx] = c;
            }
        };
        for ty in 0..t {
            for tx in 0..t {
                let i = ty * t + tx;
                let c = if !self.seen[i] {
                    Rgb(24, 20, 20)
                } else if self.grid.wall[i] {
                    Rgb(146, 64, 44)
                } else if self.trail[i] {
                    Rgb(176, 136, 84)
                } else {
                    Rgb(96, 66, 40)
                };
                for dy in 0..s {
                    for dx in 0..s {
                        self.px[(1 + ty * s + dy) * w + 1 + tx * s + dx] = c;
                    }
                }
            }
        }
        let seen = |x: f32, y: f32| self.seen[y as usize * t + x as usize];
        for p in self.polys.iter().filter(|p| p.alive && seen(p.x, p.y)) {
            put(&mut self.px, p.x, p.y, Rgb::hsv(p.hue, 0.6, 1.0));
        }
        let (ex, ey) = center(self.exit);
        if self.smiley && seen(ex, ey) {
            put(&mut self.px, ex, ey, YELLOW);
        }
        for r in &self.rats {
            let (x, y) = r.pos();
            if seen(x, y) {
                put(&mut self.px, x, y, Rgb(200, 200, 200));
            }
        }
        let (dy, dx) = self.a.sin_cos();
        let reach = 1.2 / s as f32;
        put(&mut self.px, self.x + dx * reach, self.y + dy * reach, Rgb(255, 120, 60));
        put(&mut self.px, self.x, self.y, Rgb::WHITE);
    }
}

#[derive(Clone, Copy)]
enum Pose {
    /// Seen from the side, nose towards `+u` (1) or `-u` (-1).
    Side(f32),
    Front,
    Back,
}

fn inside(u: f32, t: f32, cu: f32, ct: f32, ru: f32, rt: f32) -> bool {
    ((u - cu) / ru).powi(2) + ((t - ct) / rt).powi(2) < 1.0
}

/// A rat's colour at `(u, t)` in its sprite: `t` from 0 at the ground up,
/// `u` across in the same units, 0 at its centre.
fn rat_pixel(pose: Pose, u: f32, t: f32, gait: f32) -> Option<Rgb> {
    let fur = |t: f32| FUR.scale(0.7 + 0.6 * t);
    let step = gait.sin();
    match pose {
        Pose::Side(dir) => {
            let u = u * dir;
            if inside(u, t, 0.66, 0.4, 0.045, 0.045) {
                return Some(Rgb(16, 10, 10));
            }
            if inside(u, t, 0.93, 0.31, 0.05, 0.05) {
                return Some(PINK);
            }
            if inside(u, t, 0.45, 0.58, 0.06, 0.07) {
                return Some(PINK.scale(0.85));
            }
            if inside(u, t, 0.45, 0.58, 0.11, 0.12) {
                return Some(FUR.scale(0.8));
            }
            if inside(u, t, 0.55, 0.34, 0.3, 0.2) || inside(u, t, 0.8, 0.3, 0.14, 0.1) {
                return Some(fur(t + 0.1));
            }
            if inside(u, t, -0.05, 0.36, 0.55, 0.3) {
                return Some(fur(t));
            }
            let feet = [0.3 + 0.08 * step, -0.35 - 0.08 * step];
            if t < 0.1 && feet.iter().any(|&fu| (u - fu).abs() < 0.06) {
                return Some(PINK.scale(0.7));
            }
            if (-1.0..-0.5).contains(&u) {
                let tail = 0.3 - (-0.5 - u) * 0.5 + 0.05 * (u * 8.0 + gait * 0.5).sin();
                if (t - tail).abs() < 0.035 {
                    return Some(PINK.scale(0.8));
                }
            }
            None
        }
        Pose::Front => {
            if inside(u.abs(), t, 0.09, 0.36, 0.04, 0.04) {
                return Some(Rgb(16, 10, 10));
            }
            if inside(u, t, 0.0, 0.22, 0.045, 0.04) {
                return Some(PINK);
            }
            if inside(u.abs(), t, 0.2, 0.52, 0.055, 0.06) {
                return Some(PINK.scale(0.85));
            }
            if inside(u.abs(), t, 0.2, 0.52, 0.1, 0.1) {
                return Some(FUR.scale(0.8));
            }
            if inside(u, t, 0.0, 0.3, 0.22, 0.2) {
                return Some(fur(t + 0.15));
            }
            if inside(u, t, 0.0, 0.3, 0.3, 0.26) {
                return Some(fur(t - 0.1));
            }
            let lift = if u < 0.0 { step } else { -step }.max(0.0) * 0.04;
            if t >= lift && t < 0.07 + lift && ((u.abs() - 0.15).abs() < 0.05) {
                return Some(PINK.scale(0.7));
            }
            None
        }
        Pose::Back => {
            if (u - 0.03 * (gait * 0.5).sin()).abs() < 0.035 && t < 0.2 {
                return Some(PINK.scale(0.8));
            }
            if inside(u.abs(), t, 0.2, 0.55, 0.09, 0.09) {
                return Some(FUR.scale(0.75));
            }
            if inside(u, t, 0.0, 0.32, 0.3, 0.28) {
                return Some(fur(t));
            }
            None
        }
    }
}

fn edge(a: (f32, f32, f32), b: (f32, f32, f32), c: (f32, f32)) -> f32 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// Fill a screen triangle whose vertices carry one over their depth, keeping
/// the pixels nearer than what the depth buffer holds.
fn tri(buf: &mut [Rgb], zb: &mut [f32], w: usize, h: usize, p: [(f32, f32, f32); 3], col: Rgb) {
    let area = edge(p[0], p[1], (p[2].0, p[2].1));
    if area.abs() < 1e-6 {
        return;
    }
    let lo = |f: fn(&(f32, f32, f32)) -> f32| p.iter().map(f).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
    let hi = |f: fn(&(f32, f32, f32)) -> f32| p.iter().map(f).fold(f32::MIN, f32::max).ceil().max(0.0) as usize;
    let (x0, x1) = (lo(|q| q.0), hi(|q| q.0).min(w));
    let (y0, y1) = (lo(|q| q.1), hi(|q| q.1).min(h));
    for y in y0..y1 {
        for x in x0..x1 {
            let q = (x as f32 + 0.5, y as f32 + 0.5);
            let b0 = edge(p[1], p[2], q) / area;
            let b1 = edge(p[2], p[0], q) / area;
            let b2 = 1.0 - b0 - b1;
            if b0 < 0.0 || b1 < 0.0 || b2 < 0.0 {
                continue;
            }
            let z = 1.0 / (b0 * p[0].2 + b1 * p[1].2 + b2 * p[2].2);
            let i = y * w + x;
            if z < zb[i] {
                zb[i] = z;
                buf[i] = col;
            }
        }
    }
}

impl Animation for Maze3d {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        self.t += 1;
        self.timer += 1;
        match self.phase {
            Phase::In => {
                if self.timer >= FADE + HOLD {
                    self.phase = Phase::Walk;
                    self.set_off(rng);
                }
            }
            Phase::Walk => {
                self.walk(rng);
                self.touch();
            }
            Phase::Out => {
                if self.timer >= FADE {
                    self.reset(rng);
                }
            }
        }
        self.move_rats(rng);
        if self.roll_t < 1.0 {
            self.roll_t = (self.roll_t + 1.0 / ROLL).min(1.0);
            self.roll = self.roll_from + (self.roll_to - self.roll_from) * smooth(self.roll_t);
        }
        let (tx, ty) = (self.x.floor() as i32, self.y.floor() as i32);
        let t = self.grid.t;
        self.trail[(ty * t + tx) as usize] = true;
        for y in (ty - 1).max(0)..=(ty + 1).min(t - 1) {
            for x in (tx - 1).max(0)..=(tx + 1).min(t - 1) {
                self.seen[(y * t + x) as usize] = true;
            }
        }

        let (w, ph) = (self.w, self.ph);
        let r = self.roll.rem_euclid(TAU);
        let mut px = std::mem::take(&mut self.px);
        if r < 1e-3 || r > TAU - 1e-3 || (r - PI).abs() < 1e-3 {
            let mut zb = std::mem::take(&mut self.zb);
            self.render(&mut px, &mut zb, w, ph);
            if (r - PI).abs() < 1e-3 {
                px.reverse();
            }
            self.zb = zb;
        } else {
            // Render a square that still covers the screen once rotated.
            let d = ((w * w + ph * ph) as f32).sqrt().ceil() as usize + 2;
            let (mut big, mut bigz) = (std::mem::take(&mut self.big), std::mem::take(&mut self.bigz));
            big.resize(d * d, Rgb::BLACK);
            bigz.resize(d * d, 0.0);
            self.render(&mut big, &mut bigz, d, d);
            let (s, co) = r.sin_cos();
            let half = d as f32 / 2.0;
            for y in 0..ph {
                for x in 0..w {
                    let ox = x as f32 + 0.5 - w as f32 / 2.0;
                    let oy = y as f32 + 0.5 - ph as f32 / 2.0;
                    let bx = ((half + ox * co - oy * s) as usize).min(d - 1);
                    let by = ((half + ox * s + oy * co) as usize).min(d - 1);
                    px[y * w + x] = big[by * d + bx];
                }
            }
            self.big = big;
            self.bigz = bigz;
        }
        self.px = px;
        if self.map {
            self.draw_map();
        }
        let fade = match self.phase {
            Phase::In => self.timer as f32 / FADE as f32,
            Phase::Walk => 1.0,
            Phase::Out => 1.0 - self.timer as f32 / FADE as f32,
        };
        if fade < 1.0 {
            for p in &mut self.px {
                *p = p.scale(fade.max(0.0));
            }
        }
        c.blit_pixels(&self.px);
    }

    /// Space shows or hides the map.
    fn key(&mut self, key: crate::Key) {
        if key == crate::Key::Space {
            self.map = !self.map;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mazes_are_perfect() {
        let mut rng = Rng::new(3);
        for n in [1, 2, 8, 14] {
            let g = Grid::carve(n, &mut rng);
            let open = g.wall.iter().filter(|&&w| !w).count() as i32;
            // n * n cells and n * n - 1 passages: a spanning tree.
            assert_eq!(open, 2 * n * n - 1);
            assert!(g.distances((0, 0)).iter().all(|&d| d != u32::MAX));
        }
    }

    #[test]
    fn walks_to_the_exit_through_open_tiles() {
        for seed in 0..6 {
            let mut rng = Rng::new(seed);
            let mut m = Maze3d::new(8, 4, &mut rng);
            let mut c = Canvas::new(8, 4);
            let mut frames = 0;
            while m.phase != Phase::Out {
                m.step(&mut c, &mut rng);
                assert!(!m.grid.solid(m.x.floor() as i32, m.y.floor() as i32), "seed {seed} walked into a wall");
                frames += 1;
                assert!(frames < 40_000, "seed {seed} never found the exit");
            }
            let touched = m.polys.iter().filter(|p| !p.alive).count() as f32;
            assert_eq!(m.roll_to, touched * PI, "seed {seed}: one half turn per polyhedron");
        }
    }

    #[test]
    fn space_toggles_the_map() {
        let run = |space: bool| {
            let mut rng = Rng::new(9);
            let mut m = Maze3d::new(80, 24, &mut rng);
            let mut c = Canvas::new(80, 24);
            if space {
                m.key(crate::Key::Space);
            }
            for _ in 0..30 {
                m.step(&mut c, &mut rng);
            }
            c
        };
        let (plain, map) = (run(false), run(true));
        assert_ne!(plain.get(1, 1), map.get(1, 1));
        assert_eq!(plain.get(79, 23), map.get(79, 23));
    }
}
