//! Reading Monster Hunter: World Iceborne: process and address maps, per-frame data, Hunter's Notes in the save.

pub mod attach;
pub mod data;
pub mod demo;
pub mod reader;
pub mod save;
mod save_tables;

pub use reader::WorldReader;
