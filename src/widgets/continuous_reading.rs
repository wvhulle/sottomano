use std::{fmt::Display, time::SystemTime};

use crate::{
    data::source::Source,
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        group_item::GroupItem,
        tooltip_body::{KeyValue, Refresh, RefreshInfo, Section, Tooltip, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct FailedReading {
    icon: Glyph,
    message: String,
}

impl BarWidget for FailedReading {
    fn name(&self) -> &'static str {
        "Failed reading"
    }

    fn icon(&self) -> Glyph {
        self.icon
    }

    fn state(&self) -> WidgetState {
        WidgetState::Failed
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::dictionary([KeyValue::new("Error", self.message.clone())])
    }
}

pub struct ContinuousReading<S> {
    data: Option<S>,
    failure: Option<FailedReading>,
    refresh: Option<Refresh>,
}

impl<S> Default for ContinuousReading<S> {
    fn default() -> Self {
        Self {
            data: None,
            failure: None,
            refresh: None,
        }
    }
}

impl<S> ContinuousReading<S> {
    pub fn load(&mut self, data: S) {
        self.data = Some(data);
        self.failure = None;
        self.refresh = Some(Refresh::Live);
    }

    pub fn get(&self) -> Option<&S> {
        self.data.as_ref()
    }
}

impl<S: BarWidget> ContinuousReading<S> {
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

    pub fn group_item<M: 'static>(
        &self,
        target: impl Into<HoverTarget>,
    ) -> Option<GroupItem<'_, M>> {
        self.display()
            .map(|widget| GroupItem::reading(widget, target))
    }

    pub fn tooltip(&self) -> Option<Tooltip> {
        self.tooltip_with(self.display()?.tooltip())
    }

    pub fn tooltip_with_sections(
        &self,
        extra: impl IntoIterator<Item = Section>,
    ) -> Option<Tooltip> {
        let own = self
            .display()?
            .tooltip()
            .with_unmarked_refresh(self.refresh);
        self.tooltip_with(own.with_sections(extra))
    }

    pub fn tooltip_with(&self, body: TooltipBody) -> Option<Tooltip> {
        self.display()?;
        let tooltip = Tooltip::of(body.with_unmarked_refresh(self.refresh));
        (!tooltip.is_empty()).then_some(tooltip)
    }

    pub fn push<E: Display>(&mut self, reading: Result<S, E>) {
        self.refresh = Some(Refresh::Live);
        self.accept(reading);
    }

    fn accept<E: Display>(&mut self, reading: Result<S, E>) {
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

impl<S: Occasional> ContinuousReading<S> {
    pub fn present_reading(&self) -> Option<&S> {
        self.data.as_ref().filter(|data| data.present())
    }

    pub fn present(&self) -> Option<&dyn BarWidget> {
        match (self.present_reading(), self.failure.as_ref()) {
            (Some(data), _) => Some(data),
            (None, Some(failure)) => Some(failure),
            (None, None) => None,
        }
    }

    pub fn present_group_item<M: 'static>(
        &self,
        target: impl Into<HoverTarget>,
    ) -> Option<GroupItem<'_, M>> {
        self.present()
            .map(|widget| GroupItem::reading(widget, target))
    }

    pub fn present_tooltip(&self) -> Option<Tooltip> {
        self.tooltip_with(self.present_reading()?.tooltip())
    }
}

impl<S: Source + BarWidget> ContinuousReading<S> {
    pub fn receive<E: Display>(&mut self, reading: Result<S, E>) {
        self.refresh = Some(Refresh::Periodic(RefreshInfo {
            source: S::NAME,
            every: S::INTERVAL,
            last: SystemTime::now(),
        }));
        self.accept(reading);
    }
}
