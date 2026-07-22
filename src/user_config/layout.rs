use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub corner_radius_px: u16,
    pub vertical_padding_px: f32,
    pub horizontal_group_padding_px: f32,
    pub horizontal_widget_spacing_px: f32,
    pub horizontal_group_spacing_px: f32,
    pub floating: bool,
    pub blur: bool,
    pub shadow: bool,
    pub float_offset_px: f32,
    /// Track width of a settings slider (e.g. the panel-settings float-offset
    /// knob). Chrome of the session-only settings UI, not a bar element.
    pub slider_width_px: f32,
    pub outer_height_px: f32,
    pub focus_underline_thickness_px: f32,
    pub window_segment_thickness_px: f32,
    pub menu_expand_ms: u64,
    /// Corner radius applied to window-list icons, as a fraction of the icon
    /// side — `0.0` (the default) leaves them square. Applied uniformly to
    /// every icon regardless of source (app, theme, or favicon).
    pub icon_corner_fraction: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            corner_radius_px: 8,
            vertical_padding_px: 3.0,
            horizontal_group_padding_px: 5.0,
            horizontal_widget_spacing_px: 3.0,
            horizontal_group_spacing_px: 16.0,
            floating: true,
            blur: true,
            shadow: false,
            float_offset_px: 0.0,
            slider_width_px: 60.0,
            outer_height_px: 28.0,
            focus_underline_thickness_px: 3.4,
            window_segment_thickness_px: 2.0,
            menu_expand_ms: 90,
            icon_corner_fraction: 0.0,
        }
    }
}
