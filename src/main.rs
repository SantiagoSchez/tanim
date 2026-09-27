mod export;
mod term;

use tanim::anims::{Animation, Entry, CATALOG};
use tanim::canvas::{Canvas, Screen, HALF};
use tanim::rng::Rng;
use std::io::Write;
use std::process::ExitCode;
use std::time::{Duration, Instant};

const USAGE: &str = "\
tanim - endless procedural terminal animations

USAGE:
    tanim               play a random animation
    tanim <name>        play the named animation (unique prefixes work)
    tanim --list        show the catalog
    tanim --help        show this help
    tanim --export-html [dir] [names...]
                        write web pages that play the animations in a browser
                        (dir/index.html with all of them, plus one per name;
                        default dir: html, default names: all)

OPTIONS:
    --seed <n>          repeat the same run every time
    --opaque            paint backgrounds instead of showing the terminal's own
    --zoom <n>          start zoomed in n steps (negative: out), where supported

While playing, press Enter to switch to another random animation
and Q, Esc or Ctrl+C to quit. Up/Down zoom in and out; while zoomed,
W, A, S and D move the view. In dungeon, Space builds a new level.
";

/// Options that may appear anywhere on the command line.
struct Opts {
    seed: Option<u64>,
    opaque: bool,
    zoom: i32,
}

impl Opts {
    fn rng(&self) -> Rng {
        self.seed.map_or_else(Rng::from_entropy, Rng::new)
    }

