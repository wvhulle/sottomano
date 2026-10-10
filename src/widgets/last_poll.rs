use std::{fmt::Display, time::SystemTime};

use crate::{
    data::source::Source,
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        group_item::GroupItem,
        tooltip_body::{KeyValue, RefreshInfo, Section, Tooltip, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct FailedReading {
    icon: Glyph,
    message: String,
}

impl BarWidget for FailedReading {
    fn icon(&self) -> Glyph {
        self.icon
    }

    fn state(&self) -> WidgetState {
        WidgetState::Failed
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[Section::untitled_dictionary([KeyValue::new(
            "Error",
            self.message.clone(),
        )])])
    }
}

pub struct LastPoll<S> {
    data: Option<S>,
    failure: Option<FailedReading>,
    last_refreshed: Option<SystemTime>,
}

impl<S> Default for LastPoll<S> {
    fn default() -> Self {
        Self {
            data: None,
            failure: None,
            last_refreshed: None,
        }
    }
}

impl<S> LastPoll<S> {
    pub fn load(&mut self, data: Option<S>) {
        self.data = data;
        self.failure = None;
        self.last_refreshed = Some(SystemTime::now());
    }

    pub fn get(&self) -> Option<&S> {
        self.data.as_ref()
    }
}

impl<S: BarWidget> LastPoll<S> {
    pub fn display(&self) -> Option<&dyn BarWidget> {
        match (self.data.as_ref(), self.failure.as_ref()) {
            (Some(data), _) => Some(data),
            (None, Some(failure)) => Some(failure),
            (None, None) => None,
        }
    }

    pub fn failure(&self) -> Option<&dyn BarWidget> {
        self.failure
            .as_ref()
            .map(|failure| failure as &dyn BarWidget)
    }

    pub fn reading_item<M: 'static>(&self, target: HoverTarget) -> Option<GroupItem<'_, M>> {
        self.display()
            .map(|widget| GroupItem::reading(widget, target))
    }

    pub fn live_tooltip(&self) -> Option<Tooltip> {
        Some(Tooltip::of(self.display()?.tooltip()))
    }

    pub fn receive<E: Display>(&mut self, reading: Result<S, E>) {
        self.last_refreshed = Some(SystemTime::now());
        match reading {
            Ok(data) => {
                self.data = Some(data);
                self.failure = None;
            }
            Err(failure) => {
                let icon = self
                    .data
                    .as_ref()
                    .map(BarWidget::icon)
                    .or_else(|| self.failure.as_ref().map(|failed| failed.icon))
                    .unwrap_or_else(S::fallback_icon);
                self.data = None;
                self.failure = Some(FailedReading {
                    icon,
                    message: failure.to_string(),
                });
            }
        }
    }
}

impl<S: Occasional> LastPoll<S> {
    pub fn present(&self) -> Option<&dyn BarWidget> {
        match (
            self.data.as_ref().filter(|data| data.present()),
            self.failure.as_ref(),
        ) {
            (Some(data), _) => Some(data),
            (None, Some(failure)) => Some(failure),
            (None, None) => None,
        }
    }
}

impl<S: Source + BarWidget> LastPoll<S> {
    pub fn tooltip(&self) -> Option<Tooltip> {
        self.tooltip_with(self.display()?.tooltip())
    }

    pub fn tooltip_with_sections(&self, extra: &[Section]) -> Option<Tooltip> {
        match self.display()?.tooltip() {
            TooltipBody::Sections(sections) => self.tooltip_with(TooltipBody::sections(
                &[sections.as_slice(), extra].concat(),
            )),
            other => self.tooltip_with(other),
        }
    }

    pub fn tooltip_with(&self, body: TooltipBody) -> Option<Tooltip> {
        self.display()?;
        let tooltip = Tooltip {
            footer: self.last_refreshed.map(|last| RefreshInfo {
                every: S::INTERVAL,
                last,
            }),
            ..Tooltip::of(body)
        };
        (!tooltip.is_empty()).then_some(tooltip)
    }
}
