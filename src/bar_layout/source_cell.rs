//! A holder for one [`Source`] reading: owns the latest data and delegates
//! its refresh cadence to the [`polling`] stream (declared by the source
//! itself via [`Source::INTERVAL`]). Groups compose cells and keep only their
//! visuals.

use std::time::SystemTime;

use iced::Subscription;

use crate::{
    data_sources::{RefreshInfo, Source, Tooltip},
    message_subscriptions::polling,
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

impl<S: Source + Send + 'static> SourceCell<S> {
    /// The cell's readings: an immediate first poll (the initial load), then
    /// one per [`Source::INTERVAL`]. `None` clears the reading — a poll that
    /// finds no sensor takes the cell off the bar rather than freezing it.
    pub fn subscription() -> Subscription<Option<S>> {
        polling::<S>()
    }

    pub fn load(&mut self, data: Option<S>) {
        self.data = data;
        self.last_refreshed = Some(SystemTime::now());
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
