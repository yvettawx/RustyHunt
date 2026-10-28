//! Main window: 1280x800, custom title bar, seven pages in the sidebar.

mod home;
mod logs;
mod modules;
mod onboarding;
mod pages_plugins;
mod pages_themes;
mod records;
mod settings;
pub mod style;
pub mod widgets;

use std::sync::Arc;

use egui::{Align2, Color32, CornerRadius, Id, Rect, Sense, Stroke, Ui, ViewportBuilder, ViewportId, pos2, vec2};
use rh_core::i18n;
use ui_kit::Font;

use crate::notify::Kind;
use crate::state::{Action, Connection, Shared};
use style::Pal;
use widgets::{Btn, button, link, switch, text_at};

pub const TITLE: &str = "RustyHunt";

/// The main window is the root viewport of its own egui context.
pub fn main_viewport() -> ViewportId {
    ViewportId::ROOT
}

/// Icon for the tray and windows: an orange diamond in a black square.
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let mut v = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let inset = size as f32 * 0.06;
            let inside = fx > inset && fy > inset && fx < size as f32 - inset && fy < size as f32 - inset;
            let d = (fx - c).abs() + (fy - c).abs();
            if d < size as f32 * 0.22 {
                v[i..i + 4].copy_from_slice(&[0xff, 0x6b, 0x1a, 0xff]);
            } else if inside {
                v[i..i + 4].copy_from_slice(&[0x14, 0x14, 0x14, 0xff]);
            }
        }
    }
    v
}

pub fn icon() -> egui::IconData {
    egui::IconData {
        rgba: icon_rgba(64),
        width: 64,
        height: 64,
    }
}

pub fn builder(shared: &Shared) -> ViewportBuilder {
    let mut b = ViewportBuilder::default()
        .with_title(TITLE)
        .with_decorations(false)
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([980.0, 640.0])
        .with_resizable(true)
        .with_icon(Arc::new(icon()))
        .with_taskbar(true);
    if let Some([x, y, w, h]) = shared.settings().window {
        b = b.with_position([x, y]).with_inner_size([w, h]);
    }
    b
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Modules,
    Records,
    Themes,
    Plugins,
    Settings,
    Logs,
}

impl Page {
    pub fn from_key(k: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|p| p.key().ends_with(k))
    }

    const ALL: [Page; 7] = [
        Page::Home,
        Page::Modules,
        Page::Records,
        Page::Themes,
        Page::Plugins,
        Page::Settings,
        Page::Logs,
    ];

    fn key(self) -> &'static str {
        match self {
            Page::Home => "nav.home",
            Page::Modules => "nav.modules",
            Page::Records => "nav.records",
            Page::Themes => "nav.themes",
            Page::Plugins => "nav.plugins",
            Page::Settings => "nav.settings",
            Page::Logs => "nav.logs",
        }
    }
}

/// Action to run once the dialog is confirmed
pub type OnYes = Box<dyn FnOnce(&Arc<Shared>) + Send>;

/// Dialog: confirmation before turning something on (injection, plugin permissions and so on).
pub struct Modal {
    pub title: String,
    pub lines: Vec<String>,
    pub yes: String,
    pub on_yes: OnYes,
}

pub struct MainWindow {
    pub shared: Arc<Shared>,
    pub page: Page,
    pub closed: bool,
    pub modal: Option<Modal>,
    notif_open: bool,
    update_open: bool,
    home: home::State,
    modules: modules::State,
    records: records::State,
    themes: pages_themes::State,
    plugins: pages_plugins::State,
    settings: settings::State,
    logs: logs::State,
    onboarding: Option<onboarding::State>,
    started: bool,
    maximized: bool,
}

impl MainWindow {
    /// For screenshots: switch to a tab within the page
    pub fn set_tab(&mut self, tab: usize) {
        match self.page {
            Page::Records => self.records.tab = tab,
            Page::Settings => self.settings.tab = tab,
            _ => {}
        }
    }

