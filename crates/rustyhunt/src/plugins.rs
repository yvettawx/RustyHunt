//! 插件。

use std::sync::Arc;

use crate::overlay::{Frame, State};
use crate::scanner::Sink;
use crate::state::Shared;

pub struct Bridge;

impl Bridge {
    pub fn new(_shared: Arc<Shared>) -> Bridge {
        Bridge
    }
}

impl Sink for Bridge {
    fn frame(&mut self, _: &Shared, _: &Arc<rh_core::model::Snapshot>, _: &[rh_core::Event]) {}
}

pub fn draw_module(_id: &str, _cv: &ui_kit::Canvas, _f: &Frame, _st: &mut State, _ctx: &egui::Context) -> egui::Vec2 {
    egui::Vec2::ZERO
}

pub fn module_name(id: &str) -> String {
    rh_core::i18n::t(&format!("module.{id}"))
}

pub fn run_action(_shared: &Shared, _plugin: &str, _action: &str) {}

pub struct ActionInfo {
    pub plugin: String,
    pub plugin_name: String,
    pub id: String,
    pub name: String,
    pub default_key: String,
}

pub fn actions() -> Vec<ActionInfo> {
    Vec::new()
}

pub struct ChannelInfo {
    pub plugin: String,
    pub plugin_name: String,
    pub id: String,
    pub name: String,
}

pub fn channels() -> Vec<ChannelInfo> {
    Vec::new()
}
