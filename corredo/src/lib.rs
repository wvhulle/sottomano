pub mod primitives;

#[cfg(feature = "wgpu")]
use primitives::edge_alpha_fade;
use primitives::{
    activity_line, child_view, ellipsis_rule, fair_widths, flat_button, list_end_corners,
    popup_cascade, popup_connector, rectangle_morph, reveal, rounded_quads, scroll_steps,
    shared_metrics, uniform_width_column,
};

pub mod anchored_popup;
pub mod confirm_button;
pub mod content_sized_table;
pub mod context_menu;
pub mod fill_gauge;
pub mod font_faces;
pub mod frame_style;
pub mod inline_slider;
#[cfg(feature = "karaoke")]
pub mod karaoke_line;
pub mod marker_track;
#[cfg(feature = "markup")]
pub mod markup_body;
pub mod popup_trigger;
pub mod pressable_text;
pub mod record_table;
pub mod refresh_dial;
pub mod seek_bar;
#[cfg(feature = "wgpu")]
pub mod selection_scroller;
pub mod text_effects;
pub mod theme_colors;
pub mod toggle_switch;
pub mod transitioning_text;
#[cfg(feature = "chrono")]
pub mod weekday_picker;
pub mod widest_readout_text;
