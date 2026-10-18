use std::collections::HashMap;

use iced::{Task, task::Handle};
use niri_ipc::state::EventStreamState;

use crate::{
    data::source::favicons::{self, RawFavicon},
    message_subscriptions::blocking,
    widgets::icon_handle::IconHandle,
};

#[derive(Default)]
pub struct Favicons {
    cached: HashMap<String, IconHandle>,
    titles: Vec<String>,
    poll: Option<Handle>,
}

impl Favicons {
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&IconHandle> {
        self.cached.get(key)
    }

    pub fn merge(&mut self, raw: HashMap<String, RawFavicon>) {
        self.cached.extend(
            raw.into_iter()
                .filter(|(title, _)| self.titles.binary_search(title).is_ok())
                .map(|(title, raw)| (title, IconHandle::from_bytes(raw.kind, raw.bytes))),
        );
    }

    pub fn track_titles(
        &mut self,
        niri_state: &EventStreamState,
    ) -> Task<HashMap<String, RawFavicon>> {
        let titles = favicons::firefox_keys(niri_state);
        if titles == self.titles {
            return Task::none();
        }
        self.cached
            .retain(|title, _| titles.binary_search(title).is_ok());
        self.titles = titles;
        self.retry_unresolved()
    }

    pub fn retry_unresolved(&mut self) -> Task<HashMap<String, RawFavicon>> {
        let unresolved: Vec<String> = self
            .titles
            .iter()
            .filter(|title| !self.cached.contains_key(*title))
            .cloned()
            .collect();
        if unresolved.is_empty() {
            return Task::none();
        }
        let (task, handle) =
            blocking("favicons", move || favicons::resolve(unresolved)).abortable();
        self.poll = Some(handle.abort_on_drop());
        task
    }
}
