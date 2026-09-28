# AGENTS.md

Guidance for AI coding agents (and humans) working on tanim: endless procedural screensaver animations for the terminal, written in Rust, also compiled to WebAssembly for the browser. See `README.md` for the user-facing side.

## Commands

```sh
cargo build --release                        # also builds the wasm module (see build.rs)
cargo test --release                         # unit tests
./target/release/tanim --check               # every animation headless at 1x1 up to 240x70, with cost per frame
./target/release/tanim --check dog,dungeon   # just some
./target/release/tanim --dump <name> WxH <frames> [warmup] [--seed n] [--zoom n]   # raw frames for scripts
./target/release/tanim --snapshot <name> WxH <frame>    # one frame as text
make install                                 # ~/.local/bin/tanim
make html                                    # web pages in html/
make videos                                  # WebM per animation in videos/ (needs Python + ffmpeg)
make streamdeck MODELS=neo                   # Stream Deck GIFs in streamdeck/<model>/
make streamdeck-publish                      # force-push streamdeck/ to the gh-pages-assets branch
```

Before finishing any change: the build must have **zero warnings**, `cargo test --release` must pass, and `tanim --check` must run the touched animations without panicking. Look at the result too: render frames to PNG from `--dump` (the rasterizer in `scripts/video.py` can be imported for this) and check them by eye, because most bugs here are visual.

## Layout

| Path | What |
|---|---|
| `src/lib.rs` | Library root: `anims`, `canvas`, `rng`, `zoom` and the `Key` enum. Shared by the terminal binary and the web build. |
| `src/main.rs`, `src/term.rs` | Terminal binary: CLI parsing, frame loop, raw mode, key parsing (libc only). |
| `src/canvas.rs` | `Canvas` (cells), `Rgb` helpers and `Screen`, the diffing renderer that emits escape sequences. |
| `src/anims/` | One module per animation plus `mod.rs` with the `Animation` trait and the `catalog!` macro. |
| `src/zoom.rs` | Generic digital zoom and pan wrapped around every animation. |
| `src/export.rs`, `src/web.html`, `src/favicon.png` | `--export-html`: standalone pages embedding the wasm module and favicon (base64) and the canvas player. |
| `web/` | Separate crate (its own workspace) exposing the animations to JavaScript as plain `extern "C"` functions. |
| `build.rs` | Builds `web/` for `wasm32-unknown-unknown` and embeds it; without that target the build succeeds and the export is disabled. |
| `scripts/video.py` | Rasterizes `--dump` output and encodes WebM videos or Stream Deck GIFs with ffmpeg. |
| `.github/workflows/pages.yml` | Rebuilds and publishes the web version to GitHub Pages on every push to `main`, adding the Stream Deck GIFs from the `gh-pages-assets` branch at `streamdeck/`. |

## How rendering works

