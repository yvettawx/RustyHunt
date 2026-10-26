//! 布局模式：overlay 接收鼠标，游戏画面变暗，显示网格和原生 HUD 区域；
//! 拖动模块按 8px 吸附，滚轮缩放，双击复位，Ctrl Z 撤销，Esc 或热键退出并保存。

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{Align2, Color32, CornerRadius, Id, LayerId, Order, Pos2, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use rh_core::i18n;
use rh_core::model::Game;
use rh_core::modules::{self, Placement};
use rh_core::settings::ModuleConfig;
use ui_kit::Font;
use ui_kit::color::rgba;

use crate::state::Shared;

const PAPER: Color32 = Color32::from_rgb(0xf2, 0xf0, 0xe9);
const ORANGE: Color32 = Color32::from_rgb(0xff, 0x6b, 0x1a);

pub struct LayoutState {
    drag: Option<Drag>,
    undo: Vec<(String, ModuleConfig)>,
    pub grid: bool,
    pub zones: bool,
}

impl Default for LayoutState {
    fn default() -> Self {
        LayoutState {
            drag: None,
            undo: Vec::new(),
            grid: true,
            zones: true,
        }
    }
}

struct Drag {
    id: String,
    start: Pos2,
    size: (f32, f32),
}

fn top_layer() -> LayerId {
    LayerId::new(Order::Tooltip, Id::unique("layout-top"))
}

/// 变暗、网格、原生 HUD 区域，画在模块下面。
pub fn backdrop(ctx: &egui::Context, screen: Rect, s: f32, st: &LayoutState, game: Game) {
    let p = ctx.layer_painter(LayerId::new(Order::Background, Id::unique("layout-back")));
    p.rect_filled(screen, CornerRadius::ZERO, rgba(8, 8, 8, 0.55));
    if st.grid {
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
    let _ = game;
}

fn zones(game: Game, frame_w: f32) -> Vec<(Rect, String)> {
    // 原生 HUD 的坐标按 1920 宽量的，宽屏时右半边的区域跟着右边走
    modules::native_zones(game)
        .iter()
        .map(|&(x, y, w, h, key)| {
            let x = if x > 960.0 { x + (frame_w - 1920.0) } else { x };
            (Rect::from_min_size(pos2(x, y), vec2(w, h)), i18n::t(key))
        })
        .collect()
}

fn dashed_rect(p: &egui::Painter, r: Rect, color: Color32) {
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    for w in pts.windows(2) {
        p.extend(egui::Shape::dashed_line(&[w[0], w[1]], Stroke::new(1.0, color), 5.0, 4.0));
    }
}

#[allow(clippy::too_many_arguments)]
pub fn interact(ui: &mut egui::Ui, shared: &Arc<Shared>, st: &mut LayoutState, rects: &BTreeMap<String, Rect>, screen: Rect, s: f32, frame_w: f32, game: Game) {
    let ctx = ui.ctx().clone();
    let painter = ctx.layer_painter(top_layer());
    let to_screen = |r: Rect| Rect::from_min_max(screen.min + r.min.to_vec2() * s, screen.min + r.max.to_vec2() * s);
    let zone_list = zones(game, frame_w);

    // 原生 HUD 区域
    let mut zone_hit = vec![false; zone_list.len()];
    let mut module_hit = BTreeMap::new();
    for (id, r) in rects {
        for (i, (z, _)) in zone_list.iter().enumerate() {
            if r.intersects(*z) {
                zone_hit[i] = true;
                module_hit.insert(id.clone(), true);
            }
        }
    }
    if st.zones {
        for ((z, name), hit) in zone_list.iter().zip(&zone_hit) {
            let r = to_screen(*z);
            let c = if *hit { ORANGE } else { rgba(242, 240, 233, 0.35) };
            dashed_rect(&painter, r, c);
            painter.text(
                r.min + vec2(6.0, 4.0),
                Align2::LEFT_TOP,
                name,
                Font::Text.id(11.0 * s.max(0.7)),
                if *hit { ORANGE } else { rgba(242, 240, 233, 0.5) },
            );
        }
    }

    // 先放开设置锁再查插件模块的名字：插件那边持有自己的锁时会写设置
    let ids: Vec<String> = shared.settings().profile(game).modules.keys().cloned().collect();
    let names: BTreeMap<String, String> = ids
        .into_iter()
        .map(|id| {
            let n = crate::plugins::module_name(&id);
            (id, n)
        })
        .collect();

    let mut changed: Option<(String, ModuleConfig)> = None;
    for (id, r) in rects {
        let sr = to_screen(*r);
        let resp = ui.interact(sr.expand(8.0), Id::unique(("layout", id)), Sense::click_and_drag());
        let cfg = shared.settings().profile(game).module(id).cloned().unwrap_or_default();

        if resp.drag_started() {
            st.undo.push((id.clone(), cfg.clone()));
            st.drag = Some(Drag {
                id: id.clone(),
                start: r.min,
                size: (r.width(), r.height()),
            });
        }
        let mut label = format!("{}%", (cfg.scale * 100.0).round());
        if resp.dragged()
            && let Some(d) = st.drag.as_ref().filter(|d| &d.id == id)
        {
            let total = ctx
                .input(|i| i.pointer.press_origin().zip(i.pointer.latest_pos()).map(|(a, b)| b - a))
                .unwrap_or_default();
            let snap = |v: f32| (v / 8.0).round() * 8.0;
            let x = snap(d.start.x + total.x / s).clamp(0.0, (frame_w - d.size.0).max(0.0));
            let y = snap(d.start.y + total.y / s).clamp(0.0, (1080.0 - d.size.1).max(0.0));
            let mut c = cfg.clone();
            c.placement = Placement::from_top_left(x, y, frame_w, d.size);
            label = format!("{}, {}", x as i32, y as i32);
            changed = Some((id.clone(), c));
        }
        if resp.drag_stopped() {
            st.drag = None;
        }
        if resp.hovered() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.5 {
                st.undo.push((id.clone(), cfg.clone()));
                let mut c = cfg.clone();
                c.scale = ((c.scale + if scroll > 0.0 { 0.05 } else { -0.05 }) * 100.0).round() / 100.0;
                c.scale = c.scale.clamp(0.5, 2.0);
                changed = Some((id.clone(), c));
            }
        }
        if resp.double_clicked() {
            st.undo.push((id.clone(), cfg.clone()));
            let id2 = id.clone();
            shared.edit_settings(|set| set.profile_mut(game).reset_module(game, &id2));
        }

        let dragging = st.drag.as_ref().is_some_and(|d| &d.id == id);
        let hit = module_hit.contains_key(id);
        let color = if dragging || hit {
            ORANGE
        } else if resp.hovered() {
            PAPER
        } else {
            rgba(242, 240, 233, 0.6)
        };
        let outline = sr.expand(8.0);
        if resp.hovered() || dragging {
            painter.rect_stroke(outline, CornerRadius::ZERO, Stroke::new(1.0, color), StrokeKind::Inside);
        } else {
            dashed_rect(&painter, outline, color);
        }
        if resp.hovered() || dragging || hit {
            let name = names.get(id).cloned().unwrap_or_else(|| id.clone());
            let text = format!("{name}　{label}");
            let g = painter.layout_no_wrap(text, Font::TextBold.id(12.0), Color32::from_rgb(0x11, 0x11, 0x11));
            let tr = Rect::from_min_size(outline.min + vec2(-1.0, -g.size().y - 10.0), g.size() + vec2(16.0, 8.0));
            painter.rect_filled(tr, CornerRadius::ZERO, if hit || dragging { ORANGE } else { PAPER });
            painter.galley(tr.min + vec2(8.0, 4.0), g, Color32::BLACK);
        }
    }
    if let Some((id, c)) = changed {
        shared.edit_settings(|set| {
            *set.profile_mut(game).module_mut(&id) = c;
        });
    }

    // 键盘：Ctrl Z 撤销，Esc 完成
    let (undo, esc) = ctx.input(|i| (i.modifiers.ctrl && i.key_pressed(egui::Key::Z), i.key_pressed(egui::Key::Escape)));
    if undo {
        pop_undo(shared, st, game);
    }
    if esc {
        shared.act(crate::state::Action::SetLayout(false));
    }

    toolbar(&ctx, shared, st, screen, game);
    hints(&ctx, screen);
}

fn pop_undo(shared: &Shared, st: &mut LayoutState, game: Game) {
    if let Some((id, c)) = st.undo.pop() {
        shared.edit_settings(|set| *set.profile_mut(game).module_mut(&id) = c);
    }
}

fn bar_button(ui: &mut egui::Ui, label: &str, on: bool, primary: bool) -> egui::Response {
    let font = Font::Text.id(14.0);
    let g = ui.painter().layout_no_wrap(label.to_string(), font, PAPER);
    let w = g.size().x + 32.0 + if on { 14.0 } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
    let hovered = resp.hovered();
    let (bg, fg) = if primary {
        (if hovered { ORANGE } else { PAPER }, Color32::from_rgb(0x11, 0x11, 0x11))
    } else {
        (
            if hovered { Color32::from_rgb(0x1f, 0x1f, 0x1e) } else { Color32::TRANSPARENT },
            if on || hovered { PAPER } else { Color32::from_rgb(0xbd, 0xbb, 0xb4) },
        )
    };
    ui.painter().rect_filled(rect, CornerRadius::ZERO, bg);
    let mut x = rect.min.x + 16.0;
    if on {
        let c = pos2(x + 3.0, rect.center().y);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![c + vec2(0.0, -3.5), c + vec2(3.5, 0.0), c + vec2(0.0, 3.5), c + vec2(-3.5, 0.0)],
            ORANGE,
            Stroke::NONE,
        ));
        x += 14.0;
    }
    let g = ui.painter().layout_no_wrap(label.to_string(), Font::Text.id(14.0), fg);
    ui.painter().galley(pos2(x, rect.center().y - g.size().y / 2.0), g, fg);
    resp
}

