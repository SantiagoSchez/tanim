//! A maze carves itself (backtracker, Prim's or Kruskal's), gets solved by an
//! animated BFS or A* flood, shows the path, fades out and starts over.

use super::Animation;
use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

const WALL: Rgb = Rgb(10, 11, 22);
const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Gen,
    Solve,
    Trace,
    Pause,
    Fade,
}

enum Gen {
    Backtrack(Vec<usize>),
    Prim(Vec<(usize, usize)>),
    Kruskal(Vec<(usize, usize)>, Vec<usize>),
}

struct Maze {
    pw: usize,
    sc: usize,
    ox: usize,
    oy: usize,
    mw: usize,
    mh: usize,
    gw: usize,
    gh: usize,
    /// Carving order per grid cell, `NONE` for walls.
    order: Vec<u32>,
    carved: u32,
    visited: Vec<bool>,
    head: Option<usize>,
    gen: Gen,
    algo: usize,
    phase: Phase,
    timer: u32,
    hue0: f32,
    dist: Vec<u32>,
    prev: Vec<u32>,
    queue: VecDeque<usize>,
    heap: BinaryHeap<Reverse<(u32, usize)>>,
    astar: bool,
    frontier_d: u32,
    path: Vec<usize>,
    traced: usize,
    px: Vec<Rgb>,
}

impl Maze {
    fn new(w: usize, h: usize, rng: &mut Rng) -> Maze {
        let pw = w;
        let ph = h * 2;
        let sc = (pw.min(ph * 2) / 100).clamp(1, 3);
        let mut gw = pw / sc;
        let mut gh = ph / sc;
        if gw % 2 == 0 {
            gw = gw.saturating_sub(1);
        }
        if gh % 2 == 0 {
            gh = gh.saturating_sub(1);
        }
        let mw = gw / 2;
        let mh = gh / 2;
        let mut m = Maze {
            pw,
            sc,
            ox: (pw - gw * sc) / 2,
            oy: (ph - gh * sc) / 2,
            mw,
            mh,
            gw,
            gh,
            order: Vec::new(),
            carved: 0,
            visited: Vec::new(),
            head: None,
            gen: Gen::Backtrack(Vec::new()),
            algo: rng.below(3),
            phase: Phase::Gen,
            timer: 0,
            hue0: 0.0,
            dist: Vec::new(),
            prev: Vec::new(),
            queue: VecDeque::new(),
            heap: BinaryHeap::new(),
            astar: false,
            frontier_d: 0,
            path: Vec::new(),
            traced: 0,
            px: vec![WALL; pw * ph],
        };
        m.reset(rng);
        m
    }

    fn cells(&self) -> usize {
        self.mw * self.mh
    }

    /// Grid index of maze cell `m`.
    fn grid(&self, m: usize) -> usize {
        let (x, y) = (m % self.mw, m / self.mw);
        (2 * y + 1) * self.gw + 2 * x + 1
    }

    /// Neighbors of maze cell `m` as (cell, wall grid index).
    fn neighbors(&self, m: usize) -> ([(usize, usize); 4], usize) {
        let (x, y) = (m % self.mw, m / self.mw);
        let g = self.grid(m);
        let mut out = [(0, 0); 4];
        let mut n = 0;
        if x > 0 {
            out[n] = (m - 1, g - 1);
            n += 1;
        }
        if x + 1 < self.mw {
            out[n] = (m + 1, g + 1);
            n += 1;
        }
        if y > 0 {
            out[n] = (m - self.mw, g - self.gw);
            n += 1;
        }
        if y + 1 < self.mh {
            out[n] = (m + self.mw, g + self.gw);
            n += 1;
        }
        (out, n)
    }

    fn reset(&mut self, rng: &mut Rng) {
        let n = self.cells();
        self.order = vec![NONE; self.gw * self.gh];
        self.visited = vec![false; n];
        self.carved = 0;
        self.head = None;
        self.phase = Phase::Gen;
        self.timer = 0;
        self.hue0 = rng.f32();
        self.algo = (self.algo + 1) % 3;
        self.astar = rng.chance(0.5);
        if n == 0 {
            return;
        }
        let start = rng.below(n);
        self.gen = match self.algo {
            0 => {
                self.visit(start);
                Gen::Backtrack(vec![start])
            }
            1 => {
                self.visit(start);
                let (nb, k) = self.neighbors(start);
                Gen::Prim(nb[..k].iter().map(|&(c, _)| (c, start)).collect())
            }
            _ => {
                let mut edges = Vec::with_capacity(2 * n);
                for m in 0..n {
                    if m % self.mw + 1 < self.mw {
                        edges.push((m, m + 1));
                    }
                    if m / self.mw + 1 < self.mh {
                        edges.push((m, m + self.mw));
                    }
                }
                for i in (1..edges.len()).rev() {
                    edges.swap(i, rng.below(i + 1));
                }
                Gen::Kruskal(edges, (0..n).collect())
            }
        };
    }

