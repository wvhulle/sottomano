use std::{
    collections::{HashMap, hash_map::Entry},
    sync::Arc,
};

use iced::{Color, Element, Task, Theme};
use sottomano_desktop_sources::{
    desktop_windows_apps::desktop_entries::DesktopEntries,
    theme_icons::{
        icon_requests::{IconRequest, RequestedIcon, ResolvedIcon, resolved_icons},
        theme_directory_index::{
            ThemeDirectoryIndex, ThemeIndexError, indexed_icon_theme, xdg_icon_search_roots,
        },
    },
};

use crate::icon_catalog::{icon_handle::IconHandle, pixmap_handles::PixmapHandles};

const STALE_ICON_OPACITY: f32 = 0.4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconFreshness {
    Current,
    Stale,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawableIcon {
    pub handle: IconHandle,
    pub design_side_px: f32,
    pub freshness: IconFreshness,
}

impl DrawableIcon {
    fn shown_opacity(&self, opacity: f32) -> f32 {
        match self.freshness {
            IconFreshness::Current => opacity,
            IconFreshness::Stale => opacity * STALE_ICON_OPACITY,
        }
    }

    pub fn view<'a, Message: 'a>(&self, side_px: f32, opacity: f32) -> Element<'a, Message> {
        self.handle.view(side_px, self.shown_opacity(opacity))
    }

    pub fn view_with_symbolic_tint<'a, Message: 'a>(
        &self,
        side_px: f32,
        opacity: f32,
        symbolic_tint: impl Fn(&Theme) -> Color + 'a,
    ) -> Element<'a, Message> {
        self.handle
            .view_with_symbolic_tint(side_px, self.shown_opacity(opacity), symbolic_tint)
    }

    pub fn scaled_down<'a, Message: 'a>(&self, opacity: f32) -> Element<'a, Message> {
        self.handle.scaled_down(self.shown_opacity(opacity))
    }
}

#[derive(Clone, Copy)]
pub struct DrawableIconSources<'sources> {
    pub icon_catalog: &'sources IconCatalog,
    pub pixmap_handles: &'sources PixmapHandles,
}

#[derive(Debug, Clone)]
pub enum CatalogIcon {
    Pending,
    Drawable(DrawableIcon),
    Missing,
    Unloadable,
}

#[derive(Debug, Clone)]
enum ThemeIndexing {
    Unnamed,
    Indexing {
        theme_name: String,
    },
    Indexed {
        theme_name: String,
        theme_index: Arc<ThemeDirectoryIndex>,
    },
}

#[derive(Debug, Clone)]
pub enum CatalogMessage {
    ThemeIndexed {
        generation: u64,
        theme_index: Arc<ThemeDirectoryIndex>,
    },
    IconsResolved {
        generation: u64,
        requested_icons: Vec<RequestedIcon>,
    },
    ThemeIndexFailed {
        index_failure: Arc<ThemeIndexError>,
    },
}

#[derive(Debug, Clone)]
pub struct IconCatalog {
    theme_index: ThemeIndexing,
    catalog_icons: HashMap<IconRequest, CatalogIcon>,
    generation: u64,
}

