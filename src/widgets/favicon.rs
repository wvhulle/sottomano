use std::{collections::HashMap, mem::take, sync::Arc};

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
    bytes: Arc<[u8]>,
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
            FaviconKind::Raster => {
                FaviconHandle::Raster(image::Handle::from_bytes(raw.bytes.clone()))
            }
            FaviconKind::Svg => FaviconHandle::Svg(svg::Handle::from_memory(raw.bytes.clone())),
        };
        Self {
            bytes: Arc::from(raw.bytes),
            handle,
        }
    }

    fn is(&self, raw: &RawFavicon) -> bool {
        *self.bytes == *raw.bytes
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
    poll: Option<Handle>,
}

impl Favicons {
    pub fn get(&self, key: &str) -> Option<&Favicon> {
        self.cached.get(key)
    }

    pub fn merge(&mut self, raw: HashMap<String, RawFavicon>) {
        let previous = take(&mut self.cached);
        self.cached = raw
            .into_iter()
            .map(|(title, raw)| {
                let favicon = match previous.get(&title) {
                    Some(existing) if existing.is(&raw) => existing.clone(),
                    _ => Favicon::new(raw),
                };
                (title, favicon)
            })
            .collect();
    }

    pub fn poll(&mut self, niri_state: &EventStreamState) -> Task<HashMap<String, RawFavicon>> {
        let keys = favicons::firefox_keys(niri_state);
        let (task, handle) = blocking("favicons", move || favicons::resolve(keys)).abortable();
        self.poll = Some(handle.abort_on_drop());
        task
    }
}
