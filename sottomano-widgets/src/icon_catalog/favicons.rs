use std::collections::HashMap;

use iced::{Task, task::Handle};
use sottomano_compositor_model::compositor_window_state::CompositorWindowState;
use sottomano_desktop_sources::{
    blocking_background_work::blocking,
    desktop_windows_apps::{firefox_favicon_lookup, firefox_favicon_lookup::RawFavicon},
};

use crate::icon_catalog::icon_handle::IconHandle;

#[derive(Default)]
pub struct Favicons {
    pub cached: HashMap<String, IconHandle>,
    titles: Vec<String>,
    lookup: Lookup,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProfileChanges {
    Unchanged,
    ChangedMeanwhile,
}

#[derive(Default)]
enum Lookup {
    #[default]
    Idle,
    Running {
        _aborts_on_drop: Handle,
        profile_changes: ProfileChanges,
    },
}

impl Favicons {
    pub fn merge(&mut self, raw: HashMap<String, RawFavicon>) -> Task<HashMap<String, RawFavicon>> {
        self.cached.extend(
            raw.into_iter()
                .filter(|(title, _)| self.titles.binary_search(title).is_ok())
                .map(|(title, raw)| (title, IconHandle::from(raw))),
        );
        let rerun = matches!(
            self.lookup,
            Lookup::Running {
                profile_changes: ProfileChanges::ChangedMeanwhile,
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
            profile_changes, ..
        } = &mut self.lookup
        {
            *profile_changes = ProfileChanges::ChangedMeanwhile;
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
            profile_changes: ProfileChanges::Unchanged,
        };
        task
    }
}
