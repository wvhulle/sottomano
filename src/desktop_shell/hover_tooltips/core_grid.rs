use iced::{
    Border, Color, Element,
    widget::{Column, Row, Space, container},
};
use uom::si::ratio::ratio;

use crate::{
    config::Config,
    desktop_shell::Message,
    widgets::tooltip_body::{PhysicalCoreCells, ThreadCell},
};

pub struct CoreCellGrid<'a> {
    cores: &'a [PhysicalCoreCells],
    config: &'a Config,
}

impl<'a> CoreCellGrid<'a> {
    pub fn new(cores: &'a [PhysicalCoreCells], config: &'a Config) -> Self {
        Self { cores, config }
    }

    pub fn view(self) -> Element<'static, Message> {
        let gap = self.config.layout.overlay.row_spacing / 2.0;
        Row::with_children(self.cores.iter().map(|core| {
            Column::with_children(core.threads.iter().map(|thread| self.thread_cell(*thread)))
                .spacing(gap)
                .into()
        }))
        .spacing(gap)
        .into()
    }

    fn thread_cell(&self, thread: ThreadCell) -> Element<'static, Message> {
        let side = self.config.theme.font.small_text_size_px;
        let ink = thread.level.color(&self.config.theme.palette);
        let fill = thread.fill.get::<ratio>().clamp(0.0, 1.0) as f32;
        let dim_outline = Color {
            a: ink.a * 0.5,
            ..ink
        };
        container(Space::new().width(side).height(side))
            .style(move |_| container::Style {
                background: Some(
                    Color {
                        a: ink.a * fill,
                        ..ink
                    }
                    .into(),
                ),
                border: Border {
                    color: dim_outline,
                    width: 1.0,
                    radius: 2.0.into(),
                },
                ..container::Style::default()
            })
            .into()
    }
}
