pub mod toast_card_size;
pub mod toast_lifetime;
pub mod toast_placement;

use serde::Deserialize;
pub use toast_card_size::ToastCardSizeConfig;
pub use toast_lifetime::ToastLifetimeConfig;
pub use toast_placement::ToastPlacementConfig;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NotificationConfig {
    pub placement: ToastPlacementConfig,
    pub card_size: ToastCardSizeConfig,
    pub lifetime: ToastLifetimeConfig,
}
