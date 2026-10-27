//! Hotkeys: polled only while the game or one of our windows is in the foreground, so no keys are grabbed globally.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use platform::keys::{EdgeDetector, Hotkey};

use crate::state::{Action, Shared};

fn parse(s: &str) -> Option<Hotkey> {
    Hotkey::parse(s)
}

fn ours_in_front(shared: &Shared) -> bool {
    let Some(fg) = platform::window::foreground() else { return false };
    let pid = fg.pid();
    pid == std::process::id() || shared.live().pid == Some(pid)
}

pub fn spawn(shared: Arc<Shared>) {
    std::thread::Builder::new()
        .name("hotkeys".into())
        .spawn(move || {
            let mut edges = EdgeDetector::default();
            let mut held = false;
            while !shared.quit.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(16));
                if !ours_in_front(&shared) {
                    edges.reset();
                    if held {
                        held = false;
                        shared.show_all.store(false, Ordering::Relaxed);
                        shared.repaint_overlay();
                    }
                    continue;
                }
                let hk = shared.settings().hotkeys.clone();
                let fire = |s: &str, a: Action, edges: &mut EdgeDetector| {
                    if let Some(k) = parse(s)
                        && edges.poll(&k).1
                    {
                        shared.act(a);
                    }
                };
                fire(&hk.toggle_overlay, Action::ToggleOverlay, &mut edges);
                fire(&hk.layout_mode, Action::ToggleLayout, &mut edges);
                fire(&hk.density, Action::CycleDensity, &mut edges);
                fire(&hk.cycle_target, Action::CycleTarget, &mut edges);
                // Plugin actions: the user's key wins, otherwise the plugin's default
                let mut plugin_keys: std::collections::BTreeMap<String, String> = crate::plugins::default_keys().into_iter().collect();
                plugin_keys.extend(hk.plugin.clone());
                for (id, key) in &plugin_keys {
                    if let Some((plugin, action)) = id.split_once('.') {
                        fire(
                            key,
                            Action::Plugin {
                                plugin: plugin.into(),
                                action: action.into(),
                            },
                            &mut edges,
                        );
                    }
                }
                // Hold to show everything
                let down = parse(&hk.show_all).is_some_and(|k| edges.poll(&k).0);
                if down != held {
                    held = down;
                    shared.show_all.store(down, Ordering::Relaxed);
                    shared.repaint_overlay();
                }
            }
        })
        .expect("spawn the hotkey thread");
}
