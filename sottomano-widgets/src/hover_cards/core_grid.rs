use iced::{
    Border, Color, Element, Length,
    widget::{Row, container, progress_bar},
};
use sottomano_toml_config::Config;
use uom::si::ratio::ratio;

use crate::{
    group_items::reading_display_state::ReadingDisplayState,
    hover_cards::{
        PopupMessage,
        hover_card_body::hover_card_figure_payloads::{PhysicalCoreCells, ThreadCell},
    },
    widgets::{
        configured_theme::secondary_text_color,
        shared_metrics::{OUTLINE_WIDTH_PX, SMALL_CORNER_RADIUS_PX},
    },
};

pub struct CoreCellGrid<'a> {
    cores: &'a [PhysicalCoreCells],
    config: &'a Config,
}

impl<'a> CoreCellGrid<'a> {
    #[must_use]
    pub fn new(cores: &'a [PhysicalCoreCells], config: &'a Config) -> Self {
        Self { cores, config }
    }

    #[must_use]
    pub fn view(self) -> Element<'static, PopupMessage> {
        let gap = self.config.theme.spacing.item_gap;
        Row::with_children(self.cores.iter().map(|core| self.core_cell(core)))
            .spacing(gap)
            .into()
    }

    fn core_cell(&self, core: &PhysicalCoreCells) -> Element<'static, PopupMessage> {
        let side = self.config.theme.font.text_size_px;
        container(
            Row::with_children(core.threads.iter().map(|thread| thread_column(*thread)))
                .spacing(OUTLINE_WIDTH_PX)
                .height(Length::Fill),
        )
        .padding(OUTLINE_WIDTH_PX)
        .width(side)
        .height(side)
        .style(|theme| container::Style {
            border: Border {
                color: secondary_text_color(theme),
                width: OUTLINE_WIDTH_PX,
                radius: SMALL_CORNER_RADIUS_PX.into(),
            },
            ..container::Style::default()
        })
        .into()
    }
}

fn thread_column(thread: ThreadCell) -> Element<'static, PopupMessage> {
    let state = ReadingDisplayState::from(thread.level);
    progress_bar(0.0..=1.0, thread.fill.get::<ratio>() as f32)
        .vertical()
        .length(Length::Fill)
        .girth(Length::Fill)
        .style(move |theme| progress_bar::Style {
            background: Color::TRANSPARENT.into(),
            bar: state.color(theme).into(),
            border: Border::default(),
        })
        .into()
}
