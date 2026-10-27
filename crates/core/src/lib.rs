//! Data structures, events, settings, copy and module descriptions shared by the three games.
//! Upper layers depend only on this crate, never directly on one game's reader.

pub mod combat;
pub mod events;
pub mod i18n;
pub mod model;
pub mod modules;
pub mod native;
pub mod paths;
pub mod settings;

pub use events::Event;
pub use model::*;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// GitHub repository used for update checks; can be changed in settings.
pub const DEFAULT_REPO: &str = "yvettawx/RustyHunt";

/// Version of the data field table; bump the minor version when adding fields.
pub const FIELDS_VERSION: &str = "1.0";
