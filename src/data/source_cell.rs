use std::{
    fmt::Display,
    time::{Duration, SystemTime},
};

use iced::Subscription;

use crate::{
    data::source::{
        BarWidget, Glyph, Level, RefreshInfo, Source, Tooltip, TooltipBody, font_awesome::Solid,
        row,
    },
    message_subscriptions::polling,
};

pub struct FailedReading {
    icon: Option<Glyph>,
    message: String,
}

impl BarWidget for FailedReading {
    fn icon(&self) -> Glyph {
        self.icon
            .unwrap_or_else(|| Solid::TriangleExclamation.into())
    }

    fn level(&self) -> Level {
        Level::Critical
    }

    fn failed(&self) -> bool {
        true
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(vec![row("Error", self.message.clone())])
    }
}

impl<S: BarWidget> SourceCell<S> {
    pub fn live_tooltip(&self) -> Option<Tooltip> {
        Some(Tooltip::of(self.data.as_ref()?.tooltip()))
    }

    pub fn display(&self) -> Option<&dyn BarWidget> {
        match (self.data.as_ref(), self.failure.as_ref()) {
            (Some(data), _) => Some(data),
            (None, Some(failure)) => Some(failure),
            (None, None) => None,
        }
    }
}

pub struct SourceCell<S> {
    data: Option<S>,
    failure: Option<FailedReading>,
    last_refreshed: Option<SystemTime>,
}

impl<S> Default for SourceCell<S> {
    fn default() -> Self {
        Self {
            data: None,
            failure: None,
            last_refreshed: None,
        }
    }
}

impl<S> SourceCell<S> {
    pub fn load(&mut self, data: Option<S>) {
        self.data = data;
        self.failure = None;
        self.last_refreshed = Some(SystemTime::now());
    }

    pub fn get(&self) -> Option<&S> {
        self.data.as_ref()
    }

    pub fn footer(&self, every: Duration) -> Option<RefreshInfo> {
        self.data.as_ref()?;
        self.last_refreshed.map(|last| RefreshInfo { every, last })
    }
}

impl<S: BarWidget> SourceCell<S> {
    pub fn receive<E: Display>(&mut self, reading: Result<Option<S>, E>) {
        self.last_refreshed = Some(SystemTime::now());
        match reading {
            Ok(data) => {
                self.data = data;
                self.failure = None;
            }
            Err(failure) => {
                let icon = self
                    .data
                    .as_ref()
                    .map(BarWidget::icon)
                    .or_else(|| self.failure.as_ref().and_then(|failed| failed.icon));
                self.data = None;
                self.failure = Some(FailedReading {
                    icon,
                    message: failure.to_string(),
                });
            }
        }
    }
}

impl<S: Source> SourceCell<S> {
    pub fn subscription() -> Subscription<Result<Option<S>, S::Error>> {
        polling::<S>()
    }

    pub fn tooltip(&self) -> Option<Tooltip> {
        self.tooltip_with(self.display()?.tooltip())
    }

    pub fn tooltip_with(&self, body: TooltipBody) -> Option<Tooltip> {
        self.display()?;
        Some(Tooltip {
            body,
            footer: self.last_refreshed.map(|last| RefreshInfo {
                every: S::INTERVAL,
                last,
            }),
        })
    }
}
