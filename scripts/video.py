"""Render every animation (or the ones named) to a WebM video in videos/,
or to GIF screensavers for Elgato Stream Deck in streamdeck/<model>/.

Frames come from `tanim --dump`, get rasterized here with a monospace font
(block and box-drawing characters are drawn geometrically so they tile without
seams) and are encoded by ffmpeg: two-pass VP9 for videos, a per-animation
palette for GIFs.

    python3 scripts/video.py              # all animations
    python3 scripts/video.py fire maze    # only some
    python3 scripts/video.py --size 100x30 --seconds 6 --fps 20 --scale 1
    python3 scripts/video.py dog --seed 42    # same video every time
    python3 scripts/video.py --streamdeck neo,mk2   # Stream Deck GIFs
    python3 scripts/video.py --streamdeck all aurora
    python3 scripts/video.py --sync DIR   # update the published GIFs (CI)

Requires Pillow, numpy, fontTools and ffmpeg.
"""

import argparse
import os
import subprocess
import sys
import tempfile

import numpy as np
from fontTools.ttLib import TTCollection, TTFont
from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "target", "release", "tanim")
OUT = os.path.join(ROOT, "videos")
STREAMDECK_OUT = os.path.join(ROOT, "streamdeck")
# Stream Deck models with animated screensavers: GIF size and the cell width
# in pixels that tiles it exactly (cells are twice as tall, so every half-block
# pixel is a whole square). Neo is the canvas the Stream Deck app itself
# converts screensavers to, MK.2 Elgato's recommendation, + XL its spec sheet,
# + the community's; XL follows its key grid (2:1).
STREAMDECK = {
    "neo": (480, 320, 4),
    "mk2": (480, 272, 4),
    "xl": (768, 384, 6),
    "plus": (800, 480, 5),
    "plusxl": (1280, 800, 8),
}
# Seconds over which a GIF's end cross-fades into its start, so it loops
# without a jump. Short, because the loop points are picked to look alike.
LOOP_FADE = 0.75
# Keep screensavers light (Elgato publishes no limit): past this, re-encode
# with fewer frames.
STREAMDECK_MAX_BYTES = 3_000_000
# The Stream Deck app keeps only the first 120 frames (8 s at 15 fps) of a
# screensaver: anything past that, where the loop closes, would be cut.
STREAMDECK_MAX_FRAMES = 120
# VP9 settings picked by measuring size against SSIM on a lossless reference:
# CRF 28 is visually indistinguishable at 2x zoom, and two passes at cpu-used 1
# save 17-40% over a single fast pass at the same quality.
def vp9(crf=28):
    return ["-c:v", "libvpx-vp9", "-crf", str(crf), "-b:v", "0", "-pix_fmt", "yuv420p",
            "-row-mt", "1", "-deadline", "good", "-cpu-used", "1", "-an"]

# Cell size at scale 1; `configure` multiplies these by --scale.
BASE_CW, BASE_CH, BASE_FONT = 9, 18, 15
SCALE = 2
CW, CH, FONT_SIZE = BASE_CW * SCALE, BASE_CH * SCALE, BASE_FONT * SCALE
SS = 4  # supersampling for geometric glyphs

# Glyph fallback chain: first font whose cmap has the char wins. macOS paths
# first, then the Debian/Ubuntu packages fonts-dejavu-core and
# fonts-ipafont-gothic (half-width katakana for matrix) that CI installs.
FONTS = [
    ("~/Library/Fonts/DejaVuSansMono.ttf", 0),
    ("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", 0),
    ("/System/Library/Fonts/Menlo.ttc", 0),
    ("~/Library/Fonts/DejaVuSans.ttf", 0),
    ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0),
    ("/System/Library/Fonts/Apple Symbols.ttf", 0),
    ("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc", 0),
    ("/usr/share/fonts/opentype/ipafont-gothic/ipag.ttf", 0),
]

