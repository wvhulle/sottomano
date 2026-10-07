use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[allow(clippy::struct_field_names)]
pub struct ToastCardSizeConfig {
    pub card_width_px: f32,
    pub icon_size_px: f32,
    pub app_badge_size_px: f32,
    pub image_preview_max_height_px: f32,
}

impl Default for ToastCardSizeConfig {
    fn default() -> Self {
        Self {
            card_width_px: 380.0,
            icon_size_px: 44.0,
            app_badge_size_px: 20.0,
            image_preview_max_height_px: 180.0,
        }
    }
}
