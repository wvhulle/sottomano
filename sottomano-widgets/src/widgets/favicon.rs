use std::collections::HashMap;

use iced::{Task, task::Handle};
use sottomano_desktop_sources::{
    blocking_background_work::blocking,
    compositor_window_ipc::compositor_window_state::CompositorWindowState,
    desktop_windows_apps::firefox_favicon_lookup::{self, RawFavicon},
};

use crate::widgets::icon_handle::IconHandle;

#[derive(Default)]
pub struct Favicons {
    cached: HashMap<String, IconHandle>,
    titles: Vec<String>,
    lookup: Lookup,
}

#[derive(Default)]
enum Lookup {
    #[default]
    Idle,
    Running {
        _aborts_on_drop: Handle,
        profile_changed_meanwhile: bool,
    },
}

impl Favicons {
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&IconHandle> {
        self.cached.get(key)
    }

    pub fn merge(&mut self, raw: HashMap<String, RawFavicon>) -> Task<HashMap<String, RawFavicon>> {
        self.cached.extend(
            raw.into_iter()
                .filter(|(title, _)| self.titles.binary_search(title).is_ok())
                .map(|(title, raw)| (title, IconHandle::from_bytes(raw.kind, raw.bytes))),
        );
        let rerun = matches!(
            self.lookup,
            Lookup::Running {
                profile_changed_meanwhile: true,
                ..
            }
        );
        self.lookup = Lookup::Idle;
        if rerun {
            self.retry_unresolved()
        } else {
            Task::none()
        }
    }

    pub fn track_titles(
        &mut self,
        compositor: &CompositorWindowState,
    ) -> Task<HashMap<String, RawFavicon>> {
        let titles = firefox_favicon_lookup::firefox_keys(compositor);
        if titles == self.titles {
            return Task::none();
        }
        self.cached
            .retain(|title, _| titles.binary_search(title).is_ok());
        self.titles = titles;
        self.lookup = Lookup::Idle;
        self.retry_unresolved()
    }

    pub fn retry_unresolved(&mut self) -> Task<HashMap<String, RawFavicon>> {
        if let Lookup::Running {
            profile_changed_meanwhile,
            ..
        } = &mut self.lookup
        {
            *profile_changed_meanwhile = true;
            return Task::none();
        }
        let unresolved: Vec<String> = self
            .titles
            .iter()
            .filter(|title| !self.cached.contains_key(*title))
            .cloned()
            .collect();
        if unresolved.is_empty() {
            return Task::none();
        }
        let (task, handle) = Task::future(blocking("favicons", move || {
            firefox_favicon_lookup::resolve(unresolved)
        }))
        .abortable();
        self.lookup = Lookup::Running {
            _aborts_on_drop: handle.abort_on_drop(),
            profile_changed_meanwhile: false,
        };
        task
    }
}
