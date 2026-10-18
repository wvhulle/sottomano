use iced::{
    Alignment, Element, Length, Padding,
    alignment::Vertical,
    widget::{Column, Row, Space, container},
};
use sottomano_config::{Config, screen_edges::Position};

pub struct PagerSlot<'a, Message> {
    content: Element<'a, Message>,
    indicator: Element<'a, Message>,
    ticks: Option<Element<'a, Message>>,
    gap_to_neighbours: f32,
}

impl<'a, Message: 'a> PagerSlot<'a, Message> {
    #[must_use]
    pub fn new(content: Element<'a, Message>, indicator: Element<'a, Message>) -> Self {
        Self {
            content,
            indicator,
            ticks: None,
            gap_to_neighbours: 0.0,
        }
    }

    #[must_use]
    pub fn ticks(mut self, ticks: Element<'a, Message>) -> Self {
        self.ticks = Some(ticks);
        self
    }

    #[must_use]
    pub fn spaced_by(mut self, gap_to_neighbours: f32) -> Self {
        self.gap_to_neighbours = gap_to_neighbours;
        self
    }

    #[must_use]
    pub fn view(self, config: &Config, position: Position) -> Element<'a, Message> {
        let band = config.panel_bar.indicator_thickness;
        let Self {
            content,
            indicator,
            ticks,
            gap_to_neighbours,
        } = self;
        let half_gap = gap_to_neighbours / 2.0;
        let share_of_gap = if position.is_vertical() {
            Padding::ZERO.top(half_gap).bottom(half_gap)
        } else {
            Padding::ZERO.left(half_gap).right(half_gap)
        };
        match position {
            Position::Bottom => stacked([
                indicator,
                stacked_label(content, share_of_gap),
                ticks_across(ticks, band, Vertical::Bottom),
            ]),
            Position::Top => stacked([
                ticks_across(ticks, band, Vertical::Top),
                stacked_label(content, share_of_gap),
                indicator,
            ]),
            Position::Left => Row::with_children([
                ticks.unwrap_or_else(|| Space::new().width(band).into()),
                stacked_label(content, share_of_gap),
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

fn stacked_label<'a, Message: 'a>(
    content: Element<'a, Message>,
    share_of_gap: Padding,
) -> Element<'a, Message> {
    container(content)
        .padding(share_of_gap)
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
