//! The `layout` config group (matches odin's `LayoutConfig.qml`).

use serde::Deserialize;

/// Only the chrome-relevant knobs so far.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Layout {
    pub bg_opacity: f32,
    pub corner_radius: u16,
    pub vertical_padding: u16,
    pub horizontal_group_padding: u16,
    pub horizontal_widget_spacing: u16,
    pub horizontal_group_spacing: u16,
    pub floating: bool,
    pub blur: bool,
    pub shadow: bool,
    pub float_offset: f32,
    /// Thickness of the thin state bars (menu-group affordance, pager
    /// focus/urgent).
    pub focus_underline_thickness: f32,
    /// Thickness of the per-window tick segments under each workspace label.
    /// Thinner than the focus bar, so they read as secondary detail (odin's
    /// `windowSegmentThickness`).
    pub window_segment_thickness: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            bg_opacity: 0.70,
            corner_radius: 8,
            vertical_padding: 3,
            horizontal_group_padding: 6,
            horizontal_widget_spacing: 5,
            horizontal_group_spacing: 9,
            floating: true,
            blur: true,
            shadow: false,
            float_offset: 0.0,
            focus_underline_thickness: 3.4,
            window_segment_thickness: 2.0,
        }
    }
}
