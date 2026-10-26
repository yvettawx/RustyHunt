//! 主界面的颜色，从当前主题的 `window.*` 令牌取。

use egui::Color32;
use ui_kit::Theme;

#[derive(Clone, Copy, Debug)]
pub struct Pal {
    pub paper: Color32,
    pub ink: Color32,
    pub mute: Color32,
    pub faint: Color32,
    pub line: Color32,
    pub block: Color32,
    pub onblock: Color32,
    pub accent: Color32,
    pub desk: Color32,
    pub dark: bool,
}

impl Pal {
    pub fn of(theme: &Theme) -> Pal {
        let c = |k: &str| theme.color(k);
        Pal {
            paper: c("window.paper"),
            ink: c("window.ink"),
            mute: c("window.mute"),
            faint: c("window.faint"),
            line: c("window.line"),
            block: c("window.block"),
            onblock: c("window.onblock"),
            accent: c("window.accent"),
            desk: c("window.desk"),
            dark: theme.manifest.dark,
        }
    }

    /// 悬停时的浅底色
    pub fn hover(&self) -> Color32 {
        if self.dark {
            Color32::from_rgba_unmultiplied(255, 255, 255, 10)
        } else {
            Color32::from_rgba_unmultiplied(0, 0, 0, 13)
        }
    }
}

pub fn apply(ctx: &egui::Context, theme: &Theme) {
    let p = Pal::of(theme);
    let mut v = if p.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = p.paper;
    v.window_fill = p.paper;
    v.extreme_bg_color = p.paper;
    v.faint_bg_color = p.paper;
    v.override_text_color = Some(p.ink);
    v.selection.bg_fill = p.accent.gamma_multiply(0.35);
    v.selection.stroke = egui::Stroke::new(1.0, p.ink);
    v.hyperlink_color = p.ink;
    v.window_stroke = egui::Stroke::new(1.0, p.line);
    v.window_corner_radius = egui::CornerRadius::ZERO;
    v.menu_corner_radius = egui::CornerRadius::ZERO;
    v.popup_shadow = egui::Shadow {
        offset: [0, 12],
        blur: 34,
        spread: 0,
        color: Color32::from_black_alpha(50),
    };
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = egui::CornerRadius::ZERO;
        w.bg_fill = p.paper;
        w.weak_bg_fill = p.paper;
        w.fg_stroke.color = p.ink;
        w.bg_stroke = egui::Stroke::new(1.0, p.line);
    }
    v.widgets.hovered.weak_bg_fill = p.hover();
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, p.ink);
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0, p.ink);
    v.text_cursor.stroke = egui::Stroke::new(1.5, p.ink);
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(12.0, 5.0);
        s.spacing.scroll.bar_width = 6.0;
        s.spacing.scroll.floating = true;
        s.interaction.selectable_labels = false;
        s.text_styles.insert(egui::TextStyle::Body, ui_kit::Font::Text.id(13.0));
        s.text_styles.insert(egui::TextStyle::Button, ui_kit::Font::Text.id(13.0));
        s.text_styles.insert(egui::TextStyle::Small, ui_kit::Font::Text.id(11.0));
        s.text_styles.insert(egui::TextStyle::Monospace, ui_kit::Font::Mono.id(12.0));
        s.text_styles.insert(egui::TextStyle::Heading, ui_kit::Font::TextBold.id(30.0));
    });
}
