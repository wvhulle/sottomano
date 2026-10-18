use iced::{
    Border, Color, Element, Length,
    widget::{Row, Space, container},
};
use uom::si::ratio::ratio;
use vetrata_config::Config;

use crate::{
    shell_message::Message,
    widgets::{
        palette_colors::PaletteColors,
        tooltip_body::{PhysicalCoreCells, ThreadCell},
        widget_state::WidgetState,
    },
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
    pub fn view(self) -> Element<'static, Message> {
        let gap = self.config.layout.overlay.row_spacing / 2.0;
        Row::with_children(self.cores.iter().map(|core| self.core_cell(core)))
            .spacing(gap)
            .into()
    }

    fn core_cell(&self, core: &PhysicalCoreCells) -> Element<'static, Message> {
        let side = self.config.theme.font.text_size_px;
        let outline = PaletteColors::from(&self.config.theme.palette).inactive;
        let thread_height = side - 2.0 * OUTLINE_PX;
        container(
            Row::with_children(
                core.threads
                    .iter()
                    .map(|thread| self.thread_column(*thread, thread_height)),
            )
            .spacing(OUTLINE_PX),
        )
        .padding(OUTLINE_PX)
        .width(side)
        .height(side)
        .style(move |_| container::Style {
            border: Border {
                color: outline,
                width: OUTLINE_PX,
                radius: 2.0.into(),
            },
            ..container::Style::default()
        })
        .into()
    }

    fn thread_column(&self, thread: ThreadCell, height: f32) -> Element<'static, Message> {
        let ink: Color =
            WidgetState::from(thread.level).color(&PaletteColors::from(&self.config.theme.palette));
        let fill = thread.fill.get::<ratio>().clamp(0.0, 1.0) as f32;
        container(
            container(Space::new().width(Length::Fill).height(height * fill)).style(move |_| {
                container::Style {
                    background: Some(ink.into()),
                    ..container::Style::default()
                }
            }),
        )
        .width(Length::Fill)
        .height(height)
        .align_bottom(height)
        .into()
    }
}
