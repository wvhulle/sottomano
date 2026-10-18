pub mod calendar;
pub mod launched_programs;
pub mod layout;
pub mod load;
pub mod notification_toasts;
pub mod panel_bar;
pub mod screen_edges;
pub mod secrets;
pub mod severity;
pub mod theme;
pub mod tooltips;
pub mod widget_groups;

pub use load::{Config, ConfigFile, LoadError, config_home, default_config_toml};
pub use secrets::SecretPaths;
pub use theme::palette::Palette;
