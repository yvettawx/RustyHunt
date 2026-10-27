//! RustyHunt: an in-game overlay and desktop app for Monster Hunter. Rewritten from HunterPie.

#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod bestiary;
mod crash;
mod hotkeys;
mod injection;
mod logging;
mod notify;
mod overlay;
mod plugins;
mod records;
mod scanner;
mod screenshot;
mod state;
mod themes;
mod tray;
mod update;
mod win;
mod window;

use std::sync::Arc;

use rh_core::i18n;
use rh_core::paths;
use rh_core::settings::Settings;

struct Args {
    tray: bool,
    demo: bool,
    screenshot: Option<(String, std::path::PathBuf)>,
    data_dir: Option<std::path::PathBuf>,
}

fn args() -> Args {
    let mut a = Args {
        tray: false,
        demo: false,
        screenshot: None,
        data_dir: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(x) = it.next() {
        match x.as_str() {
            "--tray" => a.tray = true,
            "--demo" => a.demo = true,
            "--data-dir" => a.data_dir = it.next().map(Into::into),
            "--screenshot" => {
                if let (Some(what), Some(path)) = (it.next(), it.next()) {
                    a.screenshot = Some((what, path.into()));
                    a.demo = true;
                }
            }
            _ => {}
        }
    }
    a
}

fn main() {
    let args = args();
    paths::init(args.data_dir.clone());
    let mut settings = Settings::load(&paths::settings_file());
    logging::init(settings.log_level);
    crash::install();
    log::info!("starting RustyHunt {}, data folder {}", rh_core::VERSION, paths::root().display());

    if args.screenshot.is_none() && !win::single_instance() {
        // One is already running: ask it to open its main window
        if let Some(h) = win::own_window_any(window::TITLE) {
            win::focus_window(h);
        }
        log::info!("another instance is running, exiting");
        return;
    }
    if update::apply_pending() {
        return;
    }

    if args.demo {
        settings.demo = true;
    }
    if let Some((what, _)) = &args.screenshot {
        settings.first_run_done = true;
        if let Some(scene) = what.strip_prefix("overlay:") {
            let _ = scanner::DEMO_SCENE.set(scene.to_string());
        }
    }
    let i = i18n::load_bundled(&paths::bundled("locales"), settings.language, settings.term_language);
    i18n::install(i);

    let theme = themes::load_active(&settings);
    let shared = state::Shared::new(settings, theme);
    *shared.notifications.lock().unwrap() = notify::Notifications::load();
    crash::report_previous(&shared);

    let sinks: Vec<Box<dyn scanner::Sink>> = vec![
        Box::new(records::Recorder::new()),
        Box::new(injection::Injector::new()),
        Box::new(plugins::Bridge::new(shared.clone())),
    ];
    let _scanner = scanner::Scanner::new(shared.clone(), sinks).spawn();
    hotkeys::spawn(shared.clone());
    themes::watch(shared.clone());
    if args.screenshot.is_none() {
        update::check_on_start(shared.clone());
    }

    let viewport = egui::ViewportBuilder::default()
        .with_title(overlay_title())
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_mouse_passthrough(true)
        .with_taskbar(false)
        .with_resizable(false)
        .with_inner_size([1.0, 1.0])
        .with_position([-32000.0, -32000.0])
        .with_icon(Arc::new(crate::window::icon()));
    // OpenGL into a layered (click-through) window shows nothing on some monitors, and DX12 presented
    // straight to the window has no alpha. DX12 through DirectComposition is transparent everywhere.
    let mut wgpu_setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    wgpu_setup.instance_descriptor.backends = eframe::wgpu::Backends::DX12;
    wgpu_setup.instance_descriptor.backend_options.dx12.presentation_system = eframe::wgpu::Dx12SwapchainKind::DxgiFromVisual;
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: wgpu_setup.into(),
            ..Default::default()
        },
        ..Default::default()
    };
    let shot = args.screenshot.clone();
    let start_tray = args.tray;
    let result = eframe::run_native(
        "RustyHunt",
        options,
        Box::new(move |cc| {
            let app = app::App::new(cc, shared.clone(), start_tray);
            Ok(match shot {
                Some((what, path)) => Box::new(screenshot::Shot::new(app, shared, what, path)) as Box<dyn eframe::App>,
                None => Box::new(app),
            })
        }),
    );
    if let Err(e) = result {
        log::error!("failed to create the window: {e}");
        win::error_box("RustyHunt", &format!("{e}"));
    }
}

pub fn overlay_title() -> &'static str {
    "RustyHunt Overlay"
}
