pub mod chrome;
pub mod font;
pub mod load;
pub mod locker;
pub mod notifications;
pub mod palette;
pub mod spacing;
pub mod widgets;
pub mod window_list;

pub use load::{Config, SecretPaths, load};
pub use notifications::NotificationConfig;
pub use palette::Palette;