    fn carve(&mut self, g: usize) {
        if self.order[g] == NONE {
            self.order[g] = self.carved;
            self.carved += 1;
        }
        self.head = Some(g);
    }

    fn visit(&mut self, m: usize) {
        self.visited[m] = true;
        let g = self.grid(m);
        self.carve(g);
    }

    fn wall_between(&self, a: usize, b: usize) -> usize {
        (self.grid(a) + self.grid(b)) / 2
    }

    /// One generation step; returns false when the maze is complete.
    fn gen_step(&mut self, rng: &mut Rng) -> bool {
        let mut gen = std::mem::replace(&mut self.gen, Gen::Backtrack(Vec::new()));
        let alive = match &mut gen {
            Gen::Backtrack(stack) => match stack.last().copied() {
                None => false,
                Some(cur) => {
                    let (nb, k) = self.neighbors(cur);
                    let mut free = [(0, 0); 4];
                    let mut f = 0;
                    for &(c, wall) in &nb[..k] {
                        if !self.visited[c] {
                            free[f] = (c, wall);
                            f += 1;
                        }
                    }
                    if f == 0 {
                        stack.pop();
                        self.head = stack.last().map(|&m| self.grid(m));
                    } else {
                        let (c, wall) = free[rng.below(f)];
                        self.carve(wall);
                        self.visit(c);
                        stack.push(c);
                    }
                    true
                }
            },
            Gen::Prim(front) => loop {
                if front.is_empty() {
                    break false;
                }
                let (c, from) = front.swap_remove(rng.below(front.len()));
                if self.visited[c] {
                    continue;
                }
                let wall = self.wall_between(c, from);
                self.carve(wall);
                self.visit(c);
                let (nb, k) = self.neighbors(c);
                for &(d, _) in &nb[..k] {
                    if !self.visited[d] {
                        front.push((d, c));
                    }
                }
                break true;
            },
            Gen::Kruskal(edges, parent) => loop {
                let Some((a, b)) = edges.pop() else { break false };
                let (ra, rb) = (find(parent, a), find(parent, b));
                if ra == rb {
                    continue;
                }
                parent[ra] = rb;
                if !self.visited[a] {
                    self.visit(a);
                }
                let wall = self.wall_between(a, b);
                self.carve(wall);
                if !self.visited[b] {
                    self.visit(b);
                }
                break true;
            },
        };
        self.gen = gen;
        alive
    }

    fn start_solve(&mut self) {
        let n = self.gw * self.gh;
        self.dist = vec![NONE; n];
        self.prev = vec![NONE; n];
        self.queue.clear();
        self.heap.clear();
        let s = self.grid(0);
        self.dist[s] = 0;
        self.frontier_d = 0;
        if self.astar {
            self.heap.push(Reverse((self.h(s), s)));
        } else {
            self.queue.push_back(s);
        }
        self.phase = Phase::Solve;
        self.head = None;
    }

    fn goal(&self) -> usize {
        self.grid(self.cells() - 1)
    }

    fn h(&self, g: usize) -> u32 {
        let t = self.goal();
        let dx = (g % self.gw).abs_diff(t % self.gw);
        let dy = (g / self.gw).abs_diff(t / self.gw);
        (dx + dy) as u32
    }

    /// One solver expansion; returns true once the goal is reached.
    fn solve_step(&mut self) -> bool {
        let cur = if self.astar {
            self.heap.pop().map(|Reverse((_, g))| g)
        } else {
            self.queue.pop_front()
        };
        let Some(cur) = cur else { return true };
        let goal = self.goal();
        if cur == goal {
            return true;
        }
        let d = self.dist[cur];
        self.frontier_d = self.frontier_d.max(d);
        for nb in [cur - 1, cur + 1, cur - self.gw, cur + self.gw] {
            if self.order[nb] != NONE && self.dist[nb] == NONE {
                self.dist[nb] = d + 1;
                self.prev[nb] = cur as u32;
                if self.astar {
                    self.heap.push(Reverse((d + 1 + self.h(nb), nb)));
                } else {
                    self.queue.push_back(nb);
                }
            }
        }
        false
    }

