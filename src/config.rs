pub mod bar_size;
pub mod font;
pub mod load;
pub mod locale;
pub mod palette;
pub mod secrets;
pub mod spacing;
pub mod toasts;
pub mod tray;
mod watch;
pub mod widget_sizes;
pub mod window_list;

pub use load::{Config, ConfigFile, LoadError, config_home};
pub use palette::Palette;
pub use secrets::SecretPaths;
pub use toasts::ToastConfig;