fn toolbar(ctx: &egui::Context, shared: &Arc<Shared>, st: &mut LayoutState, screen: Rect, game: Game) {
    egui::Area::new(Id::unique("layout-toolbar"))
        .order(Order::Tooltip)
        .fixed_pos(pos2(screen.center().x, screen.min.y + 22.0))
        .pivot(Align2::CENTER_TOP)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(Color32::from_rgb(0x11, 0x11, 0x11))
                .shadow(egui::Shadow {
                    offset: [0, 8],
                    blur: 30,
                    spread: 0,
                    color: rgba(0, 0, 0, 0.4),
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                        let (r, _) = ui.allocate_exact_size(vec2(0.0, 40.0), Sense::hover());
                        let title = i18n::t("layout.title");
                        let g = ui.painter().layout_no_wrap(title, Font::TextBold.id(14.0), PAPER);
                        let (tr, _) = ui.allocate_exact_size(vec2(g.size().x + 36.0, 40.0), Sense::hover());
                        ui.painter().galley(pos2(tr.min.x + 18.0, tr.center().y - g.size().y / 2.0), g, PAPER);
                        ui.painter()
                            .line_segment([tr.right_top(), tr.right_bottom()], Stroke::new(1.0, Color32::from_rgb(0x33, 0x33, 0x33)));
                        let _ = r;

                        // 方案
                        let (profiles, active) = {
                            let set = shared.settings();
                            let l = set.layouts(game);
                            (l.profiles.keys().cloned().collect::<Vec<_>>(), l.active.clone())
                        };
                        let mut pick = active.clone();
                        ui.add_space(8.0);
                        ui.visuals_mut().override_text_color = Some(PAPER);
                        egui::ComboBox::from_id_salt("layout-profile")
                            .selected_text(egui::RichText::new(&active).color(PAPER).size(14.0))
                            .show_ui(ui, |ui| {
                                for p in &profiles {
                                    ui.selectable_value(&mut pick, p.clone(), p);
                                }
                            });
                        ui.add_space(8.0);
                        if pick != active {
                            shared.edit_settings(|s| s.layouts_mut(game).active = pick.clone());
                        }
                        if bar_button(ui, &i18n::t("action.undo"), false, false).clicked() {
                            pop_undo(shared, st, game);
                        }
                        if bar_button(ui, &i18n::t("layout.reset_all"), false, false).clicked() {
                            let all = shared.settings().profile(game).clone();
                            for (id, c) in all.modules {
                                st.undo.push((id, c));
                            }
                            shared.edit_settings(|s| {
                                let p = s.profile_mut(game);
                                let ids: Vec<String> = p.modules.keys().cloned().collect();
                                for id in ids {
                                    p.reset_module(game, &id);
                                }
                            });
                        }
                        if bar_button(ui, &i18n::t("layout.grid"), st.grid, false).clicked() {
                            st.grid = !st.grid;
                        }
                        if bar_button(ui, &i18n::t("layout.zones"), st.zones, false).clicked() {
                            st.zones = !st.zones;
                        }
                        if bar_button(ui, &i18n::t("action.done"), false, true).clicked() {
                            shared.act(crate::state::Action::SetLayout(false));
                        }
                    });
                });
        });
}

fn hints(ctx: &egui::Context, screen: Rect) {
    let p = ctx.layer_painter(top_layer());
    let items = [
        (i18n::t("layout.key_drag"), i18n::t("layout.move")),
        (i18n::t("layout.key_wheel"), i18n::t("layout.scale")),
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
    let mut x = screen.center().x - total / 2.0;
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
