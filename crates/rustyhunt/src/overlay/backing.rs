//! Backing of the player module: a soft dark area behind it that also hides the native health bar
//! under its default position. It follows the module wherever it is placed. Three styles: off,
//! solid, blur.
//!
//! Solid is drawn in the overlay as a dark rectangle that fades out over a wide margin. Blur uses
//! the system (DWM) acrylic effect, which always fills a whole window with a hard edge, so it is
//! built from nested windows, each slightly smaller and only partly opaque: together the blur
//! builds up from nothing at the outer edge to full strength inside. Window shapes (regions) and
//! per-pixel alpha can't be used because acrylic ignores them, and the plain blur effect shows as
//! solid black on some systems.

use egui::epaint::Mesh;
use egui::{Color32, Id, LayerId, Order, Pos2, Rect, Shape, vec2};
use rh_core::modules;
use rh_core::settings::Backing as Mode;

use super::Frame;

/// Nested blur windows from the outer edge inwards
const LAYERS: usize = 12;
/// Steps of the solid gradient on each side
const STEPS: usize = 8;

#[derive(Default)]
pub struct Backing {
    wins: Vec<isize>,
    shown: bool,
    last: Option<((i32, i32, i32, i32), u32)>,
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A rectangle that fades in from transparent at its edge to `color` over `fx` / `fy`, along a smooth curve.
fn feathered(rect: Rect, fx: f32, fy: f32, color: Color32) -> Shape {
    // Grid lines from each edge inwards; alpha on a line is the product of both axes
    let line = |lo: f32, hi: f32, f: f32| -> Vec<(f32, f32)> {
        let f = f.min((hi - lo) / 2.0);
        let mut v: Vec<(f32, f32)> = (0..=STEPS)
            .map(|i| (lo + f * i as f32 / STEPS as f32, smooth(i as f32 / STEPS as f32)))
            .collect();
        v.extend((0..=STEPS).rev().map(|i| (hi - f * i as f32 / STEPS as f32, smooth(i as f32 / STEPS as f32))));
        v
    };
    let xs = line(rect.min.x, rect.max.x, fx);
    let ys = line(rect.min.y, rect.max.y, fy);
    let mut mesh = Mesh::default();
    for &(y, ay) in &ys {
        for &(x, ax) in &xs {
            mesh.colored_vertex(Pos2::new(x, y), color.gamma_multiply(ax * ay));
        }
    }
    let n = xs.len() as u32;
    for j in 0..ys.len() as u32 - 1 {
        for i in 0..n - 1 {
            let a = j * n + i;
            mesh.add_triangle(a, a + 1, a + n + 1);
            mesh.add_triangle(a, a + n + 1, a + n);
        }
    }
    Shape::mesh(mesh)
}

impl Backing {
    pub fn hide(&mut self) {
        if self.shown {
            for &h in &self.wins {
                crate::win::hide_window(h);
            }
        }
        self.shown = false;
        self.last = None;
    }

    /// `module` is where the player module was drawn last frame (screen points); None before it has been drawn.
    pub fn draw(&mut self, ctx: &egui::Context, f: &Frame, module: Option<Rect>, screen: Rect, s: f32, overlay: Option<isize>) {
        let o = &f.profile.player;
        if o.backing == Mode::None || f.snap.player.hp_max <= 0.0 {
            self.hide();
            return;
        }
        // Around the module with a margin; before the first frame, over the native health bar
        let (fx, fy) = (40.0 * s, 30.0 * s);
        let base = module.map(|m| m.expand2(vec2(fx, fy))).unwrap_or_else(|| {
            let (_, [x, y, w, h]) = modules::player_metrics(f.game);
            Rect::from_min_size(screen.min + vec2(x, y) * s, vec2(w, h) * s)
        });
        let (sw, sh) = (o.backing_w.clamp(0.6, 1.4), o.backing_h.clamp(0.6, 1.8));
        let rect = Rect::from_center_size(base.center(), vec2(base.width() * sw, base.height() * sh));
        match o.backing {
            Mode::Solid => {
                self.hide();
                let op = o.backing_opacity.clamp(0.0, 1.0);
                let painter = ctx.layer_painter(LayerId::new(Order::Background, Id::unique("player-backing")));
                painter.add(feathered(rect, fx, fy, Color32::from_rgba_unmultiplied(10, 10, 10, (op * 255.0) as u8)));
            }
            Mode::Blur => {
                let ppp = ctx.pixels_per_point();
                let Some(origin) = crate::win::window_origin(overlay) else { return };
                let phys = |r: Rect| {
                    (
                        origin.0 + (r.min.x * ppp) as i32,
                        origin.1 + (r.min.y * ppp) as i32,
                        (r.width() * ppp) as i32,
                        (r.height() * ppp) as i32,
                    )
                };
                let peak = o.blur_strength.clamp(0.0, 1.0);
                let key = (phys(rect), (peak * 1000.0) as u32);
                if self.last == Some(key) && self.shown {
                    return;
                }
                while self.wins.len() < LAYERS {
                    self.wins.push(crate::win::create_blur_window());
                }
                // Layer k covers the rect shrunk by k/LAYERS of the margin. Coverage reached inside
                // layer k follows a smooth curve up to `peak`; each layer adds what is missing.
                let mut covered = 0.0f32;
                for (k, &h) in self.wins.iter().enumerate() {
                    let t = k as f32 / LAYERS as f32;
                    let r = rect.shrink2(vec2(fx * t, fy * t));
                    let target = peak * smooth((k + 1) as f32 / LAYERS as f32);
                    let alpha = if covered < 1.0 { 1.0 - (1.0 - target) / (1.0 - covered) } else { 0.0 };
                    covered = target;
                    crate::win::place_blur_window(h, phys(r), (alpha * 255.0).round().clamp(1.0, 255.0) as u8, overlay);
                }
                self.last = Some(key);
                self.shown = true;
            }
            Mode::None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    #[test]
    fn feather_is_transparent_at_the_edge_and_full_inside() {
        let Shape::Mesh(m) = feathered(Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 100.0)), 40.0, 30.0, Color32::BLACK) else {
            panic!("mesh")
        };
        let at = |x: f32, y: f32| m.vertices.iter().find(|v| v.pos == pos2(x, y)).map(|v| v.color.a());
        assert_eq!(at(0.0, 0.0), Some(0));
        assert_eq!(at(40.0, 30.0), Some(255));
        assert_eq!(at(160.0, 70.0), Some(255));
        assert_eq!(at(0.0, 50.0), None, "the middle is spanned by the inner grid lines only");
    }
}
