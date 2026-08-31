use iced::{
    Alignment, Element, Length,
    alignment::Vertical,
    widget::{Column, Space, container},
};

use crate::{
    config::Config, wayland_layers::surface::Position, widgets::activity_line::activity_line,
};

pub struct PagerSlot<'a, Message> {
    content: Element<'a, Message>,
    indicator: Element<'a, Message>,
    ticks: Option<Element<'a, Message>>,
}

impl<'a, Message: 'a> PagerSlot<'a, Message> {
    pub fn new(content: Element<'a, Message>, indicator: Element<'a, Message>) -> Self {
        Self {
            content,
            indicator,
            ticks: None,
        }
    }

    #[must_use]
    pub fn ticks(mut self, ticks: Element<'a, Message>) -> Self {
        self.ticks = Some(ticks);
        self
    }

    pub fn view(self, config: &Config, position: Position) -> Element<'a, Message> {
        let band = config
            .widgets
            .focus_underline_thickness_px
            .max(config.widgets.window_segment_thickness_px);
        let ticks_against = |edge: Vertical| -> Element<'a, Message> {
            self.ticks.map_or_else(
                || Space::new().height(band).into(),
                |ticks| container(ticks).height(band).align_y(edge).into(),
            )
        };
        let label = container(self.content)
            .height(Length::Fill)
            .align_y(Vertical::Center);
        let (top, bottom) = match position {
            Position::Bottom => (self.indicator, ticks_against(Vertical::Bottom)),
            Position::Top => (ticks_against(Vertical::Top), self.indicator),
        };
        Column::with_children([top, label.into(), bottom])
            .width(Length::Shrink)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .into()
    }
}

pub fn permanently_inactive_slot<'a, Message: 'a>(
    content: Element<'a, Message>,
    config: &Config,
    position: Position,
) -> Element<'a, Message> {
    let inactive_indicator = activity_line(
        config.palette.focus,
        0.0,
        config.widgets.focus_underline_thickness_px,
    );
    PagerSlot::new(content, inactive_indicator).view(config, position)
}
