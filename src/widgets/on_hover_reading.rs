use std::fmt::Display;

use iced::{
    Task,
    futures::{Stream, StreamExt, stream},
    task::Handle,
};
use tokio::time::sleep;

use crate::{
    config::Config,
    widgets::tooltip_body::{KeyValue, Section},
};

#[derive(Debug, Clone)]
pub struct HoverFetched<S>(Result<S, String>);

#[derive(Debug)]
pub struct OnHoverReading<S> {
    latest: Option<Result<S, String>>,
    shape: Option<S>,
    fetching: Option<Handle>,
}

impl<S> Default for OnHoverReading<S> {
    fn default() -> Self {
        Self {
            latest: None,
            shape: None,
            fetching: None,
        }
    }
}

impl<S: Send + 'static> OnHoverReading<S> {
    pub fn fetch<E: Display, Message: Send + 'static>(
        &mut self,
        readings: impl Stream<Item = Result<S, E>> + Send + 'static,
        delivered: fn(HoverFetched<S>) -> Message,
        config: &Config,
    ) -> Task<Message> {
        self.keep_shape();
        let debug_delay = config.tooltips.loading.debug_delay;
        let delayed = stream::once(async move {
            sleep(debug_delay).await;
            readings
        })
        .flatten();
        let (task, fetching) = Task::run(delayed, |reading| {
            HoverFetched(reading.map_err(|failure| failure.to_string()))
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

    pub fn receive(&mut self, fetched: HoverFetched<S>) {
        if self.fetching.is_some() {
            self.latest = Some(fetched.0);
        }
    }

    pub fn section(&self, sections: impl Fn(&S) -> Vec<Section>) -> Section {
        match &self.latest {
            None => Section::estimated(self.shape.as_ref().map(&sections).unwrap_or_default()),
            Some(Ok(reading)) => Section::resolved(sections(reading)),
            Some(Err(failure)) => {
                Section::resolved(vec![Section::untitled_dictionary([KeyValue::new(
                    "Error",
                    failure.clone(),
                )])])
            }
        }
    }
}
