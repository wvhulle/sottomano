pub mod chrome;
pub mod font;
pub mod load;
pub mod notifications;
pub mod palette;
pub mod spacing;
pub mod widgets;

pub use load::{Config, SecretPaths, load};
pub use notifications::NotificationConfig;
pub use palette::Palette;
