use iced::{
    Alignment, Element,
    widget::{Row, button, mouse_area},
};

use crate::{
    config::Config,
    wayland_layers::tooltip::{HoverTarget, TooltipMessage, hoverable},
    widgets::{bar_widget::BarWidget, flat_button, source_reading::source_reading},
};

pub struct Reading<'w, M> {
    widget: Option<&'w dyn BarWidget>,
    target: HoverTarget,
    on_press: Option<M>,
    on_right_press: Option<M>,
}

impl<'w, M> Reading<'w, M> {
    pub fn new(widget: Option<&'w dyn BarWidget>, target: HoverTarget) -> Self {
        Self {
            widget,
            target,
            on_press: None,
            on_right_press: None,
        }
    }

    pub fn is_present(&self) -> bool {
        self.widget.is_some()
    }

    pub fn on_press(mut self, message: M) -> Self {
        self.on_press = Some(message);
        self
    }

    pub fn on_right_press(mut self, message: M) -> Self {
        self.on_right_press = Some(message);
        self
    }

    fn view<'a>(
        self,
        config: &Config,
        opacity: f32,
        wrap: impl Fn(TooltipMessage) -> M + 'a,
    ) -> Option<Element<'a, M>>
    where
        M: Clone + 'a,
    {
        let reading = source_reading(config, self.widget, opacity)?;
        let mut element = hoverable(reading, self.target, wrap);
        if let Some(message) = self.on_press {
            element = button(element)
                .padding(0)
                .style(flat_button(config.palette.bar_foreground))
                .on_press(message)
                .into();
        }
        if let Some(message) = self.on_right_press {
            element = mouse_area(element).on_right_press(message).into();
        }
        Some(element)
    }
}

pub struct ReadingRow<'w, M> {
    entries: Vec<Reading<'w, M>>,
    opacity: f32,
}

impl<M> Default for ReadingRow<'_, M> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            opacity: 1.0,
        }
    }
}

impl<'w, M> ReadingRow<'w, M> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn push(mut self, reading: Reading<'w, M>) -> Self {
        self.entries.push(reading);
        self
    }

    pub fn row<'a>(
        self,
        config: &Config,
        wrap: impl Fn(TooltipMessage) -> M + Clone + 'a,
    ) -> Option<Row<'a, M>>
    where
        M: Clone + 'a,
    {
        let Self { entries, opacity } = self;
        let readings: Vec<Element<'a, M>> = entries
            .into_iter()
            .filter_map(|entry| entry.view(config, opacity, wrap.clone()))
            .collect();
        (!readings.is_empty()).then(|| {
            Row::with_children(readings)
                .spacing(config.spacing.widget_spacing_px)
                .align_y(Alignment::Center)
        })
    }
}
