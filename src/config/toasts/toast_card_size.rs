use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[allow(clippy::struct_field_names)]
pub struct ToastCardSizeConfig {
    pub card_width_px: f32,
    pub app_icon_size_px: f32,
    pub image_size_px: f32,
    pub badge_size_px: f32,
}

impl Default for ToastCardSizeConfig {
    fn default() -> Self {
        Self {
            card_width_px: 380.0,
            app_icon_size_px: 40.0,
            image_size_px: 48.0,
            badge_size_px: 20.0,
        }
    }
}
