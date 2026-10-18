use std::time::SystemTime;

use iced::{
    Task,
    futures::{StreamExt, stream},
    task::Handle,
};
use tokio::time::sleep;
use vetrata_config::Config;
use vetrata_sources::{ReadingSource, SourceDelivery};

use crate::widgets::tooltip_body::{Freshness, Readiness, Section};

#[derive(Debug)]
pub struct OnHoverReading<S: ReadingSource> {
    latest: Option<Result<S, S::Error>>,
    shape: Option<S>,
    fetching: Option<Handle>,
    freshness: Option<Freshness>,
}

impl<S: ReadingSource> Default for OnHoverReading<S> {
    fn default() -> Self {
        Self {
            latest: None,
            shape: None,
            fetching: None,
            freshness: None,
        }
    }
}

impl<S: ReadingSource> OnHoverReading<S> {
    pub fn start_hover_fetch<Message: Send + 'static>(
        &mut self,
        inputs: S::StreamInputs,
        delivered: fn(SourceDelivery<S>) -> Message,
        config: &Config,
    ) -> Task<Message> {
        self.keep_shape();
        self.freshness = Some(Freshness::from_source_cadence::<S>());
        let debug_delay = config.tooltips.loading.debug_delay;
        let delayed = stream::once(async move {
            sleep(debug_delay).await;
            S::delivery_stream(inputs)
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

    pub fn receive_delivery(&mut self, delivery: SourceDelivery<S>) {
        if self.fetching.is_none() {
            return;
        }
        match delivery {
            SourceDelivery::Absent => {
                self.latest = None;
                self.shape = None;
            }
            SourceDelivery::Reading(reading) => self.latest = Some(reading),
        }
        if let Some(Freshness::Polled { last, .. }) = &mut self.freshness {
            *last = SystemTime::now();
        }
    }

    pub fn reading(&self) -> Option<&S> {
        match &self.latest {
            Some(Ok(reading)) => Some(reading),
            Some(Err(_)) => None,
            None => self.shape.as_ref(),
        }
    }

    pub fn sections(&self, sections: impl Fn(&S) -> Vec<Section>) -> Vec<Section>
    where
        S::Error: Into<Section>,
    {
        let Some(freshness) = self.freshness else {
            return Vec::new();
        };
        let readiness = if self.latest.is_some() {
            Readiness::Settled
        } else {
            Readiness::Estimated
        };
        let unmarked = match &self.latest {
            None => self.shape.as_ref().map(&sections).unwrap_or_default(),
            Some(Ok(reading)) => sections(reading),
            Some(Err(failure)) => vec![failure.clone().into()],
        };
        unmarked
            .into_iter()
            .map(|section| section.stamped(readiness, freshness))
            .collect()
    }
}
