//! 《怪物猎人 世界：冰原》的读取：进程和地址表、每帧的数据、存档里的猎人笔记。

pub mod attach;
pub mod data;
pub mod demo;
pub mod reader;
pub mod save;
mod save_tables;

pub use reader::WorldReader;
