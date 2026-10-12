use std::{fmt::Display, time::SystemTime};

use iced::{
    Task,
    futures::{StreamExt, stream},
    task::Handle,
};
use tokio::time::sleep;

use crate::{
    config::Config,
    message_subscriptions::{Cadence, Readings},
    widgets::tooltip_body::{KeyValue, Readiness, RefreshInfo, Section},
};

#[derive(Debug)]
pub struct OnHoverReading<S> {
    latest: Option<Result<S, String>>,
    shape: Option<S>,
    fetching: Option<Handle>,
    cadence: Cadence,
    fetch_started: Option<SystemTime>,
}

impl<S> Default for OnHoverReading<S> {
    fn default() -> Self {
        Self {
            latest: None,
            shape: None,
            fetching: None,
            cadence: Cadence::Once,
            fetch_started: None,
        }
    }
}

impl<S: Send + 'static> OnHoverReading<S> {
    pub fn fetch<E: Display + 'static, Message: Send + 'static>(
        &mut self,
        readings: Readings<S, E>,
        delivered: fn(Result<S, String>) -> Message,
        config: &Config,
    ) -> Task<Message> {
        self.keep_shape();
        self.cadence = readings.cadence;
        self.fetch_started = Some(SystemTime::now());
        let debug_delay = config.tooltips.loading.debug_delay;
        let delayed = stream::once(async move {
            sleep(debug_delay).await;
            readings.stream
        })
        .flatten();
        let (task, fetching) = Task::run(delayed, |reading| {
            reading.map_err(|failure| failure.to_string())
        })
        .abortable();
        self.fetching = Some(fetching.abort_on_drop());
        task.map(delivered)
    }

    pub fn stop(&mut self) {
        self.fetching = None;
        self.keep_shape();
    }

    fn keep_shape(&mut self) {
        if let Some(Ok(reading)) = self.latest.take() {
            self.shape = Some(reading);
        }
    }

    pub fn receive(&mut self, fetched: Result<S, String>) {
        if self.fetching.is_some() {
            self.latest = Some(fetched);
            self.fetch_started = Some(SystemTime::now());
        }
    }

    fn refresh_info(&self) -> Option<RefreshInfo> {
        let Cadence::Periodic { source, every } = self.cadence else {
            return None;
        };
        self.fetch_started.map(|last| RefreshInfo {
            source,
            every,
            last,
        })
    }

    pub fn sections(&self, sections: impl Fn(&S) -> Vec<Section>) -> Vec<Section> {
        let readiness = if self.latest.is_some() {
            Readiness::Arriving
        } else {
            Readiness::Estimated
        };
        let unmarked = match &self.latest {
            None => self.shape.as_ref().map(&sections).unwrap_or_default(),
            Some(Ok(reading)) => sections(reading),
            Some(Err(failure)) => vec![Section::dictionary([KeyValue::new(
                "Error",
                failure.clone(),
            )])],
        };
        let refresh = self.refresh_info();
        unmarked
            .into_iter()
            .map(|section| section.with_readiness(readiness).with_refresh(refresh))
            .collect()
    }
}