    pub fn new(shared: Arc<Shared>) -> MainWindow {
        let first = !shared.settings().first_run_done;
        MainWindow {
            page: Page::Home,
            closed: false,
            modal: None,
            notif_open: false,
            update_open: false,
            home: Default::default(),
            modules: Default::default(),
            records: Default::default(),
            themes: Default::default(),
            plugins: Default::default(),
            settings: Default::default(),
            logs: Default::default(),
            onboarding: first.then(|| onboarding::State::new(&shared)),
            started: false,
            maximized: false,
            shared,
        }
    }

    pub fn onboarding_step(&mut self, step: usize) {
        let shared = self.shared.clone();
        if let Some(o) = self.onboarding.as_mut() {
            o.set_step(&shared, step);
        }
    }

    pub fn open_onboarding(&mut self) {
        self.onboarding = Some(onboarding::State::new(&self.shared));
    }

    pub fn ask(&mut self, title: String, lines: Vec<String>, yes: String, on_yes: impl FnOnce(&Arc<Shared>) + Send + 'static) {
        self.modal = Some(Modal {
            title,
            lines,
            yes,
            on_yes: Box::new(on_yes),
        });
    }

    pub fn on_close(&mut self, shared: &Shared) {
        // Remember the window position
        if let Some(r) = self.shared.ctx.get().and_then(|c| c.input_for(main_viewport(), |i| i.viewport().outer_rect)) {
            let inner = self
                .shared
                .ctx
                .get()
                .and_then(|c| c.input_for(main_viewport(), |i| i.viewport().inner_rect))
                .unwrap_or(r);
            shared.edit_settings(|s| s.window = Some([r.min.x, r.min.y, inner.width(), inner.height()]));
        }
        if shared.settings().close_to_tray {
            self.closed = true;
            shared.repaint();
        } else {
            shared.act(Action::Quit);
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if !self.started {
            self.started = true;
            crate::update::confirm_started();
        }
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(false));
        self.shared.main_focused.store(focused, std::sync::atomic::Ordering::Relaxed);
        let theme = self.shared.theme();
        let pal = Pal::of(&theme);
        let full = ctx.content_rect();
        ui.painter().rect_filled(full, CornerRadius::ZERO, pal.paper);

        resize_edges(ui, full);

        let title = Rect::from_min_size(full.min, vec2(full.width(), 44.0));
        self.title_bar(ui, &pal, title);

        let side = Rect::from_min_max(pos2(full.min.x, title.max.y), pos2(full.min.x + 196.0, full.max.y));
        self.sidebar(ui, &pal, side);

        let content = Rect::from_min_max(pos2(side.max.x + 1.0, title.max.y), full.max);
        let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(content).id_salt("content"));
        let page = self.page;
        egui::ScrollArea::vertical()
            .id_salt(("page", page as u8))
            .auto_shrink([false, false])
            .show(&mut cui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: 52,
                        right: 56,
                        top: 28,
                        bottom: 32,
                    })
                    .show(ui, |ui| {
                        ui.set_width(content.width() - 108.0 - 8.0);
                        match page {
                            Page::Home => home::ui(self, ui, &pal),
                            Page::Modules => modules::ui(self, ui, &pal),
                            Page::Records => records::ui(self, ui, &pal),
                            Page::Themes => pages_themes::ui(self, ui, &pal),
                            Page::Plugins => pages_plugins::ui(self, ui, &pal),
                            Page::Settings => settings::ui(self, ui, &pal),
                            Page::Logs => logs::ui(self, ui, &pal),
                        }
                    });
            });

        if let Some(mut ob) = self.onboarding.take() {
            let keep = ob.ui(self, ui, &pal, Rect::from_min_max(pos2(full.min.x, title.max.y), full.max));
            if keep {
                self.onboarding = Some(ob);
            }
        }
        self.notifications(ui, &pal, full);
        self.toast(ui, &pal, full);
        if self.update_open {
            self.update_panel(ui, &pal, Rect::from_min_max(pos2(full.max.x - 460.0, title.max.y), full.max));
        }
        self.modal(ui, &pal, Rect::from_min_max(pos2(full.min.x, title.max.y), full.max));

        // Connection status, logs and the like change; refreshing the main window twice a second is enough.
        // While the game runs, only when the window has focus (see Shared::main_window_live).
        if self.shared.main_window_live() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }

    fn title_bar(&mut self, ui: &mut Ui, pal: &Pal, r: Rect) {
        let drag = ui.interact(r, Id::unique("titlebar"), Sense::click_and_drag());
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            self.maximized = !self.maximized;
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(self.maximized));
        }
        let p = ui.painter();
        let mark = Rect::from_min_size(pos2(r.min.x + 22.0, r.center().y - 7.0), vec2(14.0, 14.0));
        p.rect_filled(mark, CornerRadius::ZERO, pal.block);
        widgets::diamond(ui, mark.center(), 5.0, pal.accent);
        let g = ui.painter().layout_job({
            let mut job = egui::text::LayoutJob::default();
            job.append(
                "RustyHunt",
                0.0,
                egui::TextFormat {
                    font_id: Font::Num.id(widgets::T_SMALL),
                    color: pal.ink,
                    extra_letter_spacing: 2.2,
                    ..Default::default()
                },
            );
            job
        });
        ui.painter().galley(pos2(mark.max.x + 10.0, r.center().y - g.size().y / 2.0), g, pal.ink);

        // Right side: connection status, bell, minimize, maximize, close
        let mut x = r.max.x;
        let btn = |ui: &mut Ui, x: &mut f32, id: &str| -> (Rect, egui::Response) {
            *x -= 44.0;
            let br = Rect::from_min_size(pos2(*x, r.min.y), vec2(44.0, 44.0));
            let resp = ui.interact(br, Id::unique(("tb", id)), Sense::click());
            (br, resp)
        };
        let (cr, close) = btn(ui, &mut x, "close");
        let (mr, max) = btn(ui, &mut x, "max");
        let (nr, min) = btn(ui, &mut x, "min");
        let (br, bell) = btn(ui, &mut x, "bell");
        let p = ui.painter();
        for (rect, resp, danger) in [(&cr, &close, true), (&mr, &max, false), (&nr, &min, false), (&br, &bell, false)] {
            if resp.hovered() {
                p.rect_filled(
                    *rect,
                    CornerRadius::ZERO,
                    if danger { Color32::from_rgb(0xc4, 0x37, 0x2b) } else { pal.hover() },
                );
            }
        }
        let ic = |_rect: &Rect, hov: bool, white: bool| {
            if hov && white {
                Color32::WHITE
            } else if hov {
                pal.ink
            } else {
                pal.mute
            }
        };
        let c = cr.center();
        let s = Stroke::new(1.2, ic(&cr, close.hovered(), true));
        p.line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], s);
        p.line_segment([c + vec2(5.0, -5.0), c + vec2(-5.0, 5.0)], s);
        p.rect_stroke(
            Rect::from_center_size(mr.center(), vec2(9.0, 9.0)),
            CornerRadius::ZERO,
            Stroke::new(1.2, ic(&mr, max.hovered(), false)),
            egui::StrokeKind::Inside,
        );
        p.line_segment(
            [nr.center() + vec2(-5.0, 0.0), nr.center() + vec2(5.0, 0.0)],
            Stroke::new(1.2, ic(&nr, min.hovered(), false)),
        );
        // Bell
        let b = br.center();
        let bs = Stroke::new(1.1, ic(&br, bell.hovered(), false));
        let pts = [
            b + vec2(-4.0, 3.0),
            b + vec2(-4.0, -1.0),
            b + vec2(-3.0, -4.0),
            b + vec2(0.0, -5.5),
            b + vec2(3.0, -4.0),
            b + vec2(4.0, -1.0),
            b + vec2(4.0, 3.0),
            b + vec2(5.5, 4.0),
            b + vec2(-5.5, 4.0),
            b + vec2(-4.0, 3.0),
        ];
        p.add(egui::Shape::line(pts.to_vec(), bs));
        p.line_segment([b + vec2(-1.5, 6.0), b + vec2(1.5, 6.0)], bs);
        if self.shared.notifications.lock().unwrap().unread() > 0 {
            p.circle_filled(br.center() + vec2(7.0, -7.0), 3.0, pal.accent);
        }
        if close.clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if max.clicked() {
            self.maximized = !self.maximized;
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(self.maximized));
        }
        if min.clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        if bell.clicked() {
            self.notif_open = !self.notif_open;
        }

        let live = self.shared.live();
        let (dot, text) = match &live.connection {
            Connection::Connected => (pal.ok, format!("{} {}", i18n::t("conn.connected"), i18n::t("game.world"))),
            Connection::Demo => (pal.hue, i18n::t("conn.demo")),
            Connection::MapMismatch { .. } => (pal.accent, i18n::t("conn.mismatch")),
            Connection::Failed(_) => (pal.faint, i18n::t("conn.failed")),
            Connection::Searching => (pal.faint, i18n::t("conn.waiting")),
        };
        drop(live);
        let tr = text_at(
            ui,
            pos2(br.min.x - 16.0, r.center().y),
            Align2::RIGHT_CENTER,
            &text,
            Font::Text,
            widgets::T_SMALL,
            pal.mute,
        );
        ui.painter().circle_filled(pos2(tr.min.x - 11.0, r.center().y), 3.0, dot);
    }

    fn sidebar(&mut self, ui: &mut Ui, pal: &Pal, r: Rect) {
        ui.painter().rect_filled(r, CornerRadius::ZERO, pal.nav);
        ui.painter()
            .line_segment([pos2(r.max.x + 0.5, r.min.y), pos2(r.max.x + 0.5, r.max.y)], Stroke::new(1.0, pal.line));
        let mut y = r.min.y + 24.0;
        let new_records = false;
        for (i, page) in Page::ALL.iter().enumerate() {
            let row = Rect::from_min_size(pos2(r.min.x, y), vec2(r.width(), 40.0));
            let resp = ui.interact(row, Id::unique(("nav", i)), Sense::click());
            let on = *page == self.page;
            // The current page is a block in the interaction colour; it slides in when picked
            let sel = widgets::ease(ui, resp.id.with("on"), on);
            let hov = widgets::ease(ui, resp.id.with("hover"), resp.hovered() && !on);
            let block = Rect::from_min_max(pos2(r.min.x + 12.0, row.min.y + 2.0), pos2(r.max.x - 12.0, row.max.y - 2.0));
            ui.painter().rect_filled(block, CornerRadius::ZERO, pal.tint(hov));
            if sel > 0.0 {
                let b = Rect::from_min_max(block.min, pos2(block.min.x + block.width() * sel, block.max.y));
                ui.painter().rect_filled(b, CornerRadius::ZERO, pal.hue);
            }
            let num_color = if sel > 0.5 { pal.onhue.gamma_multiply(0.6) } else { pal.faint };
            text_at(
                ui,
                pos2(r.min.x + 24.0, row.center().y + 1.0),
                Align2::LEFT_CENTER,
                &format!("{:02}", i + 1),
                Font::Num,
                widgets::T_MICRO,
                num_color,
            );
            let color = if sel > 0.5 { pal.onhue } else { pal.mute.lerp_to_gamma(pal.ink, hov) };
            let lr = text_at(
                ui,
                pos2(r.min.x + 52.0, row.center().y),
                Align2::LEFT_CENTER,
                &i18n::t(page.key()),
                if on { Font::TextBold } else { Font::Text },
                widgets::T_BODY,
                color,
            );
            if *page == Page::Records && new_records {
                ui.painter().circle_filled(pos2(lr.max.x + 9.0, row.center().y), 2.5, pal.accent);
            }
            if resp.clicked() {
                self.page = *page;
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            y += 40.0;
        }

        // Bottom: overlay toggle, layout mode toggle, version
        let mut by = r.max.y - 22.0 - 14.0;
        text_at(
            ui,
            pos2(r.min.x + 24.0, by + 7.0),
            Align2::LEFT_CENTER,
            rh_core::VERSION,
            Font::NumBold,
            widgets::T_MICRO,
            pal.faint,
        );
        by -= 14.0 + 34.0;
        let rows = [(i18n::t("action.layout"), 1), (i18n::t("label.overlay"), 0)];
        for (label, which) in rows {
            let row = Rect::from_min_size(pos2(r.min.x + 24.0, by), vec2(r.width() - 48.0, 32.0));
            text_at(
                ui,
                pos2(row.min.x, row.center().y),
                Align2::LEFT_CENTER,
                &label,
                Font::Text,
                widgets::T_BODY,
                pal.mute,
            );
            let sr = Rect::from_min_size(pos2(row.max.x - 28.0, row.center().y - 7.0), vec2(28.0, 14.0));
            let mut on = if which == 0 {
                self.shared.settings().overlay_visible
            } else {
                self.shared.layout_mode()
            };
            let mut changed = false;
            ui.scope_builder(egui::UiBuilder::new().max_rect(sr), |ui| {
                changed = switch(ui, pal, &mut on).changed();
            });
            if changed {
                if which == 0 {
                    self.shared.edit_settings(|s| s.overlay_visible = on);
                } else {
                    self.shared.act(Action::SetLayout(on));
                }
            }
            by -= 30.0;
        }
    }

    fn notifications(&mut self, ui: &mut Ui, pal: &Pal, full: Rect) {
        if !self.notif_open {
            return;
        }
        let panel = Rect::from_min_size(pos2(full.max.x - 132.0 - 340.0, full.min.y + 44.0), vec2(340.0, 420.0));
        let mut close = false;
        egui::Area::new(Id::unique("notif-panel"))
            .order(egui::Order::Foreground)
            .fixed_pos(panel.min)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(pal.paper)
                    .stroke(Stroke::new(1.0, pal.line))
                    .shadow(egui::Shadow {
                        offset: [0, 16],
                        blur: 50,
                        spread: 0,
                        color: Color32::from_black_alpha(70),
                    })
                    .show(ui, |ui| {
                        ui.set_width(340.0);
                        egui::Frame::new().inner_margin(egui::Margin::symmetric(18, 12)).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                widgets::label(ui, i18n::t("notify.title"), Font::TextBold, widgets::T_BODY, pal.ink);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if link(ui, pal, &i18n::t("notify.mark_read")).clicked() {
                                        self.shared.notifications.lock().unwrap().mark_all_read();
                                    }
                                });
                            });
                        });
                        widgets::hline(ui, pal);
                        let items = self.shared.notifications.lock().unwrap().items.clone();
                        egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                            if items.is_empty() {
                                egui::Frame::new().inner_margin(18).show(ui, |ui| {
                                    widgets::label_mute(ui, pal, &i18n::t("notify.empty"), 13.0);
                                });
                            }
                            for n in items {
                                egui::Frame::new().inner_margin(egui::Margin::symmetric(18, 12)).show(ui, |ui| {
                                    ui.set_width(304.0);
                                    ui.horizontal(|ui| {
                                        if !n.read {
                                            let (r, _) = ui.allocate_exact_size(vec2(6.0, 14.0), Sense::hover());
                                            ui.painter().circle_filled(r.center(), 2.5, pal.accent);
                                        }
                                        widgets::label(ui, &n.title, Font::TextBold, widgets::T_BODY, pal.ink);
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            let t = chrono::DateTime::from_timestamp(n.time, 0)
                                                .map(|d| d.with_timezone(&chrono::Local).format("%m-%d %H:%M").to_string())
                                                .unwrap_or_default();
                                            widgets::label(ui, t, Font::NumBold, widgets::T_MICRO, pal.mute);
                                        });
                                    });
                                    if !n.body.is_empty() {
                                        widgets::label_mute(ui, pal, &n.body, 12.0);
                                    }
                                    if let Some(a) = self.notification_action(&n.kind)
                                        && link(ui, pal, &a.0).clicked()
                                    {
                                        (a.1)(self);
                                        close = true;
                                    }
                                });
                                widgets::hline(ui, pal);
                            }
                        });
                    });
            });
        // Click outside the panel to close it
        if ui.ctx().input(|i| i.pointer.any_click())
            && let Some(p) = ui.ctx().pointer_interact_pos()
            && !panel.contains(p)
            && !Rect::from_min_size(pos2(full.max.x - 176.0, full.min.y), vec2(44.0, 44.0)).contains(p)
        {
            close = true;
        }
        if close {
            self.notif_open = false;
            self.shared.notifications.lock().unwrap().mark_all_read();
        }
    }

    /// A button on a notification: text and action.
    #[allow(clippy::type_complexity)]
    fn notification_action(&self, kind: &Kind) -> Option<(String, Box<dyn FnOnce(&mut MainWindow)>)> {
        match kind {
            Kind::Update { .. } => Some((i18n::t("action.view"), Box::new(|w: &mut MainWindow| w.update_open = true))),
            Kind::MapMismatch { .. } => Some((
                i18n::t("action.update_maps"),
                Box::new(|w: &mut MainWindow| {
                    let s = w.shared.clone();
                    std::thread::spawn(move || crate::update::check(&s, false));
                }),
            )),
            Kind::Crash { path } => {
                let p = std::path::PathBuf::from(path);
                Some((i18n::t("action.open_folder"), Box::new(move |_| crate::win::open(p.parent().unwrap_or(&p)))))
            }
            Kind::SaveRead { .. } | Kind::Crown { .. } => Some((
                i18n::t("action.view"),
                Box::new(|w: &mut MainWindow| {
                    w.page = Page::Records;
                    w.records.tab = 1;
                }),
            )),
            _ => None,
        }
    }

    fn toast(&mut self, ui: &mut Ui, pal: &Pal, full: Rect) {
        if !self.shared.settings().notify.toast {
            return;
        }
        let mut g = self.shared.notifications.lock().unwrap();
        let Some((n, at)) = g.toast.clone() else { return };
        let age = at.elapsed().as_secs_f32();
        if age > 8.0 {
            g.toast = None;
            return;
        }
        drop(g);
        let a = (age / 0.25).min(1.0).min((8.0 - age) / 0.3).clamp(0.0, 1.0);
        let pos = pos2(full.max.x - 24.0 - 300.0, full.max.y - 24.0 - 96.0 + (1.0 - a) * 20.0);
        let mut dismiss = false;
        let mut act = None;
        egui::Area::new(Id::unique("toast"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ui.ctx(), |ui| {
                ui.set_opacity(a);
                egui::Frame::new()
                    .fill(pal.block)
                    .inner_margin(egui::Margin::symmetric(18, 16))
                    .shadow(egui::Shadow {
                        offset: [0, 16],
                        blur: 40,
                        spread: 0,
                        color: Color32::from_black_alpha(70),
                    })
                    .show(ui, |ui| {
                        ui.set_width(264.0);
                        widgets::label(ui, &n.title, Font::TextBold, widgets::T_BODY, pal.onblock);
                        if !n.body.is_empty() {
                            widgets::label(ui, &n.body, Font::Text, widgets::T_SMALL, pal.onblock.gamma_multiply(0.65));
                        }
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 16.0;
                            let inv = Pal {
                                ink: pal.onblock,
                                faint: pal.onblock.gamma_multiply(0.6),
                                ..*pal
                            };
                            if let Some((label, _)) = self.notification_action(&n.kind)
                                && link(ui, &inv, &label).clicked()
                            {
                                act = Some(n.kind.clone());
                            }
                            if link(ui, &inv, &i18n::t("action.later")).clicked() {
                                dismiss = true;
                            }
                        });
                    });
            });
        if let Some(k) = act
            && let Some((_, f)) = self.notification_action(&k)
        {
            f(self);
            dismiss = true;
        }
        if dismiss {
            self.shared.notifications.lock().unwrap().toast = None;
        }
        if self.shared.main_window_live() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(30));
        }
    }

    fn modal(&mut self, ui: &mut Ui, pal: &Pal, area: Rect) {
        let Some(m) = &self.modal else { return };
        let (title, lines, yes) = (m.title.clone(), m.lines.clone(), m.yes.clone());
        let mut result = None;
        egui::Area::new(Id::unique("modal-bg"))
            .order(egui::Order::Foreground)
            .fixed_pos(area.min)
            .show(ui.ctx(), |ui| {
                let (r, _) = ui.allocate_exact_size(area.size(), Sense::click());
                ui.painter().rect_filled(r, CornerRadius::ZERO, Color32::from_black_alpha(115));
            });
        egui::Area::new(Id::unique("modal"))
            .order(egui::Order::Tooltip)
            .fixed_pos(area.center() - vec2(210.0, 110.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(pal.paper)
                    .inner_margin(egui::Margin {
                        left: 28,
                        right: 28,
                        top: 26,
                        bottom: 22,
                    })
                    .shadow(egui::Shadow {
                        offset: [0, 20],
                        blur: 60,
                        spread: 0,
                        color: Color32::from_black_alpha(100),
                    })
                    .show(ui, |ui| {
                        ui.set_width(364.0);
                        widgets::label(ui, &title, Font::TextBold, widgets::T_HEAD, pal.ink);
                        ui.add_space(10.0);
                        for l in &lines {
                            ui.add(egui::Label::new(widgets::rich(l, Font::Text, widgets::T_BODY, pal.ink)).wrap());
                        }
                        ui.add_space(20.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if button(ui, pal, &yes, Btn::Primary, false).clicked() {
                                result = Some(true);
                            }
                            if button(ui, pal, &i18n::t("action.cancel"), Btn::Outline, false).clicked() {
                                result = Some(false);
                            }
                        });
                    });
            });
        match result {
            Some(true) => {
                if let Some(m) = self.modal.take() {
                    (m.on_yes)(&self.shared);
                }
            }
            Some(false) => self.modal = None,
            None => {}
        }
    }

    fn update_panel(&mut self, ui: &mut Ui, pal: &Pal, r: Rect) {
        let mut close = false;
        egui::Area::new(Id::unique("update-panel"))
            .order(egui::Order::Foreground)
            .fixed_pos(r.min)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(pal.paper)
                    .stroke(Stroke::new(1.0, pal.line))
                    .inner_margin(egui::Margin::symmetric(32, 28))
                    .shadow(egui::Shadow {
                        offset: [-20, 0],
                        blur: 50,
                        spread: 0,
                        color: Color32::from_black_alpha(46),
                    })
                    .show(ui, |ui| {
                        ui.set_width(r.width() - 64.0);
                        ui.set_min_height(r.height() - 56.0);
                        ui.horizontal(|ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                                if link(ui, pal, "×").clicked() {
                                    close = true;
                                }
                            });
                        });
                        let st = crate::update::with_state(|s| (s.latest.clone(), s.releases.clone(), s.download.clone(), s.error.clone(), s.checked));
                        let (latest, releases, dl, err, checked) = st;
                        let cur = rh_core::VERSION;
                        match &latest {
                            Some(rel) if rel.version().is_some_and(|v| crate::update::newer(&v, cur)) => {
                                widgets::label(ui, rel.version().unwrap_or_default(), Font::NumHeavy, widgets::T_DISPLAY, pal.ink);
                                let size = rel.package().map(|a| format!("{:.1} MB", a.size as f32 / 1048576.0)).unwrap_or_default();
                                widgets::label_mute(ui, pal, &i18n::tf("update.current", &[("v", cur), ("size", &size)]), 12.0);
                                ui.add_space(20.0);
                                match &dl {
                                    crate::update::Download::Idle | crate::update::Download::Failed(_) => {
                                        if button(ui, pal, &i18n::t("action.download"), Btn::Primary, false).clicked() {
                                            crate::update::start_download(self.shared.clone(), rel.clone());
                                        }
                                        if let crate::update::Download::Failed(e) = &dl {
                                            widgets::label(ui, e, Font::Text, widgets::T_SMALL, pal.accent);
                                        }
                                    }
                                    crate::update::Download::Running { done, total } => {
                                        let r = if *total > 0 { *done as f32 / *total as f32 } else { 0.0 };
                                        widgets::thin_bar(ui, pal.ink, pal.line, r, vec2(ui.available_width(), 2.0));
                                        ui.horizontal(|ui| {
                                            widgets::label_mute(ui, pal, &i18n::t("update.downloading"), 12.0);
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                widgets::label_mute(ui, pal, &format!("{}%", (r * 100.0).round()), 12.0);
                                            });
                                        });
                                    }
                                    crate::update::Download::Ready { version } => {
                                        widgets::label_mute(ui, pal, &format!("{version}　{}", i18n::t("update.ready")), 12.0);
                                    }
                                }
                            }
                            _ => {
                                widgets::label(ui, cur, Font::NumHeavy, widgets::T_DISPLAY, pal.ink);
                                let msg = if let Some(e) = &err {
                                    i18n::tf("update.failed", &[("e", e)])
                                } else if checked {
                                    i18n::t("update.latest")
                                } else {
                                    i18n::t("update.checking")
                                };
                                widgets::label_mute(ui, pal, &msg, 12.0);
                                ui.add_space(12.0);
                                if button(ui, pal, &i18n::t("action.check_update"), Btn::Outline, true).clicked() {
                                    let s = self.shared.clone();
                                    std::thread::spawn(move || crate::update::check(&s, false));
                                }
                            }
                        }
                        ui.add_space(30.0);
                        for rel in releases.iter().filter(|r| r.version().is_some()).take(8) {
                            ui.horizontal(|ui| {
                                widgets::label(ui, rel.version().unwrap_or_default(), Font::TextBold, widgets::T_BODY, pal.ink);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    widgets::label(ui, rel.date(), Font::NumBold, widgets::T_SMALL, pal.mute);
                                });
                            });
                            for n in rel.notes().iter().take(12) {
                                let (kind, rest) = n.split_once(' ').unwrap_or(("", n));
                                ui.horizontal(|ui| {
                                    let (r, _) = ui.allocate_exact_size(vec2(30.0, 18.0), Sense::hover());
                                    text_at(ui, r.left_center(), Align2::LEFT_CENTER, kind, Font::Text, widgets::T_SMALL, pal.mute);
                                    widgets::label(ui, rest, Font::Text, widgets::T_BODY, pal.ink);
                                });
                                widgets::hline(ui, pal);
                            }
                            ui.add_space(20.0);
                        }
                    });
            });
        if close {
            self.update_open = false;
        }
    }

    pub fn open_update(&mut self) {
        self.update_open = true;
    }
}

