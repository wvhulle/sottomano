use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    path::PathBuf,
};

use iced::{
    Subscription, Task,
    futures::{Stream, StreamExt, stream},
};
use uom::{fmt::DisplayStyle::Abbreviation, si::angle::degree};

use crate::{
    config::SecretPaths,
    data::source::{
        Delivery, LiveSource, Source, command, emit_changes, present_or_absent,
        wallpaper::{self, DisplayedWallpaper, Position, Wallpaper},
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::listening,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        phosphor::Phosphor,
        simple_icons::SimpleIcon,
        tooltip_body::{Cell, KeyValue, Section, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct Lookup {
    place: Option<Place>,
    article: Option<Article>,
    photo: Photo,
}

impl Lookup {
    pub fn of(wallpaper: &Wallpaper) -> Self {
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

impl From<Failure> for Section {
    fn from(failure: Failure) -> Self {
        match failure {
            Failure::Resolve(wallpaper::Error::Fetch(fetch)) => fetch.into(),
            other => Section::error(other),
        }
    }
}

impl Source for Lookup {
    type Error = Failure;
    type Key = PathBuf;

    const NAME: &'static str = "wallpaper lookup";
}

impl LiveSource for Lookup {
    fn events(unsplash_key: PathBuf) -> impl Stream<Item = Delivery<Self>> + Send {
        present_or_absent(stream::once(wallpaper::daemon_installed()), move || {
            let unsplash_key = unsplash_key.clone();
            emit_changes(wallpaper::displayed())
                .flat_map(move |displayed| {
                    let unsplash_key = unsplash_key.clone();
                    match displayed {
                        Err(failure) => {
                            stream::iter([Delivery::Reading(Err(Failure::Query(failure)))]).boxed()
                        }
                        Ok(DisplayedWallpaper::NoImageFile) => {
                            stream::iter([Delivery::Absent]).boxed()
                        }
                        Ok(DisplayedWallpaper::ImageFile(path)) => {
                            match wallpaper::photo_id(&path) {
                                None => stream::iter([Delivery::Absent]).boxed(),
                                Some(id) => {
                                    let unresolved = Lookup::of(&wallpaper::unresolved(&path, &id));
                                    let resolved = async move {
                                        Delivery::Reading(
                                            wallpaper::resolve(path, id, &unsplash_key)
                                                .await
                                                .map(|wallpaper| Lookup::of(&wallpaper))
                                                .map_err(Failure::Resolve),
                                        )
                                    };
                                    stream::iter([Delivery::Reading(Ok(unresolved))])
                                        .chain(stream::once(resolved))
                                        .boxed()
                                }
                            }
                        }
                    }
                })
                .boxed()
        })
    }
}

impl BarWidget for Lookup {
    fn name(&self) -> &'static str {
        "Wallpaper lookup"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Image(self.state().weight()).into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::dictionary([KeyValue::new("File", self.photo.file_name.clone())])
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
    fn name(&self) -> &'static str {
        "Wallpaper place"
    }

    fn icon(&self) -> Glyph {
        Phosphor::MapPin(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        Some(self.name.clone())
    }

    fn tooltip(&self) -> TooltipBody {
        let coordinates = self.position.into_iter().flat_map(|position| {
            [
                KeyValue::new(
                    "Latitude",
                    format!(
                        "{:.5}",
                        position.latitude.into_format_args(degree, Abbreviation)
                    ),
                ),
                KeyValue::new(
                    "Longitude",
                    format!(
                        "{:.5}",
                        position.longitude.into_format_args(degree, Abbreviation)
                    ),
                ),
            ]
        });
        TooltipBody::dictionary(
            [KeyValue::new("Where", self.name.as_str())]
                .into_iter()
                .chain(coordinates),
        )
    }
}

impl BarWidget for Article {
    fn name(&self) -> &'static str {
        "Wallpaper article"
    }

    fn icon(&self) -> Glyph {
        SimpleIcon::Wikipedia.into()
    }

    fn label(&self) -> Option<String> {
        Some(self.title.clone())
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([
            Section::dictionary([KeyValue::new(
                "Article",
                Cell::Link {
                    label: self.title.clone(),
                    url: self.url.clone(),
                },
            )])
            .titled("Wikipedia"),
            Section::dictionary(
                self.summary
                    .as_deref()
                    .map(|summary| KeyValue::new(" ", summary)),
            )
            .titled("Summary"),
        ])
    }
}

impl BarWidget for Photo {
    fn name(&self) -> &'static str {
        "Wallpaper photo"
    }

    fn icon(&self) -> Glyph {
        SimpleIcon::Unsplash.into()
    }

    fn label(&self) -> Option<String> {
        self.photographer.clone()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([
            Section::dictionary(
                [
                    Some(KeyValue::new("File", self.file_name.as_str())),
                    self.photographer
                        .as_deref()
                        .map(|photographer| KeyValue::new("Photographer", photographer)),
                    Some(KeyValue::new(
                        "Unsplash",
                        Cell::Link {
                            label: "photo page".to_owned(),
                            url: self.url.clone(),
                        },
                    )),
                ]
                .into_iter()
                .flatten(),
            )
            .titled("Photo"),
            Section::dictionary(
                self.caption
                    .as_deref()
                    .map(|caption| KeyValue::new(" ", caption)),
            )
            .titled("Caption"),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallpaperTarget {
    Lookup,
    Place,
    Article,
    Photo,
}

impl From<WallpaperTarget> for HoverTarget {
    fn from(target: WallpaperTarget) -> Self {
        HoverTarget::Wallpaper(target)
    }
}

pub struct WallpaperGroup {
    lookup: ContinuousReading<Lookup>,
    unsplash_key: Option<PathBuf>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum WallpaperMessage {
    Lookup(Box<Delivery<Lookup>>),
    Expansion(ExpansionMessage),
}

impl WallpaperGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn new(secrets: &SecretPaths) -> Self {
        Self {
            lookup: ContinuousReading::default(),
            unsplash_key: secrets.unsplash_access_key.clone(),
            expansion: GroupExpansion::default(),
        }
    }

    pub fn subscription(&self) -> Subscription<WallpaperMessage> {
        match &self.unsplash_key {
            Some(unsplash_key) => listening::<Lookup>(unsplash_key.clone())
                .map(|reading| WallpaperMessage::Lookup(Box::new(reading))),
            None => Subscription::none(),
        }
    }

    pub fn update(&mut self, message: WallpaperMessage) -> Task<WallpaperMessage> {
        match message {
            WallpaperMessage::Lookup(delivery) => self.lookup.push(*delivery),
            WallpaperMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
        }
        Task::none()
    }

    pub fn tooltip_for(&self, target: WallpaperTarget) -> Option<Tooltip> {
        match target {
            WallpaperTarget::Lookup => self.lookup.tooltip(),
            WallpaperTarget::Place => self
                .lookup
                .tooltip_with(self.lookup.get()?.place.as_ref()?.tooltip()),
            WallpaperTarget::Article => self
                .lookup
                .tooltip_with(self.lookup.get()?.article.as_ref()?.tooltip()),
            WallpaperTarget::Photo => self.lookup.tooltip_with(self.lookup.get()?.photo.tooltip()),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, WallpaperMessage> {
        let lookup = self.lookup.get();
        GroupContent::new(
            &self.expansion,
            WallpaperMessage::Expansion,
            [
                self.lookup.group_item(WallpaperTarget::Lookup),
                lookup
                    .and_then(|lookup| lookup.place.as_ref())
                    .map(|place| GroupItem::reading(place, WallpaperTarget::Place)),
                lookup
                    .and_then(|lookup| lookup.article.as_ref())
                    .map(|article| GroupItem::reading(article, WallpaperTarget::Article)),
                lookup.map(|lookup| GroupItem::reading(&lookup.photo, WallpaperTarget::Photo)),
            ]
            .into_iter()
            .flatten(),
        )
    }
}
