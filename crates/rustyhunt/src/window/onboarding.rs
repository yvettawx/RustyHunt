use egui::{Rect, Ui};

use super::MainWindow;
use super::style::Pal;
use crate::state::Shared;

pub struct State;

impl State {
    pub fn new(_shared: &Shared) -> State {
        State
    }

    pub fn ui(&mut self, _w: &mut MainWindow, _ui: &mut Ui, _pal: &Pal, _r: Rect) -> bool {
        false
    }
}
