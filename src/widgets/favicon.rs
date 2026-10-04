use std::collections::HashMap;

use iced::{
    Element, Task,
    task::Handle,
    widget::{image, svg},
};
use niri_ipc::state::EventStreamState;

use crate::{
    data::source::favicons::{self, FaviconKind, RawFavicon},
    message_subscriptions::blocking,
};

#[derive(Clone)]
pub struct Favicon {
    handle: FaviconHandle,
}

#[derive(Clone)]
enum FaviconHandle {
    Raster(image::Handle),
    Svg(svg::Handle),
}

impl Favicon {
    fn new(raw: RawFavicon) -> Self {
        let handle = match raw.kind {
            FaviconKind::Raster => FaviconHandle::Raster(image::Handle::from_bytes(raw.bytes)),
            FaviconKind::Svg => FaviconHandle::Svg(svg::Handle::from_memory(raw.bytes)),
        };
        Self { handle }
    }

    pub fn view<'a, Message: 'a>(&self, side: f32, opacity: f32) -> Element<'a, Message> {
        match &self.handle {
            FaviconHandle::Raster(handle) => image(handle.clone())
                .width(side)
                .height(side)
                .opacity(opacity)
                .into(),
            FaviconHandle::Svg(handle) => svg(handle.clone())
                .width(side)
                .height(side)
                .opacity(opacity)
                .into(),
        }
    }
}

#[derive(Default)]
pub struct Favicons {
    cached: HashMap<String, Favicon>,
    titles: Vec<String>,
    poll: Option<Handle>,
}

impl Favicons {
    pub fn get(&self, key: &str) -> Option<&Favicon> {
        self.cached.get(key)
    }

    pub fn merge(&mut self, raw: HashMap<String, RawFavicon>) {
        self.cached.extend(
            raw.into_iter()
                .filter(|(title, _)| self.titles.binary_search(title).is_ok())
                .map(|(title, raw)| (title, Favicon::new(raw))),
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
