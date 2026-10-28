//! For development: `--screenshot main[:page]|overlay[:scene]|layout <file>` runs a few dozen frames of demo data
//! and saves the picture as a PNG for checking the UI. Overlay shots also write `<file>.json` with each
//! module's rect in 1080-high reference coordinates, for cropping. The pictures are the CPU-drawn frames
//! themselves, exactly what goes to the windows.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use egui::Rect;

use crate::app::App;
use crate::layered::Bitmap;
use crate::state::{Action, Shared};

static SHOT: OnceLock<(String, PathBuf)> = OnceLock::new();
static DONE: AtomicBool = AtomicBool::new(false);
static OVERLAY_FRAMES: AtomicU32 = AtomicU32::new(0);

/// Seconds after which a shot that never became due ends the program anyway
const GIVE_UP: f64 = 30.0;

pub fn init(what: String, path: PathBuf) {
    let _ = SHOT.set((what, path));
}

/// Opens the page or onboarding step named after `main:`.
pub fn prepare(app: &mut App, what: &str) {
    let Some(rest) = what.strip_prefix("main:") else { return };
    let mut it = rest.split(':');
    if rest.starts_with("onboarding") {
        app.open_onboarding(it.nth(1).and_then(|t| t.parse().ok()).unwrap_or(0));
    } else if let Some(page) = it.next().and_then(crate::window::Page::from_key) {
        app.set_page(page, it.next().and_then(|t| t.parse().ok()).unwrap_or(0));
    }
}

/// Scenes switch state at a fixed time; capture shortly after, while the change still animates
fn due(shared: &Shared, frames: u32) -> bool {
    frames >= 120 && shared.now() >= game_world::demo::SCENE_SWITCH + 0.45
}

/// Called with every main window frame (BGRA pixels).
pub fn main_frame(shared: &Shared, _app: &App, frames: u32, px: &[[u8; 4]], w: usize, h: usize) {
    let Some((what, path)) = SHOT.get() else { return };
    if !what.starts_with("main") || DONE.load(Ordering::Relaxed) {
        return;
    }
    if due(shared, frames) {
        save(px, w, h, path, false);
        finish(shared, path);
    } else if shared.now() > GIVE_UP {
        shared.act(Action::Quit);
    }
    if let Some(ctx) = shared.ctx.get() {
        ctx.request_repaint();
    }
}

/// Called with every overlay frame.
pub fn overlay_frame(shared: &Shared, bitmap: &mut Bitmap, rects: &BTreeMap<String, Rect>) {
    let Some((what, path)) = SHOT.get() else { return };
    if what.starts_with("main") || DONE.load(Ordering::Relaxed) {
        return;
    }
    let frames = OVERLAY_FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    if what == "layout" && frames == 5 {
        shared.act(Action::SetLayout(true));
    }
    if due(shared, frames) {
        let rects: BTreeMap<&String, [f32; 4]> = rects.iter().map(|(k, r)| (k, [r.min.x, r.min.y, r.width(), r.height()])).collect();
        let _ = std::fs::write(path.with_extension("json"), serde_json::to_string(&rects).unwrap_or_default());
        let (w, h) = (bitmap.w, bitmap.h);
        save(bitmap.pixels(), w, h, path, true);
        finish(shared, path);
    } else if shared.now() > GIVE_UP {
        shared.act(Action::Quit);
    }
    if let Some(ctx) = shared.overlay_ctx.get() {
        ctx.request_repaint();
    }
}

fn finish(shared: &Shared, path: &Path) {
    DONE.store(true, Ordering::Relaxed);
    log::info!("screenshot saved {}", path.display());
    shared.act(Action::Quit);
}

/// `px` is BGRA with premultiplied alpha.
fn save(px: &[[u8; 4]], w: usize, h: usize, path: &Path, alpha: bool) {
    if alpha {
        // Also keep the transparent picture, for putting it over a game frame
        let mut raw = image::RgbaImage::new(w as u32, h as u32);
        for (i, &[b, g, r, a]) in px.iter().enumerate() {
            let un = |c: u8| {
                if a == 0 {
                    0
                } else {
                    ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8
                }
            };
            raw.put_pixel((i % w) as u32, (i / w) as u32, image::Rgba([un(r), un(g), un(b), a]));
        }
        let _ = raw.save(path.with_extension("alpha.png"));
    }
    let mut buf = image::RgbaImage::new(w as u32, h as u32);
    for (i, &[b, g, r, a]) in px.iter().enumerate() {
        // Premultiplied over dark gray (the overlay is transparent); the main window is opaque
        let bg = if alpha { 46u32 } else { 0 };
        let over = |c: u8| (c as u32 + bg * (255 - a as u32) / 255).min(255) as u8;
        let p = if alpha { [over(r), over(g), over(b), 255] } else { [r, g, b, 255] };
        buf.put_pixel((i % w) as u32, (i / w) as u32, image::Rgba(p));
    }
    let _ = buf.save(path);
}
