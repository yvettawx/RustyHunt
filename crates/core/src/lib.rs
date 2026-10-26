//! 三作共用的数据结构、事件、设置、文案和模块描述。上层只依赖这里，
//! 不直接依赖某一作的读取逻辑。

pub mod events;
pub mod i18n;
pub mod model;
pub mod modules;
pub mod paths;
pub mod settings;

pub use events::Event;
pub use model::*;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 检查更新用的 GitHub 仓库，可以在设置里改。
pub const DEFAULT_REPO: &str = "RustyHunt/RustyHunt";

/// 数据字段表的版本，加字段时升小版本。
pub const FIELDS_VERSION: &str = "1.0";
