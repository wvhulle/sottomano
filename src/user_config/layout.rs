use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub corner_radius: u16,
    pub vertical_padding: f32,
    pub horizontal_group_padding: f32,
    pub horizontal_widget_spacing: f32,
    pub horizontal_group_spacing: f32,
    pub floating: bool,
    pub blur: bool,
    pub shadow: bool,
    pub float_offset: f32,
    pub outer_height: f32,
    pub focus_underline_thickness: f32,
    pub window_segment_thickness: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            corner_radius: 8,
            vertical_padding: 3.0,
            horizontal_group_padding: 6.0,
            horizontal_widget_spacing: 3.0,
            horizontal_group_spacing: 6.0,
            floating: true,
            blur: true,
            shadow: false,
            float_offset: 0.0,
            outer_height: 24.0,
            focus_underline_thickness: 3.4,
            window_segment_thickness: 2.0,
        }
    }
}