    fn build_path(&mut self) {
        self.path.clear();
        let mut g = self.goal();
        while g as u32 != NONE {
            self.path.push(g);
            g = self.prev[g] as usize;
        }
        self.path.reverse();
        self.traced = 0;
        self.phase = Phase::Trace;
    }

    fn color(&self, g: usize, fade: f32) -> Rgb {
        let o = self.order[g];
        if o == NONE {
            return WALL;
        }
        let total = (2 * self.cells()) as f32;
        let base = Rgb::hsv(self.hue0 + 0.75 * o as f32 / total, 0.6, 0.85);
        let c = match self.phase {
            Phase::Gen => {
                if Some(g) == self.head {
                    Rgb::WHITE
                } else {
                    base
                }
            }
            _ => {
                let d = self.dist[g];
                if d == NONE {
                    WALL.lerp(base, 0.3)
                } else if self.phase == Phase::Solve && d + 3 >= self.frontier_d {
                    Rgb(240, 250, 255)
                } else {
                    base.lerp(Rgb(230, 240, 255), 0.45)
                }
            }
        };
        WALL.lerp(c, fade)
    }

    fn draw(&mut self, c: &mut Canvas) {
        let fade = if self.phase == Phase::Fade { 1.0 - self.timer as f32 / 60.0 } else { 1.0 };
        let gold = WALL.lerp(Rgb(255, 205, 70), fade);
        let mut on_path = vec![false; 0];
        if matches!(self.phase, Phase::Trace | Phase::Pause | Phase::Fade) {
            on_path = vec![false; self.gw * self.gh];
            for &g in &self.path[..self.traced.min(self.path.len())] {
                on_path[g] = true;
            }
        }
        for gy in 0..self.gh {
            for gx in 0..self.gw {
                let g = gy * self.gw + gx;
                let mut col = if on_path.get(g) == Some(&true) { gold } else { self.color(g, fade) };
                if self.phase != Phase::Gen && (g == self.grid(0) || g == self.goal()) {
                    col = WALL.lerp(if g == self.grid(0) { Rgb(80, 255, 120) } else { Rgb(255, 80, 90) }, fade);
                }
                for dy in 0..self.sc {
                    let row = (self.oy + gy * self.sc + dy) * self.pw + self.ox + gx * self.sc;
                    self.px[row..row + self.sc].fill(col);
                }
            }
        }
        if self.phase == Phase::Trace && self.traced < self.path.len() {
            let g = self.path[self.traced];
            let (gx, gy) = (g % self.gw, g / self.gw);
            for dy in 0..self.sc {
                let row = (self.oy + gy * self.sc + dy) * self.pw + self.ox + gx * self.sc;
                self.px[row..row + self.sc].fill(Rgb::WHITE);
            }
        }
        c.blit_pixels(&self.px);
    }
}

fn find(p: &mut [usize], mut a: usize) -> usize {
    while p[a] != a {
        p[a] = p[p[a]];
        a = p[a];
    }
    a
}

impl Animation for Maze {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        let n = self.cells();
        if n == 0 {
            c.clear(WALL);
            return;
        }
        // Aim for roughly 8 s of generation and 6 s of solving at 60 fps.
        match self.phase {
            Phase::Gen => {
                let events = if self.algo == 0 { 2 * n } else { n };
                for _ in 0..events.div_ceil(480) {
                    if !self.gen_step(rng) {
                        self.start_solve();
                        break;
                    }
                }
            }
            Phase::Solve => {
                for _ in 0..(2 * n).div_ceil(360) {
                    if self.solve_step() {
                        self.build_path();
                        break;
                    }
                }
            }
            Phase::Trace => {
                self.traced += self.path.len().div_ceil(150);
                if self.traced >= self.path.len() {
                    self.traced = self.path.len();
                    self.phase = Phase::Pause;
                    self.timer = 0;
                }
            }
            Phase::Pause => {
                self.timer += 1;
                if self.timer > 150 {
                    self.phase = Phase::Fade;
                    self.timer = 0;
                }
            }
            Phase::Fade => {
                self.timer += 1;
                if self.timer > 60 {
                    self.reset(rng);
                }
            }
        }
        self.draw(c);
    }
}

pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
    Box::new(Maze::new(w, h, rng))
}
