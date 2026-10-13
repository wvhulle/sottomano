use std::time::SystemTime;

use iced::{
    Task,
    futures::{Stream, StreamExt, stream},
    task::Handle,
};
use tokio::time::sleep;

use crate::{
    config::Config,
    data::source::{Delivery, LiveSource, PolledSource, Source},
    widgets::tooltip_body::{Freshness, KeyValue, PollInfo, Readiness, Section, SourcedSection},
};

#[derive(Debug)]
pub struct OnHoverReading<S: Source> {
    latest: Option<Result<S, S::Error>>,
    shape: Option<S>,
    fetching: Option<Handle>,
    freshness: Option<Freshness>,
}

impl<S: Source> Default for OnHoverReading<S> {
    fn default() -> Self {
        Self {
            latest: None,
            shape: None,
            fetching: None,
            freshness: None,
        }
    }
}

impl<S: Source> OnHoverReading<S> {
    pub fn poll<Message: Send + 'static>(
        &mut self,
        key: S::Key,
        delivered: fn(Delivery<S>) -> Message,
        config: &Config,
    ) -> Task<Message>
    where
        S: PolledSource,
    {
        let freshness = Freshness::Polled(PollInfo {
            source: S::NAME,
            every: S::INTERVAL,
            last: SystemTime::now(),
        });
        self.start(S::polls(key), freshness, delivered, config)
    }

    pub fn listen<Message: Send + 'static>(
        &mut self,
        key: S::Key,
        delivered: fn(Delivery<S>) -> Message,
        config: &Config,
    ) -> Task<Message>
    where
        S: LiveSource,
    {
        self.start(S::events(key), Freshness::Live, delivered, config)
    }

    fn start<Message: Send + 'static>(
        &mut self,
        deliveries: impl Stream<Item = Delivery<S>> + Send + 'static,
        freshness: Freshness,
        delivered: fn(Delivery<S>) -> Message,
        config: &Config,
    ) -> Task<Message> {
        self.keep_shape();
        self.freshness = Some(freshness);
        let debug_delay = config.tooltips.loading.debug_delay;
        let delayed = stream::once(async move {
            sleep(debug_delay).await;
            deliveries
        })
        .flatten();
        let (task, fetching) = Task::run(delayed, delivered).abortable();
        self.fetching = Some(fetching.abort_on_drop());
        task
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

    pub fn receive(&mut self, delivery: Delivery<S>) {
        if self.fetching.is_none() {
            return;
        }
        match delivery {
            Delivery::Absent => {
                self.latest = None;
                self.shape = None;
            }
            Delivery::Reading(reading) => self.latest = Some(reading),
        }
        if let Some(Freshness::Polled(poll)) = &mut self.freshness {
            poll.last = SystemTime::now();
        }
    }

    pub fn sections(&self, sections: impl Fn(&S) -> Vec<Section>) -> Vec<SourcedSection> {
        let Some(freshness) = self.freshness else {
            return Vec::new();
        };
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
                failure.to_string(),
            )])],
        };
        unmarked
            .into_iter()
            .map(|section| SourcedSection::new(section, readiness, freshness))
            .collect()
    }
}
