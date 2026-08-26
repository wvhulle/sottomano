use serde::Deserialize;

#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Widgets {
    pub slider_width_px: f32,
    pub focus_underline_thickness_px: f32,
    pub window_segment_thickness_px: f32,
    pub tooltip_value_width_px: f32,
    pub workspace_preview_scale: f32,
}

impl Default for Widgets {
    fn default() -> Self {
        Self {
            slider_width_px: 60.0,
            focus_underline_thickness_px: 3.4,
            window_segment_thickness_px: 2.0,
            tooltip_value_width_px: 620.0,
            workspace_preview_scale: 0.15,
        }
    }
}
