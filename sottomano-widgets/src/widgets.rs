pub mod amount_text;
pub mod announced_level;
pub mod app_window_focus;
pub mod appearance_presets;
pub mod application_menu_lines;
pub mod configured_scroll;
pub mod configured_theme;
pub mod context_menu;
pub mod discovered_systemd_services;
pub mod favicon;
pub mod hardware_section;
pub mod hover_fetched_snapshot;
pub mod huggingface_hub_sections;
pub mod icon_glyph;
pub mod icon_handle;
pub mod idle;
pub mod input_report_sections;
pub mod keyed_record_table;
pub mod latest_snapshot;
pub mod layer_appearance_controls;
pub mod localized_durations;
pub mod localized_quantities;
pub mod localized_time;
pub mod most_used_model;
pub mod ongoing_toast;
pub mod phosphor;
pub mod pointer_drag;
pub mod program_brand_icons;
pub mod screen_edge;
pub mod shared_metrics;
pub mod shell_layer_appearance;
pub mod simple_icons;
pub mod slider_look;
pub mod spend_forecast_basis;
pub mod spend_to_level;
pub mod text;
pub mod typing_under_cards;
pub mod value_cells;
pub mod vendor_logo;
pub mod weather_icons;
pub mod window_icon;
pub mod window_title;

pub const DISABLED_ALPHA: f32 = 0.4;

use corredo::press_actions::flat_button;
use iced::{Color, Element, Theme, widget::button};

use crate::widgets::icon_glyph::IconGlyph;

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
