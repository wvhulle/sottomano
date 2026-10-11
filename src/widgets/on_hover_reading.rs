use std::{fmt::Display, future::ready, hash::Hash};

use iced::{
    Subscription,
    futures::{Stream, StreamExt, stream},
};

use crate::widgets::tooltip_body::{KeyValue, Section};

#[derive(Debug, Clone)]
pub struct OnHoverReading<Subject, S> {
    subject: Subject,
    latest: Option<Result<S, String>>,
}

impl<Subject, S> OnHoverReading<Subject, S>
where
    Subject: PartialEq + Hash + Clone + Send + Sync + 'static,
    S: Send + 'static,
{
    pub fn subscription<Readings, E>(
        subject: Subject,
        readings: fn(&Subject) -> Readings,
    ) -> Subscription<Self>
    where
        Readings: Stream<Item = Result<S, E>> + Send + 'static,
        E: Display,
    {
        Subscription::run_with((subject, readings), |(subject, readings)| {
            let fetched_for = subject.clone();
            stream::once(ready(Self {
                subject: subject.clone(),
                latest: None,
            }))
            .chain(readings(subject).map(move |reading| Self {
                subject: fetched_for.clone(),
                latest: Some(reading.map_err(|failure| failure.to_string())),
            }))
        })
    }

    pub fn section(
        reading: Option<&Self>,
        subject: &Subject,
        sections: impl FnOnce(&S) -> Vec<Section>,
    ) -> Section {
        let latest = reading
            .filter(|reading| &reading.subject == subject)
            .and_then(|reading| reading.latest.as_ref());
        match latest {
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
