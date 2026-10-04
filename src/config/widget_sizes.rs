use serde::Deserialize;

#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WidgetSizes {
    pub slider_width_px: f32,
    pub indicator_thickness_px: f32,
    pub tooltip_value_width_px: f32,
    pub workspace_preview_scale: f32,
    pub window_width_strip_scale: f32,
}

impl Default for WidgetSizes {
    fn default() -> Self {
        Self {
            slider_width_px: 60.0,
            indicator_thickness_px: 2.4,
            tooltip_value_width_px: 620.0,
            workspace_preview_scale: 0.19,
            window_width_strip_scale: 0.015,
        }
    }
}
