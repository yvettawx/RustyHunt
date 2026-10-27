//! For development: `--screenshot main[:page]|overlay|layout <file>` runs a few dozen frames of demo data
//! and saves the picture as a PNG for checking the UI.

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

fn save(img: &egui::ColorImage, path: &std::path::Path) {
    let [w, h] = img.size;
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
            self.shared.act(crate::state::Action::SetLayout(true));
        }
        if self.frames == 120 && !self.requested {
            self.requested = true;
            let path = self.path.clone();
            let done = self.done.clone();
            let shared = self.shared.clone();
            ctx.send_viewport_cmd_to(
                target,
                egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
                    save(&img, &path);
                    log::info!("截图已保存 {}", path.display());
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