# Per-animation tweaks: steps to simulate before recording (so scenes that
# build up are not empty), recording length in seconds and starting zoom.
TWEAKS = {
    "garden": {"warmup": 150},
    "snow": {"warmup": 600},
    "sand": {"warmup": 300},
    "tetris": {"warmup": 300},
    "pipes": {"warmup": 100},
    "life": {"warmup": 30},
    "fireworks": {"warmup": 30},
    "boids": {"warmup": 60},
    "rain": {"warmup": 60},
    "matrix": {"warmup": 60},
    "starfield": {"warmup": 30},
    "maze": {"seconds": 14},
    # Past the fade-in, on the START sign.
    "maze3d": {"warmup": 20},
    "dog": {"seconds": 20},
    # Long enough to see a level being built and then explored.
    "dungeon": {"seconds": 30},
    # Pure noise barely compresses; a higher CRF keeps it to a few MB and the
    # flicker hides the difference.
    "fire": {"warmup": 30, "crf": 42},
    "physarum": {"warmup": 400},
    "neurons": {"warmup": 250},
    "aurora": {"warmup": 100},
}



def configure(scale):
    global SCALE, CW, CH, FONT_SIZE
    SCALE = scale
    CW, CH, FONT_SIZE = BASE_CW * scale, BASE_CH * scale, BASE_FONT * scale


def configure_cells(cw):
    """Exact cell width in pixels (height twice that), for fixed-size GIFs."""
    global SCALE, CW, CH, FONT_SIZE
    SCALE = 1
    CW, CH, FONT_SIZE = cw, 2 * cw, max(4, round(cw * BASE_FONT / BASE_CW))


def load_fonts():
    fonts = []
    for path, index in FONTS:
        path = os.path.expanduser(path)
        if not os.path.exists(path):
            continue
        tt = TTCollection(path).fonts[index] if path.endswith(".ttc") else TTFont(path)
        cmap = set(tt.getBestCmap() or {})
        fonts.append((ImageFont.truetype(path, FONT_SIZE, index=index), cmap))
    if not fonts:
        sys.exit("video.py: no usable font found")
    return fonts


def rect(d, x0, y0, x1, y1):
    d.rectangle([round(x0), round(y0), round(x1) - 1, round(y1) - 1], fill=255)


