use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    path::PathBuf,
};

use iced::futures::{Stream, StreamExt, stream};
use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource,
    network_connectivity::internet_connectivity_gate::{self, OnlineSourceInputs},
    stream_operators::StreamOperators,
    system_protocol_clients::shell_command_output,
    wallpaper_place_article::{self, DisplayedWallpaper, Wallpaper},
};

use super::{article_label::Article, photo_credit::Photo, place_label::Place};
use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
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
    type StreamInputs = OnlineSourceInputs<PathBuf>;

    const NAME: &'static str = "Wallpaper lookup";

    fn delivery_stream(
        OnlineSourceInputs {
            buses,
            source_inputs: unsplash_key,
        }: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(wallpaper_place_article::daemon_installed()).available_while(move || {
            let unsplash_key = unsplash_key.clone();
            internet_connectivity_gate::while_online(buses.clone(), move || {
                let unsplash_key = unsplash_key.clone();
                wallpaper_place_article::displayed()
                    .distinct_until_changed()
                    .flat_map(move |displayed| displayed_lookups(displayed, unsplash_key.clone()))
            })
        })
    }
}

fn displayed_lookups(
    displayed: Result<DisplayedWallpaper, shell_command_output::Error>,
    unsplash_key: PathBuf,
) -> impl Stream<Item = SnapshotDelivery<Lookup>> + Send {
    let credited_photo = match displayed {
        Err(failure) => Err(SnapshotDelivery::live_push(Err(Failure::Query(failure)))),
        Ok(DisplayedWallpaper::NoImageFile) => Err(SnapshotDelivery::SourceUnavailable),
        Ok(DisplayedWallpaper::ImageFile(path)) => wallpaper_place_article::photo_id(&path)
            .map(|id| (path, id))
            .ok_or(SnapshotDelivery::SourceUnavailable),
    };
    match credited_photo {
        Err(settled) => stream::iter([settled]).left_stream(),
        Ok((path, id)) => {
            let unresolved = Lookup::of(&wallpaper_place_article::unresolved(&path, &id));
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
        }
    }
}

impl GroupItemReading for Lookup {
    fn name(&self) -> &'static str {
        "Wallpaper lookup"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Image(Weight::Regular).into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([LabeledValue::new("File", self.photo.file_name.clone())])
    }
}
