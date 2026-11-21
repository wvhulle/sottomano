use std::convert::identity;

use corredo::popovers::NestedMenuOpenTrigger;
use iced::{
    Alignment, Element, Length, Padding,
    alignment::Vertical,
    widget::{Column, Row, Space, container},
};
use sottomano_toml_config::{Config, screen_edges::Position};

use crate::{
    hover_cards::{HoverCardMessage, HoverTarget, hoverable},
    widgets::shared_metrics::ACCENT_STROKE_PX,
};

pub(crate) fn dragged_slot_content<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    config: &Config,
) -> Element<'a, Message> {
    container(content)
        .padding([0.0, config.theme.spacing.item_gap])
        .center_y(config.desktop_panels.height)
        .into()
}

pub struct PanelSlot<'a, Message> {
    content: Element<'a, Message>,
    indicator: Element<'a, Message>,
    ticks: Option<Element<'a, Message>>,
    along_panel_padding: f32,
}

impl<'a, Message: 'a> PanelSlot<'a, Message> {
    #[must_use]
    pub fn new(content: Element<'a, Message>, indicator: Element<'a, Message>) -> Self {
        Self {
            content,
            indicator,
            ticks: None,
            along_panel_padding: 0.0,
        }
    }

    #[must_use]
    pub fn ticks(mut self, ticks: Element<'a, Message>) -> Self {
        self.ticks = Some(ticks);
        self
    }

    #[must_use]
    pub fn padded_along_panel(mut self, along_panel_padding: f32) -> Self {
        self.along_panel_padding = along_panel_padding;
        self
    }

    #[must_use]
    pub fn view(self, position: Position) -> Element<'a, Message> {
        self.arranged(position, identity)
    }

    #[must_use]
    pub fn hovered(
        self,
        position: Position,
        opening: NestedMenuOpenTrigger,
        target: HoverTarget,
        wrap: impl Fn(HoverCardMessage) -> Message + 'a,
    ) -> Element<'a, Message> {
        self.arranged(position, |label_with_edge_line| {
            hoverable(opening, label_with_edge_line, target, wrap)
        })
    }

    fn arranged(
        self,
        position: Position,
        wrap_target: impl FnOnce(Element<'a, Message>) -> Element<'a, Message>,
    ) -> Element<'a, Message> {
        let band = ACCENT_STROKE_PX;
        let Self {
            content,
            indicator,
            ticks,
            along_panel_padding,
        } = self;
        match position {
            Position::Bottom => stacked([
                wrap_target(stacked([
                    indicator,
                    label_across_bar(content, along_panel_padding),
                ])),
                ticks_across(ticks, band, Vertical::Bottom),
            ]),
            Position::Top => stacked([
                ticks_across(ticks, band, Vertical::Top),
                wrap_target(stacked([
                    label_across_bar(content, along_panel_padding),
                    indicator,
                ])),
            ]),
            Position::Left => Row::with_children([
                ticks.unwrap_or_else(|| Space::new().width(band).into()),
                wrap_target(
                    Row::with_children([
                        container(content)
                            .padding(Padding::ZERO.vertical(along_panel_padding))
                            .into(),
                        indicator,
                    ])
                    .align_y(Alignment::Center)
                    .into(),
                ),
            ])
            .align_y(Alignment::Center)
            .into(),
        }
    }
}

fn stacked<'a, Message: 'a, const BAND_COUNT: usize>(
    bands: [Element<'a, Message>; BAND_COUNT],
) -> Element<'a, Message> {
    Column::with_children(bands)
        .width(Length::Shrink)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .into()
}

fn label_across_bar<'a, Message: 'a>(
    content: Element<'a, Message>,
    along_panel_padding: f32,
) -> Element<'a, Message> {
    container(content)
        .padding(Padding::ZERO.horizontal(along_panel_padding))
        .height(Length::Fill)
        .align_y(Vertical::Center)
        .into()
}