    /// Build an animation, applying the starting zoom as arrow presses.
    fn make(&self, e: &Entry, w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation> {
        let mut a = tanim::zoom::wrap((e.make)(w, h, rng), w, h);
        let key = if self.zoom > 0 { term::Key::Up } else { term::Key::Down };
        for _ in 0..self.zoom.unsigned_abs() {
            a.key(key);
        }
        a
    }
}

/// Remove `flag <value>` from `args` and parse the value, if present.
fn take_value<T: std::str::FromStr>(args: &mut Vec<String>, flag: &str) -> Result<Option<T>, String> {
    let Some(i) = args.iter().position(|a| a == flag) else { return Ok(None) };
    let v = args.get(i + 1).and_then(|s| s.parse().ok()).ok_or(format!("{flag} needs a number"))?;
    args.drain(i..i + 2);
    Ok(Some(v))
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut opts = Opts { seed: None, opaque: false, zoom: 0 };
    if let Some(i) = args.iter().position(|a| a == "--opaque") {
        args.remove(i);
        opts.opaque = true;
    }
    match (take_value(&mut args, "--seed"), take_value(&mut args, "--zoom")) {
        (Ok(seed), Ok(zoom)) => {
            opts.seed = seed;
            opts.zoom = zoom.unwrap_or(0);
        }
        (Err(msg), _) | (_, Err(msg)) => {
            eprintln!("tanim: {msg}");
            return ExitCode::from(2);
        }
    }
    let mut rng = opts.rng();
    let entry = match args.first().map(String::as_str) {
        None => &CATALOG[rng.below(CATALOG.len())],
        Some("-h" | "--help") => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Some("-l" | "--list") => {
            list();
            return ExitCode::SUCCESS;
        }
        Some("-V" | "--version") => {
            println!("tanim {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Some("--check") => return check(args.get(1).map(String::as_str), &opts),
        Some("--snapshot") => return snapshot(&args[1..], &opts),
        Some("--dump") => return dump(&args[1..], &opts),
        Some("--export-html") => return export_html(&args[1..]),
        Some(name) => match find(name) {
            Ok(e) => e,
            Err(msg) => {
                eprintln!("tanim: {msg}\nRun `tanim --list` to see the catalog.");
                return ExitCode::from(2);
            }
        },
    };
    if !term::is_tty() {
        eprintln!("tanim: stdout is not a terminal");
        return ExitCode::FAILURE;
    }
    run(entry, &mut rng, &opts);
    ExitCode::SUCCESS
}

fn list() {
    let width = CATALOG.iter().map(|e| e.name.len()).max().unwrap_or(0);
    for e in CATALOG {
        println!("  {:width$}  {}", e.name, e.desc);
    }
}

fn find(name: &str) -> Result<&'static Entry, String> {
    let name = name.to_ascii_lowercase();
    if let Some(e) = CATALOG.iter().find(|e| e.name == name) {
        return Ok(e);
    }
    let hits: Vec<&Entry> = CATALOG.iter().filter(|e| e.name.starts_with(&name)).collect();
    match hits.as_slice() {
        [e] => Ok(e),
        [] => Err(format!("unknown animation '{name}'")),
        many => Err(format!(
            "'{name}' is ambiguous: {}",
            many.iter().map(|e| e.name).collect::<Vec<_>>().join(", ")
        )),
    }
}

/// Most simulation steps run without drawing to catch up after a stall.
const MAX_CATCH_UP: u32 = 5;

fn run(mut entry: &'static Entry, rng: &mut Rng, opts: &Opts) {
    let clear_of = |e: &Entry| if opts.opaque { None } else { e.clear };
    let _guard = term::Term::enter();
    let mut frame = Duration::from_secs_f64(1.0 / entry.fps as f64);
    let (mut w, mut h) = term::size();
    let mut canvas = Canvas::new(w, h);
    let mut anim = opts.make(entry, w, h, rng);
    let mut screen = Screen::new();
    screen.set_clear(clear_of(entry));
    let mut out = std::io::stdout().lock();
    let mut next = Instant::now();
    'main: while term::running() {
        if term::take_resized() {
            (w, h) = term::size();
            canvas = Canvas::new(w, h);
            anim = opts.make(entry, w, h, rng);
            screen.invalidate();
        }
        anim.step(&mut canvas, rng);
        if out.write_all(screen.render(&canvas)).and_then(|_| out.flush()).is_err() {
            break;
        }
        next += frame;
        // If the terminal could not keep up, advance the simulation without
        // drawing so it drops frames instead of going into slow motion.
        let mut skipped = 0;
        while Instant::now() >= next + frame && skipped < MAX_CATCH_UP {
            anim.step(&mut canvas, rng);
            next += frame;
            skipped += 1;
        }
        // Sleep until the next frame, handling keys as they arrive.
        loop {
            let now = Instant::now();
            if next <= now {
                next = now;
            }
            match term::wait_key(next - now) {
                term::Key::None => {
                    if Instant::now() >= next {
                        break;
                    }
                }
                term::Key::Esc => break 'main,
                term::Key::Enter => {
                    // Switch to a different random animation.
                    let cur = CATALOG.iter().position(|e| std::ptr::eq(e, entry)).unwrap_or(0);
                    entry = &CATALOG[(cur + 1 + rng.below(CATALOG.len() - 1)) % CATALOG.len()];
                    frame = Duration::from_secs_f64(1.0 / entry.fps as f64);
                    canvas = Canvas::new(w, h);
                    anim = opts.make(entry, w, h, rng);
                    screen.set_clear(clear_of(entry));
                    next = Instant::now();
                    break;
                }
                key => anim.key(key),
            }
        }
    }
}

fn export_html(args: &[String]) -> ExitCode {
    let dir = args.first().map_or("html", String::as_str);
    let pages: Result<Vec<&Entry>, String> = match &args[args.len().min(1)..] {
        [] => Ok(CATALOG.iter().collect()),
        names => names.iter().map(|n| find(n)).collect(),
    };
    match pages.and_then(|pages| export::write(std::path::Path::new(dir), &pages)) {
        Ok(paths) => {
            for p in paths {
                println!("{p}");
            }
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("tanim: {msg}");
            ExitCode::FAILURE
        }
    }
}

/// Dev aid: run animations headless at several sizes to catch panics and
/// measure the cost of a frame (simulation + diff rendering).
fn check(filter: Option<&str>, opts: &Opts) -> ExitCode {
    let sizes = [(1, 1), (2, 2), (7, 3), (13, 5), (80, 24), (240, 70)];
    let mut rng = opts.rng();
    for e in CATALOG {
        if let Some(f) = filter {
            if !f.split(',').any(|n| n == e.name) {
                continue;
            }
        }
        eprint!("{:10}", e.name);
        let mut report = String::new();
        for &(w, h) in &sizes {
            let frames = if w >= 80 { 3000 } else { 1500 };
            let mut c = Canvas::new(w, h);
            let mut a = (e.make)(w, h, &mut rng);
            let mut s = Screen::new();
            let mut bytes = 0usize;
            let t = Instant::now();
            for _ in 0..frames {
                a.step(&mut c, &mut rng);
                bytes += s.render(&c).len();
            }
            if w >= 80 {
                let us = t.elapsed().as_secs_f64() * 1e6 / frames as f64;
                report += &format!("  {w}x{h}: {us:6.1}us {:5}B/f", bytes / frames);
            }
        }
        eprintln!("{report}");
    }
    ExitCode::SUCCESS
}

/// Dev aid: `--snapshot <name> [WxH] [frames]` prints the canvas as text.
/// Block-ish cells are shown as a brightness ramp.
fn snapshot(args: &[String], opts: &Opts) -> ExitCode {
    let Some(entry) = args.first().and_then(|n| find(n).ok()) else {
        eprintln!("usage: tanim --snapshot <name> [WxH] [frames]");
        return ExitCode::from(2);
    };
    let (w, h) = args
        .get(1)
        .and_then(|s| s.split_once('x'))
        .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))
        .unwrap_or((80, 24));
    let frames: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100);
    let mut rng = opts.rng();
    let mut c = Canvas::new(w, h);
    let mut a = opts.make(entry, w, h, &mut rng);
    for _ in 0..frames {
        a.step(&mut c, &mut rng);
    }
    const RAMP: &[u8] = b" .:-=+*#%@";
    let mut s = String::new();
    for y in 0..h {
        for x in 0..w {
            let cell = c.cells[y * w + x];
            let lum = match cell.ch {
                HALF => Some((cell.fg.luma() as u32 + cell.bg.luma() as u32) / 2),
                ' ' => Some(cell.bg.luma() as u32),
                '█' => Some(cell.fg.luma() as u32),
                _ => None,
            };
            match lum {
                Some(l) => s.push(RAMP[(l as usize * RAMP.len() / 256).min(RAMP.len() - 1)] as char),
                None => s.push(cell.ch),
            }
        }
        s.push('\n');
    }
    print!("{s}");
    ExitCode::SUCCESS
}

