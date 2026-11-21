use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    path::PathBuf,
};

use futures_rx::RxExt;
use iced::futures::{Stream, StreamExt, stream};
use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::AvailabilityStreams,
    network_connectivity::internet_connectivity_gate,
    secret_file_reads::AuthenticatedFetchError,
    system_protocol_clients::{dbus_desktop_buses::OnBus, shell_command_output},
    wallpaper_place_article::{
        self, DisplayedWallpaper,
        unsplash_photo_article::{self, PlaceArticle, Wallpaper},
    },
};

use super::{article_label::Article, photo_credit::Photo, place_label::Place};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Lookup {
    pub(super) place: Option<Place>,
    pub(super) article: Option<Article>,
    pub(super) photo: Photo,
}

impl From<&Wallpaper> for Lookup {
    fn from(wallpaper: &Wallpaper) -> Self {
        Self {
            place: Place::wallpaper_place(wallpaper),
            article: match &wallpaper.article {
                PlaceArticle::Found(article) => Some(Article::from(article)),
                PlaceArticle::Unmatched => None,
            },
            photo: Photo::from(wallpaper),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Failure {
    Query(shell_command_output::Error),
    Resolve(AuthenticatedFetchError),
}

impl Display for Failure {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Failure::Query(error) => write!(f, "awww: {error}"),
            Failure::Resolve(error) => write!(f, "{error}"),
        }
    }
}

impl SnapshotSource for Lookup {
    type Error = Failure;
    type StreamInputs = OnBus<PathBuf>;

    const NAME: &'static str = "Wallpaper lookup";

    fn delivery_stream(
        OnBus {
            buses,
            inputs: unsplash_key,
        }: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(wallpaper_place_article::daemon_availability()).while_available(
            move || {
                let unsplash_key = unsplash_key.clone();
                internet_connectivity_gate::while_online(buses.clone(), move || {
                    let unsplash_key = unsplash_key.clone();
                    wallpaper_place_article::displayed()
                        .distinct_until_changed()
                        .flat_map(move |displayed| {
                            displayed_lookups(displayed, unsplash_key.clone())
                        })
                })
            },
            || SnapshotDelivery::SourceUnavailable,
        )
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
            let unresolved = Lookup::from(&unsplash_photo_article::unresolved(&path, &id));
            let resolved = async move {
                SnapshotDelivery::live_push(
                    unsplash_photo_article::resolve(path, id, &unsplash_key)
                        .await
                        .map(|wallpaper| Lookup::from(&wallpaper))
                        .map_err(Failure::Resolve),
                )
            };
            stream::iter([SnapshotDelivery::live_push(Ok(unresolved))])
                .chain(stream::once(resolved))
                .right_stream()
        }
    }
}

impl Lookup {
    pub(super) fn indicator(&self) -> GroupIndicator<'_> {
        GroupIndicator::glyph_only(
            "Wallpaper lookup",
            Phosphor::Image(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Photo",
            dictionary_table([LabeledValue::new("File", self.photo.file_name.clone())]),
        )]
    }
}