impl Default for IconCatalog {
    fn default() -> Self {
        Self {
            theme_index: ThemeIndexing::Unnamed,
            catalog_icons: HashMap::new(),
            generation: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SeededIcon {
    pub request: IconRequest,
    pub drawable_icon: DrawableIcon,
}

impl FromIterator<SeededIcon> for IconCatalog {
    fn from_iter<SeededIcons: IntoIterator<Item = SeededIcon>>(seeded_icons: SeededIcons) -> Self {
        Self {
            catalog_icons: seeded_icons
                .into_iter()
                .map(|seeded_icon| {
                    (
                        seeded_icon.request,
                        CatalogIcon::Drawable(seeded_icon.drawable_icon),
                    )
                })
                .collect(),
            ..Self::default()
        }
    }
}

impl CatalogIcon {
    fn outdated(self) -> Self {
        match self {
            CatalogIcon::Drawable(drawable_icon) => CatalogIcon::Drawable(DrawableIcon {
                freshness: IconFreshness::Stale,
                ..drawable_icon
            }),
            CatalogIcon::Pending | CatalogIcon::Missing | CatalogIcon::Unloadable => {
                CatalogIcon::Pending
            }
        }
    }
}

impl From<ResolvedIcon> for CatalogIcon {
    fn from(resolved: ResolvedIcon) -> Self {
        match resolved {
            ResolvedIcon::Loaded(loaded_icon) => CatalogIcon::Drawable(DrawableIcon {
                handle: IconHandle::from(&loaded_icon.drawing),
                design_side_px: loaded_icon.design_side_px,
                freshness: IconFreshness::Current,
            }),
            ResolvedIcon::Missing => CatalogIcon::Missing,
            ResolvedIcon::Unloadable(_) => CatalogIcon::Unloadable,
        }
    }
}

impl IconCatalog {
    #[must_use]
    pub fn drawable_icon(&self, request: &IconRequest) -> Option<&DrawableIcon> {
        match self.catalog_icons.get(request)? {
            CatalogIcon::Drawable(drawable_icon) => Some(drawable_icon),
            CatalogIcon::Pending | CatalogIcon::Missing | CatalogIcon::Unloadable => None,
        }
    }

    pub fn refresh(&mut self) -> Task<CatalogMessage> {
        match &self.theme_index {
            ThemeIndexing::Unnamed => Task::none(),
            ThemeIndexing::Indexing { theme_name } | ThemeIndexing::Indexed { theme_name, .. } => {
                self.reindex(theme_name.clone())
            }
        }
    }

    pub fn reindex(&mut self, theme_name: String) -> Task<CatalogMessage> {
        self.generation += 1;
        self.theme_index = ThemeIndexing::Indexing {
            theme_name: theme_name.clone(),
        };
        self.catalog_icons = self
            .catalog_icons
            .drain()
            .map(|(request, catalog_icon)| (request, catalog_icon.outdated()))
            .collect();
        let generation = self.generation;
        Task::perform(
            indexed_icon_theme(theme_name, xdg_icon_search_roots()),
            move |indexing| match indexing {
                Ok(theme_index) => CatalogMessage::ThemeIndexed {
                    generation,
                    theme_index: Arc::new(theme_index),
                },
                Err(index_failure) => CatalogMessage::ThemeIndexFailed {
                    index_failure: Arc::new(index_failure),
                },
            },
        )
    }

    pub fn request(
        &mut self,
        requests: impl IntoIterator<Item = IconRequest>,
        desktop_entries: &Arc<DesktopEntries>,
    ) -> Task<CatalogMessage> {
        let unseen_requests: Vec<IconRequest> = requests
            .into_iter()
            .filter(|request| match self.catalog_icons.entry(request.clone()) {
                Entry::Vacant(vacant_entry) => {
                    vacant_entry.insert(CatalogIcon::Pending);
                    true
                }
                Entry::Occupied(_) => false,
            })
            .collect();
        self.resolve(unseen_requests, desktop_entries)
    }

    pub fn update(
        &mut self,
        message: CatalogMessage,
        desktop_entries: &Arc<DesktopEntries>,
    ) -> Task<CatalogMessage> {
        match message {
            CatalogMessage::ThemeIndexed {
                generation,
                theme_index,
            } if generation == self.generation => {
                let ThemeIndexing::Indexing { theme_name } = &self.theme_index else {
                    return Task::none();
                };
                self.theme_index = ThemeIndexing::Indexed {
                    theme_name: theme_name.clone(),
                    theme_index,
                };
                let known_requests = self.catalog_icons.keys().cloned().collect();
                self.resolve(known_requests, desktop_entries)
            }
            CatalogMessage::IconsResolved {
                generation,
                requested_icons,
            } if generation == self.generation => {
                requested_icons.into_iter().for_each(|requested_icon| {
                    self.catalog_icons.insert(
                        requested_icon.request,
                        CatalogIcon::from(requested_icon.resolved),
                    );
                });
                Task::none()
            }
            CatalogMessage::ThemeIndexFailed { index_failure } => {
                tracing::error!(%index_failure, "icon catalog: cannot index the icon theme");
                Task::none()
            }
            CatalogMessage::ThemeIndexed { .. } | CatalogMessage::IconsResolved { .. } => {
                Task::none()
            }
        }
    }

    fn resolve(
        &self,
        requests: Vec<IconRequest>,
        desktop_entries: &Arc<DesktopEntries>,
    ) -> Task<CatalogMessage> {
        let ThemeIndexing::Indexed { theme_index, .. } = &self.theme_index else {
            return Task::none();
        };
        if requests.is_empty() {
            return Task::none();
        }
        let generation = self.generation;
        Task::perform(
            resolved_icons(
                Arc::clone(theme_index),
                Arc::clone(desktop_entries),
                requests,
            ),
            move |requested_icons| CatalogMessage::IconsResolved {
                generation,
                requested_icons,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use iced::widget::svg;
    use sottomano_desktop_sources::theme_icons::icon_requests::{
        app_icon_request, window_theme_icon_request,
    };

    use super::*;

    fn indexed_catalog() -> IconCatalog {
        IconCatalog {
            theme_index: ThemeIndexing::Indexed {
                theme_name: "Example-Dark".to_owned(),
                theme_index: Arc::new(ThemeDirectoryIndex::default()),
            },
            catalog_icons: HashMap::new(),
            generation: 3,
        }
    }

    #[test]
    fn a_known_or_pending_request_is_not_resolved_twice() {
        let mut catalog = indexed_catalog();
        let entries = Arc::new(DesktopEntries::default());
        let _ = catalog.request([app_icon_request("firefox")], &entries);
        catalog
            .catalog_icons
            .insert(window_theme_icon_request("folder"), CatalogIcon::Missing);
        let unseen_before = catalog.catalog_icons.len();
        let _ = catalog.request(
            [
                app_icon_request("firefox"),
                window_theme_icon_request("folder"),
            ],
            &entries,
        );
        assert_eq!(catalog.catalog_icons.len(), unseen_before);
        assert!(matches!(
            catalog.catalog_icons.get(&app_icon_request("firefox")),
            Some(CatalogIcon::Pending)
        ));
    }

    #[test]
    fn results_from_an_older_generation_are_dropped() {
        let mut catalog = indexed_catalog();
        let entries = Arc::new(DesktopEntries::default());
        let _ = catalog.update(
            CatalogMessage::IconsResolved {
                generation: 2,
                requested_icons: vec![RequestedIcon {
                    request: app_icon_request("firefox"),
                    resolved: ResolvedIcon::Missing,
                }],
            },
            &entries,
        );
        assert!(catalog.catalog_icons.is_empty());
    }

    #[test]
    fn reindexing_clears_unresolved_icons_and_dims_drawn_ones() {
        let mut catalog = indexed_catalog();
        catalog
            .catalog_icons
            .insert(window_theme_icon_request("folder"), CatalogIcon::Missing);
        catalog.catalog_icons.insert(
            app_icon_request("firefox"),
            CatalogIcon::Drawable(DrawableIcon {
                handle: IconHandle::Svg(svg::Handle::from_memory(b"<svg/>".as_slice())),
                design_side_px: 48.0,
                freshness: IconFreshness::Current,
            }),
        );
        let _ = catalog.reindex("Example-Light".to_owned());
        assert_eq!(catalog.generation, 4);
        assert!(matches!(
            catalog.theme_index,
            ThemeIndexing::Indexing { ref theme_name } if theme_name == "Example-Light"
        ));
        assert!(matches!(
            catalog
                .catalog_icons
                .get(&window_theme_icon_request("folder")),
            Some(CatalogIcon::Pending)
        ));
        assert!(matches!(
            catalog.drawable_icon(&app_icon_request("firefox")),
            Some(DrawableIcon {
                freshness: IconFreshness::Stale,
                ..
            })
        ));
    }
}