- An animation draws into a `Canvas` of `w x h` terminal cells. Nearly all of them draw **half-block pixels**: each cell is `▀` with the foreground as the top pixel and the background as the bottom one, giving a `w x 2h` grid of square pixels. The usual pattern is a `Vec<Rgb>` of `w * 2h` pixels passed to `c.blit_pixels(&px)`; `c.pixel()` paints single ones. Text still works with `c.text()` after blitting (it keeps each cell's bottom pixel as background). Only `matrix` is glyph-based on purpose.
- `Screen::render` diffs against what the terminal already shows and sends only changed cells, wrapped in synchronized output (mode 2026). A small deadband skips color changes of 2 or less per channel. **Output bytes per frame are the real performance cost** (terminal CPU), far more than compute: keep static parts static, avoid per-pixel noise that changes every frame, and check the `B/f` column of `--check` before and after a change.
- Animations registered with `clear = module::BG` in the catalog get a transparent background: pixels exactly equal to that color are drawn as the terminal's default background. Keep empty pixels exactly `BG` (snap near-black fades to it), or they show as dark boxes on a transparent terminal.
- The frame loop runs each animation at its catalog fps and, when the terminal falls behind, steps without drawing (up to 5 steps) instead of going into slow motion.

## Adding or changing an animation

1. Create `src/anims/<name>.rs` with a `//!` header describing what it shows and how it works, and `pub fn new(w: usize, h: usize, rng: &mut Rng) -> Box<dyn Animation>`.
2. Register it in the `catalog!` macro in `src/anims/mod.rs` with its fps (usually 20 or 30), an English one-line description, and `clear = module::BG` if the background is a plain dark color.
3. Add a row to the catalog table in `README.md`.
4. It must survive **every size down to 1x1** (`--check` tests 1x1, 2x2, 7x3, 13x5, 80x24 and 240x70): guard empty ranges, `clamp` with min > max, divisions by zero and out-of-bounds indexing.
5. Use only the `Rng` passed in: no `std::time`, `std::process` or other OS calls inside `src/anims`, `canvas`, `rng` or `zoom`, because that code also runs as wasm. `Rng::new(seed)` must keep giving a stream per seed (`--seed` reproducibility is relied on by the scripts).
6. Keys: `Animation::key(&mut self, Key)` receives `Space` and, for animations with `has_camera() == true` (only `dog`), the zoom keys too. Everything else gets zoom and pan from `src/zoom.rs`. `key()` has no rng: set a flag and act on it in the next `step()` (see `dungeon`).

Tests live next to the code (`#[cfg(test)] mod tests`): for example `dog` checks that its camera never crops the dog, `zoom` that 1x is a pass-through, and `canvas` the transparency and deadband rules. Add one when a behaviour can regress silently.

## Web build

- `web/src/lib.rs` exports `count`, `name_ptr`/`name_len`, `desc_ptr`/`desc_len`, `fps`, `start(i, w, h, seed)`, `step() -> *const u32` (per cell: char, fg `0xRRGGBB`, bg `0xRRGGBB`) and `key(k)` (1 up, 2 down, 3-6 pan left/right/up/down, 7 space). Keep `src/web.html` in sync when changing them.
- The module must stay import-free (`WebAssembly.Module.imports` is empty) so pages work from `file://` without a server. Check with Node after changing `web/`.
- The player draws half blocks geometrically on a canvas at whole device pixels, repaints only changed cells and drops frames rather than slowing down.
- Its `⋯` options menu offers `streamdeck/<model>/<name>.gif` for download, checking each with a `HEAD` request and hiding the button when none is there (always the case from `file://`). The GIFs are not built in CI: `make streamdeck-publish` pushes the local `streamdeck/` as an orphan commit to `gh-pages-assets`, and the Pages workflow checks that branch out into `site/streamdeck`. Keep the model list in `src/web.html` in sync with `STREAMDECK` in `scripts/video.py`.

## Videos and Stream Deck GIFs

- `scripts/video.py` defaults: videos from a 240x72 virtual terminal at scale 1 (2160x1296), two-pass VP9 at CRF 28, up to 30 fps. Per-animation `TWEAKS` set warm-up, length, zoom or CRF.
- Stream Deck GIFs (`--streamdeck`) use each model's screensaver canvas: `neo` 480x320 (what the Stream Deck app itself converts to), `mk2` 480x272, `xl` 768x384, `plus` 800x480, `plusxl` 1280x800, with the cell width chosen so that the half-block pixels tile exactly.
- **The Stream Deck app keeps only the first 120 frames** (8 s at 15 fps) of a screensaver, so GIFs must never exceed that. Loops are made seamless by recording extra footage, picking the most alike start and end, and cross-fading the last 0.75 s into the frames before the start (the GIF begins clean). They are re-encoded with fewer frames above 3 MB.

## Conventions

- Code, comments, CLI help and `README.md` are in English. Match the surrounding style: `//!` module headers that explain the idea, short comments only where something is not obvious, no needless abstractions.
- Prefer `Rgb` helpers (`lerp`, `scale`, `add`, `gradient`, `hsv`) and pre-computed tables for per-pixel work.
- Generated output (`target/`, `videos/`, `streamdeck/`, `html/`) is git-ignored; `docs/gallery/` holds the GIFs shown in the README.
- Commit messages follow Conventional Commits in English (`feat:`, `fix:`, `docs:`, `ci:`…).
