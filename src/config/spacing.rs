use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[allow(clippy::struct_field_names)]
pub struct Spacing {
    pub group_padding_px: f32,
    pub widget_spacing_px: f32,
    pub group_spacing_px: f32,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            group_padding_px: 5.0,
            widget_spacing_px: 5.0,
            group_spacing_px: 16.0,
        }
    }
}
