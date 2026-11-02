pub mod activity_line;
pub mod aligned_quantity;
pub mod amount_text;
pub mod anchored_popup;
pub mod announced_level;
pub mod app_window_focus;
pub mod appearance_presets;
pub mod cascade_row;
pub mod child_view;
pub mod configured_theme;
pub mod confirm_button;
pub mod content_sized_table;
pub mod context_menu;
pub mod device_setting;
pub mod ellipsis_rule;
pub mod expand_animation;
pub mod fair_widths;
pub mod favicon;
pub mod gauge;
pub mod group_item;
pub mod group_item_reading;
pub mod group_menu;
pub mod hardware_section;
pub mod hover_fetched_snapshot;
pub mod huggingface_hub_sections;
pub mod icon_glyph;
pub mod icon_handle;
pub mod idle;
pub mod inline_slider;
pub mod input_report_sections;
pub mod karaoke_lyric_line;
pub mod keyed_record_table;
pub mod latest_snapshot;
pub mod layer_appearance_controls;
pub mod list_end_corners;
pub mod localized_durations;
pub mod localized_quantities;
pub mod localized_time;
pub mod marker_track;
pub mod markup_body;
pub mod most_used_model;
pub mod named_reading;
pub mod notched_panel;
pub mod offline_internet_reading;
pub mod ongoing_toast;
pub mod pager_slot;
pub mod panel_frame;
pub mod phosphor;
pub mod pointer_drag;
pub mod popup_connector;
pub mod pressable_text;
pub mod reading_display_state;
pub mod rectangle_morph;
pub mod refresh_dial;
pub mod region_reporter;
pub mod reveal;
pub mod rounded_quads;
pub mod shell_layer_appearance;
pub mod simple_icons;
pub mod slider_control;
pub mod source_reading;
pub mod spend_forecast_basis;
pub mod spend_to_level;
pub mod steady_label_width;
pub mod svg_icon;
pub mod text;
pub mod toggle_control;
pub mod tooltip_body;
pub mod transitioning_text;
pub mod uniform_width_column;
pub mod unit_switch;
pub mod value_cells;
pub mod vendor_logo;
pub mod weather_icons;
pub mod weekday_picker;
pub mod widest_readout_text;
pub mod widget_group;
pub mod window_icon;
pub mod window_list_viewport;
pub mod window_title;

pub const DISABLED_ALPHA: f32 = 0.4;

use iced::{Color, Element, Theme, widget::button};

use crate::widgets::icon_glyph::IconGlyph;

pub fn flat_button<'a>(
    text_color: impl Fn(&Theme) -> Color + 'a,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'a {
    move |theme, _| button::Style {
        background: None,
        text_color: text_color(theme),
        ..button::Style::default()
    }
}

pub fn glyph_button<'a, Message: Clone + 'a>(
    glyph: impl Into<IconGlyph>,
    size: f32,
    color: impl Fn(&Theme) -> Color + Copy + 'a,
    on_press: Message,
) -> Element<'a, Message> {
    button(glyph.into().view(size, color))
        .padding(0)
        .style(flat_button(color))
        .on_press(on_press)
        .into()
}
