//! For development: `--screenshot main[:page]|overlay[:scene]|layout <file>` runs a few dozen frames of demo data
//! and saves the picture as a PNG for checking the UI. Overlay shots also write `<file>.json` with each
//! module's rect in 1080-high reference coordinates, for cropping.

use std::path::PathBuf;
use std::sync::Arc;

use crate::state::Shared;

pub struct Shot {
    app: crate::app::App,
    shared: Arc<Shared>,
    what: String,
    path: PathBuf,
    frames: u32,
    requested: bool,
    done: Arc<std::sync::atomic::AtomicBool>,
}

impl Shot {
    pub fn new(app: crate::app::App, shared: Arc<Shared>, what: String, path: PathBuf) -> Shot {
        Shot {
            app,
            shared,
            what,
            path,
            frames: 0,
            requested: false,
            done: Default::default(),
        }
    }
}

fn save(img: &egui::ColorImage, path: &std::path::Path, alpha: bool) {
    let [w, h] = img.size;
    if alpha {
        // Also keep the transparent picture, for putting it over a game frame
        let mut raw = image::RgbaImage::new(w as u32, h as u32);
        for (i, p) in img.pixels.iter().enumerate() {
            raw.put_pixel((i % w) as u32, (i / w) as u32, image::Rgba(p.to_srgba_unmultiplied()));
        }
        let _ = raw.save(path.with_extension("alpha.png"));
    }
    let mut buf = image::RgbaImage::new(w as u32, h as u32);
    for (i, p) in img.pixels.iter().enumerate() {
        let [r, g, b, a] = p.to_srgba_unmultiplied();
        // The overlay is transparent, so put dark gray behind it before saving
        let bg = 46u32;
        let mix = |c: u8| ((c as u32 * a as u32 + bg * (255 - a as u32)) / 255) as u8;
        buf.put_pixel((i % w) as u32, (i / w) as u32, image::Rgba([mix(r), mix(g), mix(b), 255]));
    }
    let _ = buf.save(path);
}

impl eframe::App for Shot {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
        let ctx = ui.ctx().clone();
        self.frames += 1;
        // main:page[:tab]
        if self.frames == 1
            && let Some(rest) = self.what.strip_prefix("main:")
        {
            let mut it = rest.split(':');
            if rest.starts_with("onboarding") {
                self.app.open_onboarding(it.nth(1).and_then(|t| t.parse().ok()).unwrap_or(0));
            } else if let Some(page) = it.next().and_then(crate::window::Page::from_key) {
                self.app.set_page(page, it.next().and_then(|t| t.parse().ok()).unwrap_or(0));
            }
        }
        let target = if self.what.starts_with("main") {
            crate::window::main_viewport()
        } else {
            egui::ViewportId::ROOT
        };
        if self.what == "layout" && self.frames == 5 {
            crate::overlay::LAYOUT_INLINE.store(true, std::sync::atomic::Ordering::Relaxed);
            self.shared.act(crate::state::Action::SetLayout(true));
        }
        // Scenes switch state at a fixed time; capture shortly after, while the change still animates
        let due = self.shared.now() >= game_world::demo::SCENE_SWITCH + 0.45;
        if self.frames >= 120 && due && !self.requested {
            self.requested = true;
            if !self.what.starts_with("main") {
                let rects: std::collections::BTreeMap<String, [f32; 4]> = self
                    .app
                    .module_rects()
                    .into_iter()
                    .map(|(k, r)| (k, [r.min.x, r.min.y, r.width(), r.height()]))
                    .collect();
                let _ = std::fs::write(self.path.with_extension("json"), serde_json::to_string(&rects).unwrap_or_default());
            }
            let path = self.path.clone();
            let alpha = !self.what.starts_with("main");
            let done = self.done.clone();
            let shared = self.shared.clone();
            ctx.send_viewport_cmd_to(
                target,
                egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
                    save(&img, &path, alpha);
                    log::info!("screenshot saved {}", path.display());
                    done.store(true, std::sync::atomic::Ordering::Relaxed);
                    shared.repaint();
                })),
            );
        }
        if self.done.load(std::sync::atomic::Ordering::Relaxed) || self.frames > 900 {
            self.shared.act(crate::state::Action::Quit);
        }
        ctx.request_repaint();
        ctx.request_repaint_of(target);
    }

    fn clear_color(&self, v: &egui::Visuals) -> [f32; 4] {
        self.app.clear_color(v)
    }

    fn on_exit(&mut self) {
        self.app.on_exit();
    }
}
