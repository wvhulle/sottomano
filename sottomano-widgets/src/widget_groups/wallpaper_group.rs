use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    path::PathBuf,
};

use chrono::Locale;
use iced::{
    Subscription, Task,
    futures::{Stream, StreamExt, stream},
};
use sottomano_config::SecretPaths;
use sottomano_sources::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    stream_operators::StreamOperators,
    system_protocol_clients::shell_command_output,
    wallpaper_place_article::{self, DisplayedWallpaper, Position, Wallpaper},
};
use uom::{fmt::DisplayStyle::Abbreviation, si::angle::degree};

use crate::{
    background_work::source_subscription,
    clock::Clock,
    hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
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
    #[must_use]
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
    Query(shell_command_output::Error),
    Resolve(wallpaper_place_article::Error),
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
            Failure::Resolve(wallpaper_place_article::Error::Fetch(fetch)) => fetch.into(),
            other => Section::error(other),
        }
    }
}

impl ReadingSource for Lookup {
    type Error = Failure;
    type StreamInputs = PathBuf;

    const NAME: &'static str = "wallpaper lookup";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(unsplash_key: PathBuf) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        stream::once(wallpaper_place_article::daemon_installed()).while_held(
            move || {
                let unsplash_key = unsplash_key.clone();
                wallpaper_place_article::displayed()
                    .distinct_until_changed()
                    .flat_map(move |displayed| {
                        let unsplash_key = unsplash_key.clone();
                        let path = match displayed {
                            Err(failure) => {
                                return stream::iter([SourceDelivery::Reading(Err(
                                    Failure::Query(failure),
                                ))])
                                .left_stream();
                            }
                            Ok(DisplayedWallpaper::NoImageFile) => {
                                return stream::iter([SourceDelivery::Absent]).left_stream();
                            }
                            Ok(DisplayedWallpaper::ImageFile(path)) => path,
                        };
                        let Some(id) = wallpaper_place_article::photo_id(&path) else {
                            return stream::iter([SourceDelivery::Absent]).left_stream();
                        };
                        let unresolved =
                            Lookup::of(&wallpaper_place_article::unresolved(&path, &id));
                        let resolved = async move {
                            SourceDelivery::Reading(
                                wallpaper_place_article::resolve(path, id, &unsplash_key)
                                    .await
                                    .map(|wallpaper| Lookup::of(&wallpaper))
                                    .map_err(Failure::Resolve),
                            )
                        };
                        stream::iter([SourceDelivery::Reading(Ok(unresolved))])
                            .chain(stream::once(resolved))
                            .right_stream()
                    })
            },
            SourceDelivery::Absent,
        )
    }
}

impl BarWidget for Lookup {
    fn name(&self) -> &'static str {
        "Wallpaper lookup"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Image(Weight::Regular).into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
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
        Phosphor::MapPin(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.name.clone())
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
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

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.title.clone())
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
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

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.photographer.clone()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
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
    Lookup(Box<SourceDelivery<Lookup>>),
    Expansion(ExpansionMessage),
}

impl WallpaperGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(secrets: &SecretPaths) -> Self {
        Self {
            lookup: ContinuousReading::default(),
            unsplash_key: secrets.unsplash_access_key.clone(),
            expansion: GroupExpansion::default(),
        }
    }

    pub fn subscription(&self) -> Subscription<WallpaperMessage> {
        match &self.unsplash_key {
            Some(unsplash_key) => source_subscription::<Lookup>(unsplash_key.clone())
                .map(|reading| WallpaperMessage::Lookup(Box::new(reading))),
            None => Subscription::none(),
        }
    }

    pub fn update(&mut self, message: WallpaperMessage, clock: Clock) -> Task<WallpaperMessage> {
        match message {
            WallpaperMessage::Lookup(delivery) => self.lookup.receive_delivery(*delivery, clock),
            WallpaperMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
        }
        Task::none()
    }

    #[must_use]
    pub fn tooltip_for(&self, target: WallpaperTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            WallpaperTarget::Lookup => self.lookup.tooltip(clock),
            WallpaperTarget::Place => self
                .lookup
                .tooltip_with(self.lookup.get()?.place.as_ref()?.tooltip(clock)),
            WallpaperTarget::Article => self
                .lookup
                .tooltip_with(self.lookup.get()?.article.as_ref()?.tooltip(clock)),
            WallpaperTarget::Photo => self
                .lookup
                .tooltip_with(self.lookup.get()?.photo.tooltip(clock)),
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
