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
    _fetching: Handle,
}

impl<S: Send + 'static> OnHoverReading<S> {
    pub fn fetch<E: Display, Message: Send + 'static>(
        readings: impl Stream<Item = Result<S, E>> + Send + 'static,
        delivered: fn(HoverFetched<S>) -> Message,
        config: &Config,
    ) -> (Self, Task<Message>) {
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
        (
            Self {
                latest: None,
                _fetching: fetching.abort_on_drop(),
            },
            task.map(delivered),
        )
    }

    pub fn receive(&mut self, fetched: HoverFetched<S>) {
        self.latest = Some(fetched.0);
    }

    pub fn section(&self, sections: impl FnOnce(&S) -> Vec<Section>) -> Section {
        match &self.latest {
            None => Section::pending(),
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