def lines_mask(ch):
    """Box drawing and diagonals, drawn at SS x resolution."""
    w, h = CW * SS, CH * SS
    img = Image.new("L", (w, h), 0)
    d = ImageDraw.Draw(img)
    # Every stroke is a multiple of SCALE pixels wide, so centring on this
    # point makes them all land exactly on the pixel grid.
    cx = ((CW - SCALE) // 2 + SCALE / 2) * SS
    cy = ((CH - SCALE) // 2 + SCALE / 2) * SS
    light, heavy = SS * SCALE, SS * SCALE * 3

    # (up, down, left, right) stroke widths
    straight = {
        "─": (0, 0, light, light), "━": (0, 0, heavy, heavy),
        "│": (light, light, 0, 0), "┃": (heavy, heavy, 0, 0),
        "┌": (0, light, 0, light), "┐": (0, light, light, 0),
        "└": (light, 0, 0, light), "┘": (light, 0, light, 0),
        "┏": (0, heavy, 0, heavy), "┓": (0, heavy, heavy, 0),
        "┗": (heavy, 0, 0, heavy), "┛": (heavy, 0, heavy, 0),
    }
    if ch in straight:
        up, down, left, right = straight[ch]
        t = max(up, down, left, right)
        if up:
            rect(d, cx - up / 2, 0, cx + up / 2, cy + t / 2)
        if down:
            rect(d, cx - down / 2, cy - t / 2, cx + down / 2, h)
        if left:
            rect(d, 0, cy - left / 2, cx + t / 2, cy + left / 2)
        if right:
            rect(d, cx - t / 2, cy - right / 2, w, cy + right / 2)
    elif ch in "═║╔╗╚╝":
        t, g = light, SS * SCALE * 2  # stroke and half gap between the two lines
        if ch == "═":
            rect(d, 0, cy - g - t / 2, w, cy - g + t / 2)
            rect(d, 0, cy + g - t / 2, w, cy + g + t / 2)
        elif ch == "║":
            rect(d, cx - g - t / 2, 0, cx - g + t / 2, h)
            rect(d, cx + g - t / 2, 0, cx + g + t / 2, h)
        else:
            # Draw ╔ and mirror it into the other corners.
            for o in (-g, g):
                rect(d, cx + o - t / 2, cy + o - t / 2, w, cy + o + t / 2)
                rect(d, cx + o - t / 2, cy + o - t / 2, cx + o + t / 2, h)
            if ch in "╗╝":
                img = img.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
            if ch in "╚╝":
                img = img.transpose(Image.Transpose.FLIP_TOP_BOTTOM)
    elif ch in "╭╮╯╰":
        # Quarter ellipse through the two edge midpoints, drawn as ╭ then mirrored.
        t = round(light)
        d.arc([cx - t / 2, cy - t / 2, w + (w - cx) - t / 2, h + (h - cy) - t / 2],
              180, 270, fill=255, width=t)
        if ch in "╮╯":
            img = img.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
        if ch in "╰╯":
            img = img.transpose(Image.Transpose.FLIP_TOP_BOTTOM)
    elif ch == "╌":
        for x0 in (0, w / 2):
            rect(d, x0 + w / 8, cy - light / 2, x0 + w * 3 / 8, cy + light / 2)
    elif ch == "╱":
        d.line([(0, h), (w, 0)], fill=255, width=round(light))
    elif ch == "╲":
        d.line([(0, 0), (w, h)], fill=255, width=round(light))
    else:
        return None
    img = img.resize((CW, CH), Image.Resampling.BOX)
    return np.asarray(img, dtype=np.float32) / 255


def block_mask(ch):
    """Block elements, pixel exact so neighbouring cells tile seamlessly."""
    m = np.zeros((CH, CW), np.float32)
    hh, hw = CH // 2, round(CW / 2)
    if ch == "█":
        m[:] = 1
    elif ch == "▀":
        m[:hh] = 1
    elif ch == "▄":
        m[hh:] = 1
    elif ch == "▌":
        m[:, :hw] = 1
    elif ch == "▐":
        m[:, hw:] = 1
    elif "▁" <= ch <= "▇":
        k = ord(ch) - ord("▁") + 1
        m[CH - round(CH * k / 8):] = 1
    elif ch == "▕":
        m[:, CW - max(1, round(CW / 8)):] = 1
    elif ch == "░":
        m[:] = 0.25
    elif ch == "▙":
        m[hh:] = 1
        m[:hh, :hw] = 1
    elif ch == "▟":
        m[hh:] = 1
        m[:hh, hw:] = 1
    else:
        return None
    return m


def font_mask(ch, fonts):
    cp = ord(ch)
    font = next((f for f, cmap in fonts if cp in cmap), fonts[0][0])
    adv = font.getlength(ch)
    if adv > CW + 0.5:  # too wide for one cell: shrink to fit
        font = font.font_variant(size=max(6, int(FONT_SIZE * CW / adv)))
        adv = font.getlength(ch)
    asc, desc = font.getmetrics()
    img = Image.new("L", (CW, CH), 0)
    ImageDraw.Draw(img).text(((CW - adv) / 2, (CH - asc - desc) / 2 + asc), ch,
                             font=font, fill=255, anchor="ls")
    return np.asarray(img, dtype=np.float32) / 255


class Glyphs:
    def __init__(self):
        self.fonts = load_fonts()
        self.index = {}
        self.masks = []
        self._stacked = None

    def stacked(self):
        if self._stacked is None:
            self._stacked = np.stack(self.masks)
        return self._stacked

    def ids(self, codes):
        out = np.empty(codes.shape, np.int32)
        uniq, inv = np.unique(codes, return_inverse=True)
        for u in uniq:
            if int(u) not in self.index:
                ch = chr(int(u))
                m = block_mask(ch)
                if m is None:
                    m = lines_mask(ch)
                if m is None:
                    m = np.zeros((CH, CW), np.float32) if ch == " " else font_mask(ch, self.fonts)
                self.index[int(u)] = len(self.masks)
                self.masks.append(m)
        self._stacked = None
        lut = np.array([self.index[int(u)] for u in uniq], np.int32)
        out[:] = lut[inv.reshape(codes.shape)]
        return out


def catalog():
    out = subprocess.run([BIN, "--list"], capture_output=True, text=True, check=True).stdout
    return [(line.split()[0]) for line in out.splitlines() if line.strip()]


def fps_of(name):
    src = open(os.path.join(ROOT, "src", "anims", "mod.rs")).read()
    for line in src.splitlines():
        parts = line.strip().split(",")
        if len(parts) >= 3 and parts[0] == name:
            return int(parts[1])
    return 30


def gif_args(out):
    return ["-filter_complex",
            "split[a][b];[a]palettegen=max_colors=256:stats_mode=full[p];"
            # No dithering: 15-45% smaller, and at these sizes no visible banding.
            "[b][p]paletteuse=dither=none:diff_mode=rectangle",
            "-loop", "0", out]


def render_streamdeck(name, model, seconds, max_fps, seed, glyphs, out):
    """A GIF sized for `model`, re-encoded with fewer frames (and then a
    shorter loop) until it fits STREAMDECK_MAX_BYTES."""
    width, height, cw = STREAMDECK[model]
    cols, rows = width // cw, height // (2 * cw)
    for fps, secs in ((max_fps, seconds), (12, seconds), (10, seconds), (10, seconds * 0.6), (8, seconds * 0.5)):
        fps = min(fps, max_fps)
        secs = min(secs, STREAMDECK_MAX_FRAMES / fps)
        render(name, cols, rows, secs, fps, seed, glyphs, out, gif_args, fixed_length=True,
               loop_fade=LOOP_FADE)
        if os.path.getsize(out) <= STREAMDECK_MAX_BYTES:
            break
    return fps, secs


def render_streamdecks(models, names, out, seconds, max_fps, seed):
    """GIFs for every model and animation given, in out/<model>/<name>.gif."""
    for m in models:
        configure_cells(STREAMDECK[m][2])
        glyphs = Glyphs()
        os.makedirs(os.path.join(out, m), exist_ok=True)
        for name in names:
            path = os.path.join(out, m, f"{name}.gif")
            fps, secs = render_streamdeck(name, m, seconds, max_fps, seed, glyphs, path)
            print(f"{m:7} {name:10} {os.path.getsize(path) / 1e6:4.1f} MB  {fps:2} fps {secs:4.1f} s  {os.path.relpath(path)}", flush=True)


def module_hash(name):
    """Git blob hash of the animation's module at HEAD, or "" if unknown."""
    r = subprocess.run(["git", "rev-parse", f"HEAD:src/anims/{name}.rs"], cwd=ROOT, capture_output=True, text=True)
    return r.stdout.strip() if r.returncode == 0 else ""


def sync(out, force, seconds, max_fps, seed):
    """Bring the published Stream Deck GIFs in `out` up to date: render every
    model for the animations that miss any GIF, whose module changed since
    they were made (as recorded in out/sources.txt) or that are in `force`,
    and drop the GIFs of animations no longer in the catalog. GIFs with no
    record yet are taken as current."""
    names = catalog()
    manifest = os.path.join(out, "sources.txt")
    made = {}
    if os.path.exists(manifest):
        for line in open(manifest):
            name, _, blob = line.strip().partition(" ")
            if name:
                made[name] = blob
    now = {n: module_hash(n) for n in names}
    complete = lambda n: all(os.path.exists(os.path.join(out, m, f"{n}.gif")) for m in STREAMDECK)
    todo = [n for n in names if n in force or not complete(n) or made.get(n, now[n]) != now[n]]
    for m in STREAMDECK:
        d = os.path.join(out, m)
        for f in sorted(os.listdir(d)) if os.path.isdir(d) else []:
            if f.endswith(".gif") and f[:-4] not in names:
                os.remove(os.path.join(d, f))
                print(f"removed {m}/{f}")
    print(f"regenerating: {' '.join(todo) or 'nothing'}", flush=True)
    render_streamdecks(list(STREAMDECK), todo, out, seconds, max_fps, seed)
    with open(manifest, "w") as f:
        f.writelines(f"{n} {now[n]}\n" for n in names if complete(n))


def lossless_args(out):
    return ["-c:v", "ffv1", "-level", "3", "-pix_fmt", "bgr0", out]


def render_video(name, w, h, seconds, max_fps, seed, glyphs, out):
    """Two-pass VP9 needs the frames twice, so keep a lossless copy in between."""
    with tempfile.TemporaryDirectory() as tmp:
        master = os.path.join(tmp, "master.mkv")
        log = os.path.join(tmp, "vp9")
        render(name, w, h, seconds, max_fps, seed, glyphs, master, lossless_args)
        for n, dst in ((1, os.devnull), (2, out)):
            fmt = ["-f", "null"] if n == 1 else []
            subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", master] + vp9(TWEAKS.get(name, {}).get("crf", 28))
                           + ["-pass", str(n), "-passlogfile", log] + fmt + [dst], check=True)


def rasterize(f, glyphs):
    """One frame of cells to an RGB image."""
    h, w = f.shape
    ids = glyphs.ids(f["ch"])
    masks = glyphs.stacked()[ids]  # h, w, CH, CW
    fg = f["fg"].astype(np.float32)[:, :, None, None, :]
    bg = f["bg"].astype(np.float32)[:, :, None, None, :]
    img = bg + (fg - bg) * masks[..., None]
    return (img.transpose(0, 2, 1, 3, 4).reshape(h * CH, w * CW, 3) + 0.5).astype(np.uint8)


def seamless(imgs, frames, fade):
    """Make a loop of about `frames` frames out of the longer `imgs`.

    Procedural animations never return to an earlier state, so pick a start
    and an end whose preceding `fade` frames look most alike (at 75-100% of
    the wanted length, never longer), keep what lies between, and cross-fade
    its last `fade` frames into the ones just before the start: the loop
    begins clean and its final frame leads straight back into the first.
    """
    thumbs = np.stack([im[::6, ::6].reshape(-1) for im in imgs]).astype(np.float32)
    lo, hi = max(fade, round(frames * 0.75)), frames
    best = (np.inf, fade, fade + frames)
    for start in range(fade, len(imgs) - hi + 1):
        ends = np.arange(start + lo, start + hi + 1)
        cost = sum(np.abs(thumbs[ends - fade + i] - thumbs[start - fade + i]).mean(axis=1) for i in range(fade))
        k = int(np.argmin(cost))
        if cost[k] < best[0]:
            best = (cost[k], start, int(ends[k]))
    _, start, end = best
    out = imgs[start:end]
    n = len(out)
    for i in range(fade):
        t = (i + 1) / fade
        out[n - fade + i] = (imgs[end - fade + i] * (1 - t) + imgs[start - fade + i] * t + 0.5).astype(np.uint8)
    return out


def render(name, w, h, seconds, max_fps, seed, glyphs, out, encode=lossless_args, fixed_length=False, loop_fade=0.0):
    """Rasterize `name` and pipe the frames to ffmpeg, which writes `out`
    with the output options returned by `encode(out)`. With `loop_fade`
    (seconds) the result loops seamlessly."""
    tw = TWEAKS.get(name, {})
    fps = fps_of(name)
    out_fps = min(fps, max_fps)
    if not fixed_length:
        seconds = tw.get("seconds", seconds)
    frames = round(seconds * out_fps)
    fade = min(round(loop_fade * out_fps), frames // 3)
    # A loop needs extra footage: room to pick its start and end, plus the
    # stretch it fades out of.
    extra = (0.5 * frames + fade) / out_fps if fade else 0.0
    steps = round((seconds + extra) * fps)
    cmd = [BIN, "--dump", name, f"{w}x{h}", str(steps), str(tw.get("warmup", 0))]
    if seed is not None:
        cmd += ["--seed", str(seed)]
    if tw.get("zoom"):
        cmd += ["--zoom", str(tw["zoom"])]
    dump = subprocess.run(
        cmd,
        capture_output=True, check=True,
    ).stdout
    cells = np.frombuffer(dump, dtype=np.dtype([("ch", "<u4"), ("fg", "u1", 3), ("bg", "u1", 3)]))
    # Keep the simulation step nearest to each output frame's timestamp.
    total = frames + (round(0.5 * frames) + fade if fade else 0)
    pick = np.minimum(np.round((np.arange(total) + 1) * fps / out_fps).astype(int) - 1, steps - 1)
    cells = cells.reshape(-1, h, w)[pick]

    ff = subprocess.Popen(
        ["ffmpeg", "-v", "error", "-y",
         "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{w * CW}x{h * CH}", "-r", f"{out_fps}",
         "-i", "-"] + encode(out),
        stdin=subprocess.PIPE,
    )
    assert ff.stdin
    if fade:
        for img in seamless([rasterize(f, glyphs) for f in cells], frames, fade):
            ff.stdin.write(img.tobytes())
    else:
        for f in cells[:frames]:
            ff.stdin.write(rasterize(f, glyphs).tobytes())
    ff.stdin.close()
    if ff.wait() != 0:
        sys.exit(f"video.py: ffmpeg failed on {name}")
    return out


def main():
    ap = argparse.ArgumentParser(description=(__doc__ or "").split("\n")[0])
    ap.add_argument("names", nargs="*", help="animations to render (default: all); with --sync, "
                    "ones to regenerate even if up to date, or all")
    # A big virtual terminal gives the animations plenty of pixels to draw
    # with; at scale 1 that is a 2160x1296 video.
    ap.add_argument("--size", default="240x72", help="terminal size in cells (default 240x72)")
    ap.add_argument("--seconds", type=float, help="length (default 10 for videos, 8 for Stream Deck GIFs)")
    ap.add_argument("--fps", type=int, help="max frame rate (default 30 for videos, 15 for GIFs)")
    ap.add_argument("--streamdeck", metavar="MODELS",
                    help="Stream Deck GIFs instead of videos: comma-separated "
                         f"{', '.join(STREAMDECK)} or all")
    ap.add_argument("--sync", metavar="DIR",
                    help="update the Stream Deck GIFs published in DIR (every model): new animations, "
                         "changed modules and the names given")
    ap.add_argument("--scale", type=int, default=1, help="pixels per cell unit (default 1)")
    ap.add_argument("--seed", type=int, help="fixed seed, for videos that come out the same every time")
    args = ap.parse_args()
    w, h = map(int, args.size.split("x"))
    names = catalog()
    if args.sync and args.names == ["all"]:
        args.names = names
    for n in args.names:
        if n not in names:
            sys.exit(f"video.py: unknown animation '{n}'")
    if args.sync:
        sync(os.path.abspath(args.sync), set(args.names), args.seconds or 8, args.fps or 15, args.seed)
        return
    if args.streamdeck:
        models = list(STREAMDECK) if args.streamdeck == "all" else args.streamdeck.split(",")
        for m in models:
            if m not in STREAMDECK:
                sys.exit(f"video.py: unknown Stream Deck model '{m}' (choose from {', '.join(STREAMDECK)})")
        render_streamdecks(models, args.names or names, STREAMDECK_OUT, args.seconds or 8, args.fps or 15, args.seed)
        return
    configure(args.scale)
    os.makedirs(OUT, exist_ok=True)
    glyphs = Glyphs()
    for name in args.names or names:
        path = os.path.join(OUT, f"{name}.webm")
        render_video(name, w, h, args.seconds or 10, args.fps or 30, args.seed, glyphs, path)
        print(f"{name:10} {os.path.getsize(path) / 1e6:5.1f} MB  {os.path.relpath(path, ROOT)}")


if __name__ == "__main__":
    main()
