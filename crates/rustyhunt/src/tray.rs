//! 托盘图标：单击打开主界面；右键菜单有显示或隐藏 overlay、布局模式、
//! 密度、打开日志、退出。

use std::sync::Arc;

use rh_core::i18n;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::state::{Action, Shared};

pub struct Tray {
    _icon: TrayIcon,
    overlay: MenuItem,
    density: MenuItem,
}

impl Tray {
    pub fn new(shared: Arc<Shared>) -> Option<Tray> {
        let open = MenuItem::with_id("open", i18n::t("tray.open"), true, None);
        let overlay = MenuItem::with_id("overlay", "", true, None);
        let layout = MenuItem::with_id(
            "layout",
            format!("{}\t{}", i18n::t("action.layout"), shared.settings().hotkeys.layout_mode),
            true,
            None,
        );
        let density = MenuItem::with_id("density", "", true, None);
        let log = MenuItem::with_id("log", i18n::t("tray.open_log"), true, None);
        let quit = MenuItem::with_id("quit", i18n::t("tray.quit"), true, None);
        let menu = Menu::new();
        let sep1 = PredefinedMenuItem::separator();
        let sep2 = PredefinedMenuItem::separator();
        menu.append_items(&[&open, &sep1, &overlay, &layout, &density, &sep2, &log, &quit]).ok()?;

        let rgba = crate::window::icon_rgba(32);
        let icon = tray_icon::Icon::from_rgba(rgba, 32, 32).ok()?;
        let tray = TrayIconBuilder::new()
            .with_tooltip("RustyHunt")
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .map_err(|e| log::warn!("创建托盘图标失败：{e}"))
            .ok()?;

        let s = shared.clone();
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = e
            {
                s.act(Action::ShowMain);
            }
        }));
        let s = shared.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| match e.id.0.as_str() {
            "open" => s.act(Action::ShowMain),
            "overlay" => s.act(Action::ToggleOverlay),
            "layout" => s.act(Action::ToggleLayout),
            "density" => s.act(Action::CycleDensity),
            "log" => crate::win::open(&crate::logging::file_path()),
            "quit" => s.act(Action::Quit),
            _ => {}
        }));
        let t = Tray {
            _icon: tray,
            overlay,
            density,
        };
        t.refresh(&shared);
        Some(t)
    }

    /// 菜单里的状态文字跟着设置变。
    pub fn refresh(&self, shared: &Shared) {
        let s = shared.settings();
        let state = if s.overlay_visible { i18n::t("state.showing") } else { i18n::t("state.hidden") };
        self.overlay.set_text(format!("overlay\t{state}"));
        self.density.set_text(format!("{}\t{}", i18n::t("label.density"), i18n::t(s.density.key())));
    }
}