/// Resizing a borderless window by dragging its edges.
fn resize_edges(ui: &mut Ui, r: Rect) {
    use egui::ResizeDirection as D;
    let m = 5.0;
    let edges = [
        (
            Rect::from_min_max(r.min, pos2(r.min.x + m, r.max.y)),
            D::West,
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            Rect::from_min_max(pos2(r.max.x - m, r.min.y), r.max),
            D::East,
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            Rect::from_min_max(pos2(r.min.x, r.max.y - m), r.max),
            D::South,
            egui::CursorIcon::ResizeVertical,
        ),
        (
            Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + 3.0)),
            D::North,
            egui::CursorIcon::ResizeVertical,
        ),
        (
            Rect::from_min_max(pos2(r.max.x - 12.0, r.max.y - 12.0), r.max),
            D::SouthEast,
            egui::CursorIcon::ResizeNwSe,
        ),
    ];
    for (i, (er, dir, cursor)) in edges.into_iter().enumerate() {
        let resp = ui.interact(er, Id::unique(("resize", i)), Sense::drag());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(cursor);
        }
        if resp.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
        }
    }
}

pub fn fmt_when(ts: i64) -> String {
    let Some(t) = chrono::DateTime::from_timestamp(ts, 0) else {
        return String::new();
    };
    let t = t.with_timezone(&chrono::Local);
    let now = chrono::Local::now();
    let days = (now.date_naive() - t.date_naive()).num_days();
    match days {
        0 => i18n::tf("time.today", &[("t", &t.format("%H:%M").to_string())]),
        1 => i18n::tf("time.yesterday", &[("t", &t.format("%H:%M").to_string())]),
        2..=6 => i18n::tf("time.days_ago", &[("n", &days.to_string())]),
        7..=30 => i18n::tf("time.weeks_ago", &[("n", &(days / 7).to_string())]),
        _ => t.format("%Y-%m-%d").to_string(),
    }
}
