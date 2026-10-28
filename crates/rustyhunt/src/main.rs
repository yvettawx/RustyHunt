//! RustyHunt: an in-game overlay and desktop app for Monster Hunter. Rewritten from HunterPie.

#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod bestiary;
mod crash;
mod hotkeys;
mod injection;
mod layered;
mod logging;
mod notify;
mod overlay;
mod plugins;
mod records;
mod runner;
mod scanner;
mod screenshot;
mod state;
mod themes;
mod tray;
mod update;
mod win;
mod window;

use rh_core::i18n;
use rh_core::paths;
use rh_core::settings::Settings;

struct Args {
    tray: bool,
    demo: bool,
    screenshot: Option<(String, std::path::PathBuf)>,
    /// `--bench <seconds>`: run on demo data next to a running copy, then quit (for measuring)
    bench: Option<u64>,
    data_dir: Option<std::path::PathBuf>,
}

fn args() -> Args {
    let mut a = Args {
        tray: false,
        demo: false,
        screenshot: None,
        bench: None,
        data_dir: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(x) = it.next() {
        match x.as_str() {
            "--tray" => a.tray = true,
            "--demo" => a.demo = true,
            "--data-dir" => a.data_dir = it.next().map(Into::into),
            "--bench" => a.bench = it.next().and_then(|s| s.parse().ok()),
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

    let dev_run = args.screenshot.is_some() || args.bench.is_some();
    if !dev_run && !win::single_instance() {
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
        // Screenshots always show demo data, even with the game running
        settings.first_run_done = true;
        settings.demo = true;
        let _ = scanner::DEMO_SCENE.set(what.strip_prefix("overlay:").unwrap_or_default().to_string());
    }
    if args.bench.is_some() {
        // Demo hunt only, like screenshots: never the game
        settings.first_run_done = true;
        settings.demo = true;
        let _ = scanner::DEMO_SCENE.set(String::new());
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
    if !dev_run {
        update::check_on_start(shared.clone());
    }
    if let Some(secs) = args.bench {
        let shared = shared.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(secs));
            log::info!("bench finished after {secs} s");
            shared.act(state::Action::Quit);
        });
    }

    if let Some((what, path)) = args.screenshot.clone() {
        screenshot::init(what, path);
    }
    if let Err(e) = runner::run(shared, args.tray, args.screenshot) {
        log::error!("failed to start: {e}");
        win::error_box("RustyHunt", &e);
    }
}
