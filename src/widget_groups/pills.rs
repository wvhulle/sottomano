use iced::{Alignment, Element, widget::Row};

use crate::{
    config::Config,
    data::source::BarWidget,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{pager_slot::permanently_inactive_slot, source_reading::source_reading},
};

pub struct Pills<'w> {
    entries: Vec<(&'w dyn BarWidget, HoverTarget)>,
    opacity: f32,
}

impl Default for Pills<'_> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            opacity: 1.0,
        }
    }
}

impl<'w> Pills<'w> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn push(mut self, widget: Option<&'w dyn BarWidget>, target: HoverTarget) -> Self {
        if let Some(widget) = widget {
            self.entries.push((widget, target));
        }
        self
    }

    pub fn row<'a, M: 'a>(
        self,
        config: &Config,
        wrap: impl Fn(TooltipMessage) -> M + Clone + 'a,
    ) -> Option<Row<'a, M>> {
        let Self { entries, opacity } = self;
        let pills: Vec<Element<'a, M>> = entries
            .into_iter()
            .filter_map(|(widget, target)| {
                let reading = source_reading(config, Some(widget), opacity)?;
                Some(hoverable(reading, target, wrap.clone()))
            })
            .collect();
        (!pills.is_empty()).then(|| {
            Row::with_children(pills)
                .spacing(config.spacing.widget_spacing_px)
                .align_y(Alignment::Center)
        })
    }

    pub fn slot<'a, M: 'a>(
        self,
        config: &Config,
        position: Position,
        wrap: impl Fn(TooltipMessage) -> M + Clone + 'a,
    ) -> Option<Element<'a, M>> {
        let row = self.row(config, wrap)?;
        Some(permanently_inactive_slot(row.into(), config, position))
    }
}
