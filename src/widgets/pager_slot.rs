use iced::{Alignment, Element, Length, widget::Column};

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

    pub fn view(self, position: Position) -> Element<'a, Message> {
        let (top, bottom) = match position {
            Position::Bottom => (Some(self.indicator), self.ticks),
            Position::Top => (self.ticks, Some(self.indicator)),
        };
        let stacked = [top, Some(self.content), bottom].into_iter().flatten();
        Column::with_children(stacked)
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
    PagerSlot::new(content, inactive_indicator).view(position)
}
