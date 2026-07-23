//! A self-polling holder for one [`Source`] reading: owns the latest data,
//! its poll timer (cadence declared by the source itself via
//! [`Source::INTERVAL`]), and the off-thread poll task. Groups compose cells
//! and keep only their visuals.

use std::time::SystemTime;

use iced::{Subscription, Task};

use crate::{
    data_sources::{RefreshInfo, Source, Tooltip},
    message_subscriptions::poll,
};

pub struct SourceCell<S> {
    data: Option<S>,
    /// When the last poll completed, for the tooltip's staleness footer.
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

#[derive(Debug, Clone)]
pub enum SourceMessage<S> {
    Loaded(Option<S>),
    PollElapsed,
}

impl<S: Source + Send + 'static> SourceCell<S> {
    pub fn init() -> Task<SourceMessage<S>> {
        poll::<S>().map(SourceMessage::Loaded)
    }

    pub fn subscription() -> Subscription<SourceMessage<S>> {
        iced::time::every(S::INTERVAL).map(|_| SourceMessage::PollElapsed)
    }

    pub fn update(&mut self, msg: SourceMessage<S>) -> Task<SourceMessage<S>> {
        match msg {
            SourceMessage::Loaded(data) => {
                self.data = data;
                self.last_refreshed = Some(SystemTime::now());
                Task::none()
            }
            SourceMessage::PollElapsed => Self::init(),
        }
    }

    pub fn get(&self) -> Option<&S> {
        self.data.as_ref()
    }

    /// The tooltip for this cell's reading — its body plus a refresh footer
    /// carrying the cadence ([`Source::INTERVAL`]) and when it last polled.
    /// `None` until the first reading arrives.
    pub fn tooltip(&self) -> Option<Tooltip> {
        let data = self.data.as_ref()?;
        Some(Tooltip {
            body: data.tooltip(),
            footer: self.last_refreshed.map(|last| RefreshInfo {
                every: S::INTERVAL,
                last,
            }),
        })
    }
}
