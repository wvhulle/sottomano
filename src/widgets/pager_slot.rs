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
        let edge_side = match self.ticks {
            Some(ticks) => container(ticks)
                .height(band)
                .align_y(Vertical::Center)
                .into(),
            None => Space::new().height(band).into(),
        };
        let (top, middle, bottom) = match position {
            Position::Bottom => (self.indicator, self.content, edge_side),
            Position::Top => (edge_side, self.content, self.indicator),
        };
        Column::with_children([top, middle, bottom])
            .width(Length::Shrink)
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
