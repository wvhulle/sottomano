pub mod activity_line;
pub mod anchored_popup;
pub mod bar_widget;
pub mod bounds_probe;
pub mod confirm_button;
pub mod context_menu;
pub mod copyable;
pub mod favicon;
pub mod focus_indicator;
pub mod font_awesome;
pub mod gauge;
pub mod idle;
pub mod inline_slider;
pub mod last_poll;
pub mod menu_group;
pub mod native_icon;
pub mod overlay_shadow;
pub mod pager_slot;
pub mod popup_connector;
pub mod progress_line;
pub mod reading_row;
pub mod reveal;
pub mod section_column;
pub mod simple_icons;
pub mod slider_control;
pub mod source_reading;
pub mod text;
pub mod toggle_control;
pub mod tooltip_body;
pub mod tracked;
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
