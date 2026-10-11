pub mod activity_line;
pub mod anchored_popup;
pub mod announced_level;
pub mod bar_widget;
pub mod confirm_button;
pub mod context_menu;
pub mod continuous_reading;
pub mod copyable;
pub mod device_setting;
pub mod expand_animation;
pub mod favicon;
pub mod focus_indicator;
pub mod gauge;
pub mod glyph_button;
pub mod group_item;
pub mod hardware_section;
pub mod idle;
pub mod inline_slider;
pub mod loading_text;
pub mod localized_time;
pub mod marker_track;
pub mod native_icon;
pub mod on_hover_reading;
pub mod pager_slot;
pub mod panel_frame;
pub mod phosphor;
pub mod popup_connector;
pub mod progress_line;
pub mod rate_text;
pub mod reading_alert;
pub mod region_reporter;
pub mod reveal;
pub mod section_column;
pub mod simple_icons;
pub mod slider_control;
pub mod source_reading;
pub mod svg_icon;
pub mod temperature_thresholds;
pub mod text;
pub mod toggle_control;
pub mod tooltip_body;
pub mod uniform_width_column;
pub mod weather_icons;
pub mod widest_readout_text;
pub mod widget_group;
pub mod widget_state;
pub mod window_icon;
pub mod window_list_viewport;
pub mod window_title;

pub const DISABLED_ALPHA: f32 = 0.4;

use iced::{Color, Theme, widget::button};

pub(crate) fn flat_button(text_color: Color) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, _| button::Style {
        background: None,
        text_color,
        ..button::Style::default()
    }
}
