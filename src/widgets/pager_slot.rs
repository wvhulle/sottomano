use iced::{
    Alignment, Element, Length,
    alignment::Vertical,
    widget::{Column, Row, Space, container},
};

use crate::{config::Config, desktop_shell::monitor_surfaces::Position};

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
        let band = config.panel_bar.indicator_thickness_px;
        let Self {
            content,
            indicator,
            ticks,
        } = self;
        match position {
            Position::Bottom => stacked([
                indicator,
                stacked_label(content),
                ticks_across(ticks, band, Vertical::Bottom),
            ]),
            Position::Top => stacked([
                ticks_across(ticks, band, Vertical::Top),
                stacked_label(content),
                indicator,
            ]),
            Position::Left => Row::with_children([
                ticks.unwrap_or_else(|| Space::new().width(band).into()),
                stacked_label(content),
                indicator,
            ])
            .into(),
        }
    }
}

fn stacked<'a, Message: 'a>(bands: [Element<'a, Message>; 3]) -> Element<'a, Message> {
    Column::with_children(bands)
        .width(Length::Shrink)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .into()
}

fn stacked_label<'a, Message: 'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    container(content)
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
