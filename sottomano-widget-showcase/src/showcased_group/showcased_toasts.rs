use corredo::theming::{background_color, primary_color, secondary_text_color, success_color};
use iced::{
    Color, Element, Font, Length, Padding, Theme, border,
    widget::{Column, Row, container, stack, text},
};
use sottomano_compositor_model::{
    compositor_window_facts::{TileViewPosition, WindowFocus},
    compositor_window_state::CompositorWindowState,
};
use sottomano_desktop_sources::desktop_notifications::notification_data::NotificationData;

use crate::showcase_message::ShowcaseInteraction;

const TERMINAL_PADDING_PX: f32 = 10.0;
const WINDOW_CORNER_RADIUS_PX: f32 = 8.0;
const FOCUS_RING_WIDTH_PX: f32 = 3.0;

pub struct ShowcasedToasts {
    pub compositor: CompositorWindowState,
    pub opening: Vec<NotificationData>,
    pub terminal_scrollback: &'static [TerminalLine],
}

pub enum TerminalLine {
    Prompt {
        directory: &'static str,
        command: &'static str,
    },
    Status {
        verb: &'static str,
        detail: &'static str,
    },
}

impl TerminalLine {
    fn view<'view>(&self) -> Element<'view, ShowcaseInteraction> {
        match self {
            TerminalLine::Prompt { directory, command } => {
                terminal_row(directory, primary_color, command)
            }
            TerminalLine::Status { verb, detail } => terminal_row(verb, success_color, detail),
        }
    }
}

fn terminal_row<'view>(
    lead: &'static str,
    lead_color: fn(&Theme) -> Color,
    rest: &'static str,
) -> Element<'view, ShowcaseInteraction> {
    Row::with_children([
        text(lead)
            .font(Font::MONOSPACE)
            .style(move |theme| text::Style {
                color: Some(lead_color(theme)),
            })
            .into(),
        text(rest).font(Font::MONOSPACE).style(text::base).into(),
    ])
    .into()
}

impl ShowcasedToasts {
    pub(super) fn window_frames(&self) -> Element<'_, ShowcaseInteraction> {
        let frames = self.compositor.windows.values().filter_map(|window| {
            let TileViewPosition::InView {
                horizontal,
                vertical,
            } = window.layout.tile_position_in_view
            else {
                return None;
            };
            let (width, height) = window.layout.window_size;
            let focus_ring: fn(&Theme) -> Color = match window.focus {
                WindowFocus::Focused => primary_color,
                WindowFocus::Unfocused => secondary_text_color,
            };
            let scrollback =
                Column::with_children(self.terminal_scrollback.iter().map(TerminalLine::view));
            let frame = container(scrollback)
                .padding(TERMINAL_PADDING_PX)
                .width(Length::Fixed(f32::from(
                    u16::try_from(width).expect("fixture window widths fit in u16"),
                )))
                .height(Length::Fixed(f32::from(
                    u16::try_from(height).expect("fixture window heights fit in u16"),
                )))
                .style(move |theme| container::Style {
                    background: Some(background_color(theme).into()),
                    border: border::rounded(WINDOW_CORNER_RADIUS_PX)
                        .color(focus_ring(theme))
                        .width(FOCUS_RING_WIDTH_PX),
                    ..container::Style::default()
                });
            Some(
                container(frame)
                    .padding(Padding::ZERO.top(vertical as f32).left(horizontal as f32))
                    .into(),
            )
        });
        stack(frames).into()
    }
}
