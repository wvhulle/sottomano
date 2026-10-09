use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    path::PathBuf,
};

use iced::{Subscription, Task};
use uom::{fmt::DisplayStyle::Abbreviation, si::angle::degree};

use crate::{
    config::SecretPaths,
    data::source::{
        command,
        wallpaper::{self, Position, Wallpaper},
    },
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        last_poll::LastPoll,
        phosphor::Phosphor,
        simple_icons::SimpleIcon,
        tooltip_body::{Cell, Section, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupExpansion, GroupHandle, HandleFace, WidgetGroup},
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct Lookup {
    place: Option<Place>,
    article: Option<Article>,
    photo: Photo,
}

impl Lookup {
    fn of(wallpaper: &Wallpaper) -> Self {
        Self {
            place: Place::of(wallpaper),
            article: Article::of(wallpaper),
            photo: Photo::of(wallpaper),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Failure {
    Query(command::Error),
    Resolve(wallpaper::Error),
}

impl Display for Failure {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Failure::Query(error) => write!(f, "awww: {error}"),
            Failure::Resolve(error) => write!(f, "{error}"),
        }
    }
}

impl BarWidget for Lookup {
    fn icon(&self) -> Glyph {
        Phosphor::Image(self.state().weight()).into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[Section::untitled(&[("File", self.photo.file_name.clone())])])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    name: String,
    position: Option<Position>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Article {
    title: String,
    url: String,
    summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Photo {
    file_name: String,
    photographer: Option<String>,
    caption: Option<String>,
    url: String,
}

impl BarWidget for Place {
    fn icon(&self) -> Glyph {
        Phosphor::MapPin(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        Some(self.name.clone())
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[Section::untitled(&[
            ("Where", Some(self.name.clone())),
            (
                "Latitude",
                self.position.map(|position| {
                    format!(
                        "{:.5}",
                        position.latitude.into_format_args(degree, Abbreviation)
                    )
                }),
            ),
            (
                "Longitude",
                self.position.map(|position| {
                    format!(
                        "{:.5}",
                        position.longitude.into_format_args(degree, Abbreviation)
                    )
                }),
            ),
        ])])
    }
}

impl BarWidget for Article {
    fn icon(&self) -> Glyph {
        SimpleIcon::Wikipedia.into()
    }

    fn label(&self) -> Option<String> {
        Some(self.title.clone())
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[
            Section::titled(
                "Wikipedia",
                &[
                    ("Article", Cell::from(&self.title)),
                    ("Link", Cell::Link(self.url.clone())),
                ],
            ),
            Section::titled("Summary", &[(" ", &self.summary)]),
        ])
    }
}

impl BarWidget for Photo {
    fn icon(&self) -> Glyph {
        SimpleIcon::Unsplash.into()
    }

    fn label(&self) -> Option<String> {
        self.photographer.clone()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[
            Section::titled(
                "Photo",
                &[
                    ("File", Cell::from(&self.file_name)),
                    ("Photographer", Cell::from(&self.photographer)),
                    ("Unsplash", Cell::Link(self.url.clone())),
                ],
            ),
            Section::titled("Caption", &[(" ", &self.caption)]),
        ])
    }
}

impl Place {
    fn of(wallpaper: &Wallpaper) -> Option<Self> {
        Some(Self {
            name: wallpaper.place.clone()?,
            position: wallpaper.position,
        })
    }
}

impl Article {
    fn of(wallpaper: &Wallpaper) -> Option<Self> {
        let article = wallpaper.article.as_ref()?;
        Some(Self {
            title: article.title.clone(),
            url: article.url.clone(),
            summary: article.summary.clone(),
        })
    }
}

impl Photo {
    fn of(wallpaper: &Wallpaper) -> Self {
        Self {
            file_name: wallpaper.file_name.clone(),
            photographer: wallpaper.photographer.clone(),
            caption: wallpaper.caption.clone(),
            url: wallpaper.photo_url.clone(),
        }
    }
}

pub struct WallpaperGroup {
    lookup: LastPoll<Lookup>,
    shown: Option<String>,
    unsplash_key: Option<PathBuf>,
    wallpaper_daemon: bool,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum WallpaperMessage {
    DaemonFound(bool),
    Displayed(Result<Option<String>, command::Error>),
    Resolved(Result<Wallpaper, wallpaper::Error>),
    Expansion(ExpansionMessage),
}

impl WallpaperGroup {
    pub fn new(secrets: &SecretPaths) -> Self {
        Self {
            lookup: LastPoll::default(),
            shown: None,
            unsplash_key: secrets.unsplash_access_key.clone(),
            wallpaper_daemon: false,
            expansion: GroupExpansion::default(),
        }
    }

    pub fn probe(&self) -> Task<WallpaperMessage> {
        if self.unsplash_key.is_none() {
            return Task::none();
        }
        Task::future(async { WallpaperMessage::DaemonFound(wallpaper::daemon_installed().await) })
    }

    pub fn subscription(&self) -> Subscription<WallpaperMessage> {
        if self.unsplash_key.is_none() || !self.wallpaper_daemon {
            return Subscription::none();
        }
        wallpaper::subscription().map(WallpaperMessage::Displayed)
    }

    pub fn update(&mut self, message: WallpaperMessage) -> Task<WallpaperMessage> {
        match message {
            WallpaperMessage::DaemonFound(found) => self.wallpaper_daemon = found,
            WallpaperMessage::Displayed(Ok(path)) => return self.follow(path),
            WallpaperMessage::Displayed(Err(failure)) => {
                self.lookup.receive(Err(Failure::Query(failure)));
            }
            WallpaperMessage::Resolved(resolved) => {
                self.lookup.receive(
                    resolved
                        .map(|wallpaper| Lookup::of(&wallpaper))
                        .map_err(Failure::Resolve),
                );
            }
            WallpaperMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
        }
        Task::none()
    }

    fn follow(&mut self, path: Option<String>) -> Task<WallpaperMessage> {
        if self.shown == path {
            return Task::none();
        }
        self.shown.clone_from(&path);
        let Some(path) = path else {
            self.lookup.load(None);
            return Task::none();
        };
        let (Some(id), Some(key)) = (wallpaper::photo_id(&path), self.unsplash_key.clone()) else {
            self.lookup.load(None);
            return Task::none();
        };
        self.lookup
            .load(Some(Lookup::of(&wallpaper::unresolved(&path, &id))));
        Task::future(async move { wallpaper::resolve(path, id, &key).await })
            .map(WallpaperMessage::Resolved)
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        let body = match target {
            HoverTarget::Wallpaper => self.lookup.display()?.tooltip(),
            HoverTarget::WallpaperPlace => self.lookup.get()?.place.as_ref()?.tooltip(),
            HoverTarget::WallpaperArticle => self.lookup.get()?.article.as_ref()?.tooltip(),
            HoverTarget::WallpaperPhoto => self.lookup.get()?.photo.tooltip(),
            _ => return None,
        };
        Some(Tooltip::of(body))
    }

    pub fn widget_group(&self) -> WidgetGroup<'_, WallpaperMessage> {
        let lookup = self.lookup.get();
        WidgetGroup::new(
            &self.expansion,
            WallpaperMessage::Expansion,
            GroupHandle {
                face: HandleFace::FirstItem,
                preview: None,
                forwards_scroll: false,
            },
            &[
                self.lookup.reading_item(HoverTarget::Wallpaper),
                lookup
                    .and_then(|lookup| lookup.place.as_ref())
                    .map(|place| GroupItem::reading(place, HoverTarget::WallpaperPlace)),
                lookup
                    .and_then(|lookup| lookup.article.as_ref())
                    .map(|article| GroupItem::reading(article, HoverTarget::WallpaperArticle)),
                lookup.map(|lookup| GroupItem::reading(&lookup.photo, HoverTarget::WallpaperPhoto)),
            ],
        )
    }
}
