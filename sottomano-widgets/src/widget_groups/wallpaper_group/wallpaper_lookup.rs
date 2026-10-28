use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    path::PathBuf,
};

use iced::futures::{Stream, StreamExt, stream};
use sottomano_sources::{
    SnapshotDelivery, SnapshotSource,
    stream_operators::StreamOperators,
    system_protocol_clients::shell_command_output,
    wallpaper_place_article::{self, DisplayedWallpaper, Wallpaper},
};

use super::{article_label::Article, photo_credit::Photo, place_label::Place};
use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody},
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct Lookup {
    pub(super) place: Option<Place>,
    pub(super) article: Option<Article>,
    pub(super) photo: Photo,
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

impl SnapshotSource for Lookup {
    type Error = Failure;
    type StreamInputs = PathBuf;

    const NAME: &'static str = "Wallpaper lookup";

    fn delivery_stream(unsplash_key: PathBuf) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(wallpaper_place_article::daemon_installed()).while_held(
            move || {
                let unsplash_key = unsplash_key.clone();
                wallpaper_place_article::displayed()
                    .distinct_until_changed()
                    .flat_map(move |displayed| {
                        let unsplash_key = unsplash_key.clone();
                        let path = match displayed {
                            Err(failure) => {
                                return stream::iter([SnapshotDelivery::live_push(Err(
                                    Failure::Query(failure),
                                ))])
                                .left_stream();
                            }
                            Ok(DisplayedWallpaper::NoImageFile) => {
                                return stream::iter([SnapshotDelivery::SourceUnavailable])
                                    .left_stream();
                            }
                            Ok(DisplayedWallpaper::ImageFile(path)) => path,
                        };
                        let Some(id) = wallpaper_place_article::photo_id(&path) else {
                            return stream::iter([SnapshotDelivery::SourceUnavailable])
                                .left_stream();
                        };
                        let unresolved =
                            Lookup::of(&wallpaper_place_article::unresolved(&path, &id));
                        let resolved = async move {
                            SnapshotDelivery::live_push(
                                wallpaper_place_article::resolve(path, id, &unsplash_key)
                                    .await
                                    .map(|wallpaper| Lookup::of(&wallpaper))
                                    .map_err(Failure::Resolve),
                            )
                        };
                        stream::iter([SnapshotDelivery::live_push(Ok(unresolved))])
                            .chain(stream::once(resolved))
                            .right_stream()
                    })
            },
            SnapshotDelivery::SourceUnavailable,
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
        TooltipBody::dictionary([LabeledValue::new("File", self.photo.file_name.clone())])
    }
}
