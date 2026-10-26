use egui::Ui;

use super::MainWindow;
use super::style::Pal;

#[derive(Default)]
pub struct State {
    pub tab: usize,
}

impl State {
    pub fn select(&mut self, _id: i64) {}
}

pub fn ui(_w: &mut MainWindow, _ui: &mut Ui, _pal: &Pal) {}
