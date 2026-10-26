//! 自身模块的底板：盖住原生血条。三种样式：关、实底、模糊。
//! 实底直接画在 overlay 里，边缘羽化；模糊用一个单独的小窗口，让系统（DWM）
//! 模糊它后面的游戏画面，overlay 再在边缘叠一层渐变把硬边柔化。

use egui::epaint::Mesh;
use egui::{Color32, Id, LayerId, Order, Pos2, Rect, Shape, pos2, vec2};
use rh_core::modules;
use rh_core::settings::Backing as Mode;
use ui_kit::Theme;

use super::Frame;

#[derive(Default)]
pub struct Backing {
    win: Option<isize>,
    shown: bool,
    last: Option<(i32, i32, i32, i32, u8)>,
}

/// 四周羽化的矩形：外圈透明，往里渐变到 `color`。
fn feathered(rect: Rect, fx: f32, fy: f32, color: Color32) -> Shape {
    let xs = [rect.min.x, rect.min.x + fx, rect.max.x - fx, rect.max.x];
    let ys = [rect.min.y, rect.min.y + fy, rect.max.y - fy, rect.max.y];
    let mut mesh = Mesh::default();
    for (j, &y) in ys.iter().enumerate() {
        for (i, &x) in xs.iter().enumerate() {
            let inner = (1..=2).contains(&i) && (1..=2).contains(&j);
            mesh.colored_vertex(Pos2::new(x, y), if inner { color } else { Color32::TRANSPARENT });
        }
    }
    for j in 0..3u32 {
        for i in 0..3u32 {
            let a = j * 4 + i;
            mesh.add_triangle(a, a + 1, a + 5);
            mesh.add_triangle(a, a + 5, a + 4);
        }
    }
    Shape::mesh(mesh)
}

impl Backing {
    pub fn hide(&mut self) {
        if self.shown
            && let Some(h) = self.win
        {
            crate::win::hide_window(h);
        }
        self.shown = false;
        self.last = None;
    }

    pub fn draw(&mut self, ctx: &egui::Context, f: &Frame, _theme: &Theme, screen: Rect, s: f32, overlay: Option<isize>) {
        let o = &f.profile.player;
        if o.backing == Mode::None || f.snap.player.hp_max <= 0.0 {
            self.hide();
            return;
        }
        let (_, [x, y, w, h]) = modules::player_metrics(f.game);
        let (sw, sh) = (o.backing_w.clamp(0.6, 1.4), o.backing_h.clamp(0.6, 1.8));
        let design = Rect::from_min_size(pos2(x + w * (1.0 - sw) / 2.0, y + h * (1.0 - sh) / 2.0), vec2(w * sw, h * sh));
        let rect = Rect::from_min_max(screen.min + design.min.to_vec2() * s, screen.min + design.max.to_vec2() * s);
        let painter = ctx.layer_painter(LayerId::new(Order::Background, Id::unique("player-backing")));
        let op = o.backing_opacity.clamp(0.0, 1.0);
        let (fx, fy) = (28.0 * s, 22.0 * s);
        match o.backing {
            Mode::Solid => {
                self.hide();
                painter.add(feathered(rect, fx, fy, Color32::from_rgba_unmultiplied(10, 10, 10, (op * 255.0) as u8)));
            }
            Mode::Blur => {
                // 模糊窗口只盖中间不羽化的部分，四周用渐变过渡
                let ppp = ctx.pixels_per_point();
                let Some(origin) = crate::win::window_origin(overlay) else { return };
                let inner = rect.shrink2(vec2(fx * 0.5, fy * 0.5));
                let phys = (
                    origin.0 + (inner.min.x * ppp) as i32,
                    origin.1 + (inner.min.y * ppp) as i32,
                    (inner.width() * ppp) as i32,
                    (inner.height() * ppp) as i32,
                    (op * 0.6 * 255.0) as u8,
                );
                let hwnd = *self.win.get_or_insert_with(crate::win::create_blur_window);
                if self.last != Some(phys) || !self.shown {
                    crate::win::place_blur_window(hwnd, (phys.0, phys.1, phys.2, phys.3), phys.4, o.blur, overlay);
                    self.last = Some(phys);
                    self.shown = true;
                }
                painter.add(feathered(rect, fx, fy, Color32::from_rgba_unmultiplied(10, 10, 10, (op * 0.35 * 255.0) as u8)));
            }
            Mode::None => {}
        }
    }
}
