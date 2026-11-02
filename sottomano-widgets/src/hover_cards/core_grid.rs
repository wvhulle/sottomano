use iced::{
    Border, Color, Element, Length,
    widget::{Row, container, progress_bar},
};
use sottomano_config::Config;
use uom::si::ratio::ratio;

use crate::{
    group_items::reading_display_state::ReadingDisplayState,
    hover_cards::{
        PopupMessage,
        hover_card_body::hover_card_figure_payloads::{PhysicalCoreCells, ThreadCell},
    },
    widgets::configured_theme::secondary_text_color,
};

const OUTLINE_PX: f32 = 1.0;

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
        let gap = self.config.layout.overlay.row_spacing / 2.0;
        Row::with_children(self.cores.iter().map(|core| self.core_cell(core)))
            .spacing(gap)
            .into()
    }

    fn core_cell(&self, core: &PhysicalCoreCells) -> Element<'static, PopupMessage> {
        let side = self.config.theme.font.text_size_px;
        let thread_height = side - 2.0 * OUTLINE_PX;
        container(
            Row::with_children(
                core.threads
                    .iter()
                    .map(|thread| thread_column(*thread, thread_height)),
            )
            .spacing(OUTLINE_PX),
        )
        .padding(OUTLINE_PX)
        .width(side)
        .height(side)
        .style(|theme| container::Style {
            border: Border {
                color: secondary_text_color(theme),
                width: OUTLINE_PX,
                radius: 2.0.into(),
            },
            ..container::Style::default()
        })
        .into()
    }
}

fn thread_column(thread: ThreadCell, height: f32) -> Element<'static, PopupMessage> {
    let state = ReadingDisplayState::from(thread.level);
    progress_bar(0.0..=1.0, thread.fill.get::<ratio>() as f32)
        .vertical()
        .length(height)
        .girth(Length::Fill)
        .style(move |theme| progress_bar::Style {
            background: Color::TRANSPARENT.into(),
            bar: state.color(theme).into(),
            border: Border::default(),
        })
        .into()
}
