//! Digital zoom for any animation: it keeps drawing its whole scene and this
//! magnifies part of it, with keys choosing how much and where. At 1x it is a
//! straight pass-through, so glyphs and text are left untouched.

use crate::anims::Animation;
use crate::canvas::{Canvas, Rgb, HALF};
use crate::rng::Rng;
use crate::Key;

const LEVELS: [f32; 5] = [1.0, 1.5, 2.0, 3.0, 4.0];
/// Share of the visible width or height one pan key press moves the view.
const PAN_STEP: f32 = 0.15;
/// Per-tick easing towards the chosen zoom and position.
const EASE: f32 = 0.2;

struct Zoom {
    inner: Box<dyn Animation>,
    scene: Canvas,
    px: Vec<Rgb>,
    level: usize,
    scale: f32,
    /// View centre in scene pixels, and where it is heading.
    cx: f32,
    cy: f32,
    tx: f32,
    ty: f32,
}

/// Give `anim` zoom and pan, unless it has a camera of its own.
pub fn wrap(anim: Box<dyn Animation>, w: usize, h: usize) -> Box<dyn Animation> {
    if anim.has_camera() {
        return anim;
    }
    let (cx, cy) = (w as f32 / 2.0, h as f32);
    Box::new(Zoom {
        inner: anim,
        scene: Canvas::new(w, h),
        px: vec![Rgb::BLACK; w * 2 * h],
        level: 0,
        scale: 1.0,
        cx,
        cy,
        tx: cx,
        ty: cy,
    })
}

/// Color of scene pixel `(x, py)`, reading half blocks back into pixels.
/// Other glyphs become a blend of their two colors.
fn pixel(c: &Canvas, x: usize, py: usize) -> Rgb {
    let cell = c.cells[(py / 2) * c.w + x];
    let top = py % 2 == 0;
    match cell.ch {
        HALF => {
            if top {
                cell.fg
            } else {
                cell.bg
            }
        }
        '▄' => {
            if top {
                cell.bg
            } else {
                cell.fg
            }
        }
        '█' => cell.fg,
        ' ' => cell.bg,
        _ => cell.bg.lerp(cell.fg, 0.5),
    }
}

/// Half the visible width and height, in scene pixels, at `scale`.
fn half_view(w: usize, h: usize, scale: f32) -> (f32, f32) {
    (w as f32 / (2.0 * scale), h as f32 / scale)
}

/// Clamp a view centre so the view stays inside the scene.
fn inside(v: f32, half: f32, size: f32) -> f32 {
    v.clamp(half, (size - half).max(half))
}

impl Animation for Zoom {
    fn step(&mut self, c: &mut Canvas, rng: &mut Rng) {
        self.inner.step(&mut self.scene, rng);
        let goal = LEVELS[self.level];
        self.scale += (goal - self.scale) * EASE;
        if (goal - self.scale).abs() < 0.005 {
            self.scale = goal;
        }
        self.cx += (self.tx - self.cx) * EASE;
        self.cy += (self.ty - self.cy) * EASE;
        if self.scale == 1.0 {
            c.cells.copy_from_slice(&self.scene.cells);
            return;
        }
        let (w, ph) = (self.scene.w, 2 * self.scene.h);
        let (hx, hy) = half_view(w, self.scene.h, self.scale);
        let x0 = inside(self.cx, hx, w as f32) - hx;
        let y0 = inside(self.cy, hy, ph as f32) - hy;
        let inv = 1.0 / self.scale;
        for py in 0..ph {
            let sy = ((y0 + (py as f32 + 0.5) * inv) as usize).min(ph - 1);
            for x in 0..w {
                let sx = ((x0 + (x as f32 + 0.5) * inv) as usize).min(w - 1);
                self.px[py * w + x] = pixel(&self.scene, sx, sy);
            }
        }
        c.blit_pixels(&self.px);
    }

    fn key(&mut self, key: Key) {
        let (w, h) = (self.scene.w, self.scene.h);
        let (hx, hy) = half_view(w, h, LEVELS[self.level]);
        match key {
            Key::Up => self.level = (self.level + 1).min(LEVELS.len() - 1),
            Key::Down => self.level = self.level.saturating_sub(1),
            Key::PanLeft => self.tx -= 2.0 * hx * PAN_STEP,
            Key::PanRight => self.tx += 2.0 * hx * PAN_STEP,
            Key::PanUp => self.ty -= 2.0 * hy * PAN_STEP,
            Key::PanDown => self.ty += 2.0 * hy * PAN_STEP,
            // Anything else is for the animation itself.
            _ => return self.inner.key(key),
        }
        let (hx, hy) = half_view(w, h, LEVELS[self.level]);
        self.tx = inside(self.tx, hx, w as f32);
        self.ty = inside(self.ty, hy, 2.0 * h as f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Stripes;
    impl Animation for Stripes {
        fn step(&mut self, c: &mut Canvas, _: &mut Rng) {
            for y in 0..c.h {
                for x in 0..c.w {
                    let v = (x * 8) as u8;
                    c.set(x as i32, y as i32, HALF, Rgb(v, 0, 0), Rgb(v, 1, 0));
                }
            }
            c.text(0, 0, "hi", Rgb::WHITE);
        }
    }

    #[test]
    fn one_x_passes_everything_through() {
        let (mut z, mut rng) = (wrap(Box::new(Stripes), 20, 6), Rng::new(1));
        let mut c = Canvas::new(20, 6);
        z.step(&mut c, &mut rng);
        assert_eq!(c.cells[0].ch, 'h', "text should survive at 1x");
    }

    #[test]
    fn zooming_magnifies_and_panning_stays_inside() {
        let (mut z, mut rng) = (wrap(Box::new(Stripes), 20, 6), Rng::new(1));
        let mut c = Canvas::new(20, 6);
        z.key(Key::Up);
        z.key(Key::Up); // 2x
        for _ in 0..60 {
            z.step(&mut c, &mut rng);
        }
        // At 2x each scene column spans two screen columns.
        let row = 2 * 20;
        assert_eq!(c.cells[row + 10].fg.0, c.cells[row + 11].fg.0);
        assert_ne!(c.cells[row + 10].fg.0, c.cells[row + 12].fg.0);
        for _ in 0..20 {
            z.key(Key::PanLeft);
        }
        for _ in 0..60 {
            z.step(&mut c, &mut rng);
        }
        // Panned hard left: the view starts at the scene's left edge (the
        // red channel encodes the scene column).
        assert_eq!(c.cells[row].fg.0, 0);
        // Back to 1x: straight pass-through again.
        z.key(Key::Down);
        z.key(Key::Down);
        for _ in 0..60 {
            z.step(&mut c, &mut rng);
        }
        assert_eq!(c.cells[0].ch, 'h');
    }
}