fn ticks_across<'a, Message: 'a>(
    ticks: Option<Element<'a, Message>>,
    band: f32,
    edge: Vertical,
) -> Element<'a, Message> {
    ticks.map_or_else(
        || Space::new().height(band).into(),
        |ticks| container(ticks).height(band).align_y(edge).into(),
    )
}

#[cfg(test)]
mod tests {
    use corredo::popovers::NestedMenuOpenTrigger;
    use iced::{
        Length, Point, Rectangle, Settings, Size,
        mouse::Event as MouseEvent,
        widget::{Space, container},
    };
    use iced_test::Simulator;
    use sottomano_toml_config::screen_edges::Position;

    use super::PanelSlot;
    use crate::{
        hover_cards::{AnchorEvent, HoverCardMessage, HoverTarget},
        widgets::shared_metrics::ACCENT_STROKE_PX,
    };

    const ICON_SIDE_PX: f32 = 20.0;
    const BAR_THICKNESS_PX: f32 = 30.0;

    fn reported_anchor_in_bar(position: Position) -> Rectangle {
        let (edge_line, band_size) = match position {
            Position::Top | Position::Bottom => (
                Space::new().width(Length::Fill).height(ACCENT_STROKE_PX),
                Size::new(200.0, BAR_THICKNESS_PX),
            ),
            Position::Left => (
                Space::new().width(ACCENT_STROKE_PX).height(Length::Fill),
                Size::new(BAR_THICKNESS_PX, 200.0),
            ),
        };
        let icon = Space::new().width(ICON_SIDE_PX).height(ICON_SIDE_PX);
        let slot = PanelSlot::new(icon.into(), edge_line.into()).hovered(
            position,
            NestedMenuOpenTrigger::Hover,
            HoverTarget::Workspace(3),
            |message| message,
        );
        let mut bar = Simulator::with_size(
            Settings::default(),
            band_size,
            container(slot)
                .width(band_size.width)
                .height(band_size.height),
        );
        let inside_icon = Point::new(
            band_size.width.min(ICON_SIDE_PX) / 2.0,
            band_size.height / 2.0,
        );
        bar.point_at(inside_icon);
        let _moved = bar.simulate([iced::Event::Mouse(MouseEvent::CursorMoved {
            position: inside_icon,
        })]);
        bar.into_messages()
            .find_map(|message| match message {
                HoverCardMessage::Anchor(AnchorEvent::LaidOut(widget_anchor)) => {
                    Some(widget_anchor.anchor)
                }
                _ => None,
            })
            .expect("a hovered slot reports its anchor once laid out")
    }

    #[allow(clippy::float_cmp)]
    #[test]
    fn top_bar_target_ends_at_the_bar_edge_below_the_edge_line() {
        let anchor = reported_anchor_in_bar(Position::Top);
        assert_eq!(anchor.y, ACCENT_STROKE_PX);
        assert_eq!(anchor.y + anchor.height, BAR_THICKNESS_PX);
    }

    #[allow(clippy::float_cmp)]
    #[test]
    fn bottom_bar_target_starts_at_the_bar_edge_above_the_edge_line() {
        let anchor = reported_anchor_in_bar(Position::Bottom);
        assert_eq!(anchor.y, 0.0);
        assert_eq!(
            anchor.y + anchor.height,
            BAR_THICKNESS_PX - ACCENT_STROKE_PX
        );
    }

    #[allow(clippy::float_cmp)]
    #[test]
    fn sidebar_target_ends_right_of_its_edge_line_and_excludes_the_ticks() {
        let anchor = reported_anchor_in_bar(Position::Left);
        assert_eq!(anchor.x, ACCENT_STROKE_PX);
        assert_eq!(
            anchor.x + anchor.width,
            ACCENT_STROKE_PX + ICON_SIDE_PX + ACCENT_STROKE_PX
        );
    }
}
