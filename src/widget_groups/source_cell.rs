use std::time::{Duration, SystemTime};

use iced::Subscription;

use crate::{
    data::source::{BarWidget, RefreshInfo, Source, Tooltip, TooltipBody},
    message_subscriptions::polling,
};

impl<S: BarWidget> SourceCell<S> {
    pub fn live_tooltip(&self) -> Option<Tooltip> {
        Some(Tooltip::of(self.data.as_ref()?.tooltip()))
    }
}

pub struct SourceCell<S> {
    data: Option<S>,
    last_refreshed: Option<SystemTime>,
}

impl<S> Default for SourceCell<S> {
    fn default() -> Self {
        Self {
            data: None,
            last_refreshed: None,
        }
    }
}

impl<S> SourceCell<S> {
    pub fn load(&mut self, data: Option<S>) {
        self.data = data;
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

impl<S: Source + Send + 'static> SourceCell<S> {
    pub fn subscription() -> Subscription<Option<S>> {
        polling::<S>()
    }

    pub fn tooltip(&self) -> Option<Tooltip> {
        self.tooltip_with(self.data.as_ref()?.tooltip())
    }

    pub fn tooltip_with(&self, body: TooltipBody) -> Option<Tooltip> {
        self.data.as_ref()?;
        Some(Tooltip {
            body,
            footer: self.last_refreshed.map(|last| RefreshInfo {
                every: S::INTERVAL,
                last,
            }),
        })
    }
}
