use iced::{Alignment, Element, widget::Row};

use crate::{
    config::Config,
    data::source::BarWidget,
    wayland_layers::{
        surface::Position,
        tooltip::{CarriesTooltip, HoverTarget, hoverable},
    },
    widgets::{icon_gauge::source_reading, pager_slot::permanently_inactive_slot},
};

pub struct Pills<'w> {
    entries: Vec<(&'w dyn BarWidget, HoverTarget)>,
    alpha: f32,
}

impl Default for Pills<'_> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            alpha: 1.0,
        }
    }
}

impl<'w> Pills<'w> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alpha(mut self, alpha: f32) -> Self {
        self.alpha = alpha;
        self
    }

    pub fn push<W: BarWidget>(mut self, widget: Option<&'w W>, target: HoverTarget) -> Self {
        if let Some(widget) = widget {
            self.entries.push((widget, target));
        }
        self
    }

    pub fn row<'a, M: CarriesTooltip + 'a>(self, config: &Config) -> Option<Row<'a, M>> {
        let Self { entries, alpha } = self;
        let pills: Vec<Element<'a, M>> = entries
            .into_iter()
            .filter_map(|(widget, target)| {
                let reading = source_reading(config, Some(widget), alpha)?;
                Some(hoverable(reading, target, M::from_tooltip))
            })
            .collect();
        (!pills.is_empty()).then(|| {
            Row::with_children(pills)
                .spacing(config.spacing.widget_spacing_px)
                .align_y(Alignment::Center)
        })
    }

    pub fn slot<'a, M: CarriesTooltip + 'a>(
        self,
        config: &Config,
        position: Position,
    ) -> Option<Element<'a, M>> {
        let row = self.row(config)?;
        Some(permanently_inactive_slot(row.into(), config, position))
    }
}
