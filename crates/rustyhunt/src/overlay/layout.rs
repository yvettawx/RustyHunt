//! Layout mode: the overlay takes the mouse and the game dims. Drag a module to move it (snaps to 8px),
//! drag a corner to scale it, drag a side edge to change its width; double-click resets, Ctrl Z undoes,
//! Esc or the hotkey finishes and saves.

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{Color32, CornerRadius, CursorIcon, Id, LayerId, Order, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use rh_core::i18n;
use rh_core::model::Game;
use rh_core::modules::{self, Placement};
use rh_core::settings::{ModuleConfig, MonsterLayout};
use ui_kit::Font;
use ui_kit::color::rgba;

use crate::state::Shared;

const PAPER: Color32 = Color32::from_rgb(0xf2, 0xf0, 0xe9);
const ORANGE: Color32 = Color32::from_rgb(0xff, 0x6b, 0x1a);

#[derive(Default)]
pub struct LayoutState {
    drag: Option<Drag>,
    undo: Vec<(String, ModuleConfig)>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Grip {
    Move,
    /// A corner, by which side of the module it is on (-1 or 1 on each axis)
    Corner(f32, f32),
    /// The left (-1) or right (1) edge
    Edge(f32),
}

struct Drag {
    id: String,
    grip: Grip,
    /// Module rect when the drag started, reference coordinates
    start: Rect,
    cfg: ModuleConfig,
}

fn top_layer() -> LayerId {
    LayerId::new(Order::Tooltip, Id::unique("layout-top"))
}

/// Dimming, and the snapping grid while a module is being moved.
pub fn backdrop(ctx: &egui::Context, screen: Rect, s: f32, st: &LayoutState) {
    let p = ctx.layer_painter(LayerId::new(Order::Background, Id::unique("layout-back")));
    p.rect_filled(screen, CornerRadius::ZERO, rgba(8, 8, 8, 0.55));
    if !st.drag.as_ref().is_some_and(|d| d.grip == Grip::Move) {
        return;
    }
    let fine = rgba(242, 240, 233, 0.035);
    let major = rgba(242, 240, 233, 0.09);
    let mut x = 0.0;
    while x * s < screen.width() {
        let c = if (x as i32) % 64 == 0 { major } else { fine };
        p.line_segment(
            [pos2(screen.min.x + x * s, screen.min.y), pos2(screen.min.x + x * s, screen.max.y)],
            Stroke::new(1.0, c),
        );
        x += 8.0;
    }
    let mut y = 0.0;
    while y * s < screen.height() {
        let c = if (y as i32) % 64 == 0 { major } else { fine };
        p.line_segment(
            [pos2(screen.min.x, screen.min.y + y * s), pos2(screen.max.x, screen.min.y + y * s)],
            Stroke::new(1.0, c),
        );
        y += 8.0;
    }
}

fn dashed_rect(p: &egui::Painter, r: Rect, color: Color32) {
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    for w in pts.windows(2) {
        p.extend(egui::Shape::dashed_line(&[w[0], w[1]], Stroke::new(1.0, color), 5.0, 4.0));
    }
}

/// Grab areas of a module outline on screen: corners, then side edges if the width can change.
fn grips(outline: Rect, wide: bool) -> Vec<(Grip, Rect)> {
    let k = 14.0;
    let mut out = Vec::new();
    for (gx, gy, p) in [
        (-1.0, -1.0, outline.left_top()),
        (1.0, -1.0, outline.right_top()),
        (-1.0, 1.0, outline.left_bottom()),
        (1.0, 1.0, outline.right_bottom()),
    ] {
        out.push((Grip::Corner(gx, gy), Rect::from_center_size(p, vec2(k, k))));
    }
    if wide && outline.height() > k * 2.0 {
        for (gx, x) in [(-1.0, outline.min.x), (1.0, outline.max.x)] {
            let r = Rect::from_min_max(pos2(x - 5.0, outline.min.y + k / 2.0), pos2(x + 5.0, outline.max.y - k / 2.0));
            out.push((Grip::Edge(gx), r));
        }
    }
    out
}

fn cursor(g: Grip) -> CursorIcon {
    match g {
        Grip::Move => CursorIcon::Grab,
        Grip::Corner(x, y) if x == y => CursorIcon::ResizeNwSe,
        Grip::Corner(..) => CursorIcon::ResizeNeSw,
        Grip::Edge(_) => CursorIcon::ResizeHorizontal,
    }
}

/// The new settings of a module being dragged, and the label shown while dragging.
fn dragged(d: &Drag, delta: Vec2, frame_w: f32, side: bool) -> (ModuleConfig, String) {
    let snap = |v: f32| (v / 8.0).round() * 8.0;
    let r = d.start;
    let mut c = d.cfg.clone();
    match d.grip {
        Grip::Move => {
            let x = snap(r.min.x + delta.x).clamp(0.0, (frame_w - r.width()).max(0.0));
            let y = snap(r.min.y + delta.y).clamp(0.0, (1080.0 - r.height()).max(0.0));
            c.placement = Placement::from_top_left(x, y, frame_w, (r.width(), r.height()));
            (c, format!("{}, {}", x as i32, y as i32))
        }
        Grip::Corner(gx, gy) => {
            // Average of how far each axis was pulled; the opposite corner stays put
            let k = ((r.width() + gx * delta.x) / r.width() + (r.height() + gy * delta.y) / r.height()) / 2.0;
            c.scale = ((d.cfg.scale * k).clamp(0.5, 2.0) * 100.0).round() / 100.0;
            let size = r.size() * (c.scale / d.cfg.scale);
            let x = if gx > 0.0 { r.min.x } else { r.max.x - size.x };
            let y = if gy > 0.0 { r.min.y } else { r.max.y - size.y };
            c.placement = Placement::from_top_left(x, y, frame_w, (size.x, size.y));
            let label = format!("{}%", (c.scale * 100.0).round());
            (c, label)
        }
        Grip::Edge(gx) => {
            let old = modules::width(&d.id, side, d.cfg.width);
            let w = modules::width(&d.id, side, Some((old + gx * delta.x / d.cfg.scale).round()));
            c.width = Some(w);
            let rw = r.width() + (w - old) * d.cfg.scale;
            let x = if gx > 0.0 { r.min.x } else { r.max.x - rw };
            c.placement = Placement::from_top_left(x, r.min.y, frame_w, (rw, r.height()));
            (c, format!("{}", w as i32))
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn interact(ui: &mut egui::Ui, shared: &Arc<Shared>, st: &mut LayoutState, rects: &BTreeMap<String, Rect>, screen: Rect, s: f32, frame_w: f32, game: Game) {
    let ctx = ui.ctx().clone();
    let painter = ctx.layer_painter(top_layer());
    let to_screen = |r: Rect| Rect::from_min_max(screen.min + r.min.to_vec2() * s, screen.min + r.max.to_vec2() * s);

    // Release the settings lock before looking up plugin module names: the plugin side writes settings while holding its own lock
    let (ids, side) = {
        let set = shared.settings();
        let p = set.profile(game);
        (p.modules.keys().cloned().collect::<Vec<_>>(), p.monster.layout == MonsterLayout::Side)
    };
    let names: BTreeMap<String, String> = ids
        .into_iter()
        .map(|id| {
            let n = crate::plugins::module_name(&id);
            (id, n)
        })
        .collect();

    let delta = ctx
        .input(|i| i.pointer.press_origin().zip(i.pointer.latest_pos()).map(|(a, b)| b - a))
        .unwrap_or_default()
        / s;
    let mut changed: Option<(String, ModuleConfig)> = None;
    for (id, r) in rects {
        let outline = to_screen(*r).expand(8.0);
        let cfg = shared.settings().profile(game).module(id).cloned().unwrap_or_default();
        let wide = modules::width_range(id, side).is_some();

        // The body first, then the grips on top of it, so the grips win where they overlap
        let mut targets = vec![(Grip::Move, outline)];
        targets.extend(grips(outline, wide));
        let mut hovered = None;
        let mut label = String::new();
        for (n, (grip, area)) in targets.into_iter().enumerate() {
            let resp = ui.interact(area, Id::unique(("layout", id, n)), Sense::click_and_drag());
            if resp.hovered() || resp.dragged() {
                hovered = Some(grip);
            }
            if resp.drag_started() {
                st.undo.push((id.clone(), cfg.clone()));
                st.drag = Some(Drag {
                    id: id.clone(),
                    grip,
                    start: *r,
                    cfg: cfg.clone(),
                });
            }
            if resp.dragged()
                && let Some(d) = st.drag.as_ref().filter(|d| &d.id == id && d.grip == grip)
            {
                let (c, l) = dragged(d, delta, frame_w, side);
                label = l;
                changed = Some((id.clone(), c));
            }
            if resp.drag_stopped() {
                st.drag = None;
            }
            if grip == Grip::Move && resp.double_clicked() {
                st.undo.push((id.clone(), cfg.clone()));
                let id2 = id.clone();
                shared.edit_settings(|set| set.profile_mut(game).reset_module(game, &id2));
            }
        }
        let dragging = st.drag.as_ref().filter(|d| &d.id == id).map(|d| d.grip);
        if let Some(g) = dragging.or(hovered) {
            ctx.set_cursor_icon(if dragging == Some(Grip::Move) { CursorIcon::Grabbing } else { cursor(g) });
        }

        let active = hovered.is_some() || dragging.is_some();
        if active {
            let color = if dragging.is_some() { ORANGE } else { PAPER };
            painter.rect_stroke(outline, CornerRadius::ZERO, Stroke::new(1.0, color), StrokeKind::Inside);
            for (g, area) in grips(outline, wide) {
                let hot = dragging == Some(g) || (dragging.is_none() && hovered == Some(g));
                let shape = match g {
                    Grip::Edge(_) => Rect::from_center_size(area.center(), vec2(4.0, 18.0)),
                    _ => Rect::from_center_size(area.center(), vec2(8.0, 8.0)),
                };
                painter.rect_filled(shape, CornerRadius::ZERO, if hot { ORANGE } else { PAPER });
            }
            let name = names.get(id).cloned().unwrap_or_else(|| id.clone());
            let text = if label.is_empty() { name } else { format!("{name}　{label}") };
            let g = painter.layout_no_wrap(text, Font::TextBold.id(12.0), Color32::from_rgb(0x11, 0x11, 0x11));
            let tr = Rect::from_min_size(outline.min + vec2(-1.0, -g.size().y - 10.0), g.size() + vec2(16.0, 8.0));
            painter.rect_filled(tr, CornerRadius::ZERO, if dragging.is_some() { ORANGE } else { PAPER });
            painter.galley(tr.min + vec2(8.0, 4.0), g, Color32::BLACK);
        } else {
            dashed_rect(&painter, outline, rgba(242, 240, 233, 0.6));
        }
    }
    if let Some((id, c)) = changed {
        shared.edit_settings(|set| {
            *set.profile_mut(game).module_mut(&id) = c;
        });
    }

    // Keyboard: Ctrl Z undo, Esc done
    let (undo, esc) = ctx.input(|i| (i.modifiers.ctrl && i.key_pressed(egui::Key::Z), i.key_pressed(egui::Key::Escape)));
    if undo {
        pop_undo(shared, st, game);
    }
    if esc {
        shared.act(crate::state::Action::SetLayout(false));
    }

    hints(&ctx, screen);
}

fn pop_undo(shared: &Shared, st: &mut LayoutState, game: Game) {
    if let Some((id, c)) = st.undo.pop() {
        shared.edit_settings(|set| *set.profile_mut(game).module_mut(&id) = c);
    }
}

/// Key reminders in the bottom-right corner.
fn hints(ctx: &egui::Context, screen: Rect) {
    let p = ctx.layer_painter(top_layer());
    let items = [
        (i18n::t("layout.key_double"), i18n::t("layout.reset")),
        ("Ctrl Z".to_string(), i18n::t("action.undo")),
        ("Esc".to_string(), i18n::t("action.done")),
    ];
    let font_k = Font::Mono.id(11.0);
    let font_t = Font::Text.id(13.0);
    let mut parts = Vec::new();
    let mut total = 0.0;
    for (k, t) in &items {
        let gk = p.layout_no_wrap(k.clone(), font_k.clone(), PAPER);
        let gt = p.layout_no_wrap(t.clone(), font_t.clone(), rgba(242, 240, 233, 0.7));
        total += gk.size().x + 12.0 + 6.0 + gt.size().x + 22.0;
        parts.push((gk, gt));
    }
    let mut x = screen.max.x - total - 12.0;
    let y = screen.max.y - 40.0;
    for (gk, gt) in parts {
        let kr = Rect::from_min_size(pos2(x, y), gk.size() + vec2(12.0, 4.0));
        p.rect_stroke(kr, CornerRadius::ZERO, Stroke::new(1.0, rgba(242, 240, 233, 0.4)), StrokeKind::Inside);
        let w = gk.size().x;
        p.galley(kr.min + vec2(6.0, 2.0), gk, PAPER);
        x += w + 12.0 + 6.0;
        let tw = gt.size().x;
        p.galley(pos2(x, y + 1.0), gt, PAPER);
        x += tw + 22.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drag(id: &str, grip: Grip) -> Drag {
        Drag {
            id: id.into(),
            grip,
            start: Rect::from_min_size(pos2(100.0, 600.0), vec2(270.0, 120.0)),
            cfg: ModuleConfig::default(),
        }
    }

    #[test]
    fn corner_scales_around_the_opposite_corner() {
        let d = drag(modules::DAMAGE, Grip::Corner(-1.0, -1.0));
        let (c, label) = dragged(&d, vec2(-27.0, -12.0), 1920.0, false);
        assert_eq!(c.scale, 1.1);
        assert_eq!(label, "110%");
        let size = (270.0 * 1.1, 120.0 * 1.1);
        let (x, y) = c.placement.to_top_left(1920.0, size);
        assert!((x + size.0 - 370.0).abs() < 0.01 && (y + size.1 - 720.0).abs() < 0.01);
    }

    #[test]
    fn edge_changes_width_within_range() {
        let d = drag(modules::DAMAGE, Grip::Edge(1.0));
        let (c, _) = dragged(&d, vec2(50.0, 0.0), 1920.0, false);
        assert_eq!(c.width, Some(320.0));
        let (c, _) = dragged(&d, vec2(-500.0, 0.0), 1920.0, false);
        assert_eq!(c.width, Some(200.0));
        // Left edge: the right side stays put
        let d = drag(modules::DAMAGE, Grip::Edge(-1.0));
        let (c, _) = dragged(&d, vec2(-30.0, 0.0), 1920.0, false);
        let (x, _) = c.placement.to_top_left(1920.0, (300.0, 120.0));
        assert_eq!(x, 70.0);
    }

    #[test]
    fn move_snaps_to_grid() {
        let d = drag(modules::DAMAGE, Grip::Move);
        let (c, label) = dragged(&d, vec2(13.0, -3.0), 1920.0, false);
        assert_eq!(label, "112, 600");
        assert_eq!(c.placement.to_top_left(1920.0, (270.0, 120.0)), (112.0, 600.0));
    }
}