/// Dev aid: `--dump <name> [WxH] [frames] [warmup]` writes raw frames to
/// stdout for offline rendering (see `scripts/gif.py`). Each frame is `w * h`
/// cells of 10 bytes: the char as u32 LE, then fg and bg as RGB.
fn dump(args: &[String], opts: &Opts) -> ExitCode {
    let Some(entry) = args.first().and_then(|n| find(n).ok()) else {
        eprintln!("usage: tanim --dump <name> [WxH] [frames] [warmup]");
        return ExitCode::from(2);
    };
    let (w, h) = args
        .get(1)
        .and_then(|s| s.split_once('x'))
        .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))
        .unwrap_or((80, 24));
    let frames: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100);
    let warmup: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut rng = opts.rng();
    let mut c = Canvas::new(w, h);
    let mut a = opts.make(entry, w, h, &mut rng);
    for _ in 0..warmup {
        a.step(&mut c, &mut rng);
    }
    let mut out = std::io::stdout().lock();
    let mut buf = Vec::with_capacity(w * h * 10);
    for _ in 0..frames {
        a.step(&mut c, &mut rng);
        buf.clear();
        for cell in &c.cells {
            buf.extend_from_slice(&(cell.ch as u32).to_le_bytes());
            buf.extend_from_slice(&[cell.fg.0, cell.fg.1, cell.fg.2, cell.bg.0, cell.bg.1, cell.bg.2]);
        }
        if out.write_all(&buf).is_err() {
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
