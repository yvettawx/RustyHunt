//! 补丁与注入。

use std::sync::Mutex;

use rh_core::model::DamageSplit;

use crate::scanner::Sink;

static SPLIT: Mutex<Option<DamageSplit>> = Mutex::new(None);

pub fn damage_split() -> Option<DamageSplit> {
    SPLIT.lock().unwrap().clone()
}

pub struct Injector;

impl Injector {
    pub fn new() -> Injector {
        Injector
    }
}

impl Sink for Injector {
    fn frame(&mut self, _: &crate::state::Shared, _: &std::sync::Arc<rh_core::model::Snapshot>, _: &[rh_core::Event]) {}
}

#[derive(Debug, Clone)]
pub enum ItemStatus {
    Off,
    Waiting,
    Active(String),
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct Status {
    pub crc: ItemStatus,
    pub hook: ItemStatus,
    pub dll: String,
    pub ipc: String,
    pub last_failure: Option<String>,
    pub stracker: bool,
}

pub fn status() -> Status {
    Status {
        crc: ItemStatus::Off,
        hook: ItemStatus::Off,
        dll: "-".into(),
        ipc: "-".into(),
        last_failure: None,
        stracker: false,
    }
}
