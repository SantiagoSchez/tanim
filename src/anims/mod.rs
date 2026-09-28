//! The animation catalog. Each module exposes `pub fn new(w, h, rng) -> Box<dyn Animation>`.

use crate::canvas::{Canvas, Rgb};
use crate::rng::Rng;
use crate::Key;

pub trait Animation {
    /// Advance one tick and draw into `c`. The canvas keeps its contents between
    /// ticks and always has the size the animation was created with.
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng);

    /// React to an arrow key. Most animations ignore input.
    fn key(&mut self, _key: Key) {}

    /// True if the animation zooms with its own camera; the others get the
    /// generic digital zoom from `crate::zoom`.
    fn has_camera(&self) -> bool {
        false
    }
}

pub struct Entry {
    pub name: &'static str,
    pub desc: &'static str,
    pub fps: u32,
    /// Background the animation leaves empty space in; shown as the
    /// terminal's own background unless `--opaque` is given.
    pub clear: Option<Rgb>,
    pub make: fn(usize, usize, &mut Rng) -> Box<dyn Animation>,
}

macro_rules! catalog {
    ($($m:ident, $fps:expr, $desc:expr $(, clear = $bg:expr)?;)*) => {
        $(mod $m;)*
        pub static CATALOG: &[Entry] = &[
            $(Entry {
                name: stringify!($m),
                desc: $desc,
                fps: $fps,
                clear: catalog!(@opt $($bg)?),
                make: $m::new,
            },)*
        ];
    };
    (@opt) => { None };
    (@opt $bg:expr) => { Some($bg) };
}

catalog! {
    aquarium,  20, "Fish, bubbles and swaying seaweed in a fish tank";
    garden,    20, "Plants sprouting, branching and blooming";
    dog,       20, "A dog running, napping and chasing butterflies in a meadow";
    dungeon,   20, "A roguelike dungeon generating itself, then explored by a hero";
    rain,      30, "Thunderstorm with splashes and lightning";
    snow,      20, "Snowfall piling up under shifting wind";
    ripples,   30, "Raindrops rippling across a pond";
    starfield, 30, "Warp-speed flight through the stars", clear = starfield::BG;
    tunnel,    30, "Flying down an endless tiled tunnel";
    fireworks, 30, "Rockets bursting into colorful sparks";
    galaxy,    30, "A spiral galaxy slowly turning", clear = galaxy::BG;
    aurora,    20, "Northern lights over mountains and a lake";
    boids,     30, "A flock of birds moving as one";
    matrix,    20, "Cascading digital rain", clear = matrix::BG;
    fire,      30, "A roaring bonfire";
    plasma,    30, "Psychedelic flowing plasma";
    lava,      30, "Lava lamp blobs melting and merging";
    metaballs, 30, "Glossy blobs merging like drops of gel";
    life,      15, "Conway's Game of Life with aging cells", clear = Rgb::BLACK;
    physarum,  30, "Slime mould growing a living network", clear = Rgb::BLACK;
    neurons,   30, "Neurons firing and signalling across synapses", clear = neurons::BG;
    sand,      30, "Falling colored sand piling into dunes";
    maze,      60, "A maze carving itself, then being solved";
    snake,     20, "Self-playing snake chasing apples";
    tetris,    30, "Self-playing Tetris";
    pipes,     40, "The classic 3D pipes screensaver", clear = pipes::BG;
    toasters,  20, "Flying toasters, toast and breakfast guests, after the classic After Dark screensaver", clear = toasters::BG;
    city,      15, "Night city skyline scrolling by";
}
