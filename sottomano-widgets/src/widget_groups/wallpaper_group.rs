mod article_label;
mod photo_credit;
mod place_label;
pub mod wallpaper_lookup;

use std::path::PathBuf;

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
};
use sottomano_toml_config::SecretPaths;

use self::wallpaper_lookup::Lookup;
use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_indicators::indicator_state::IndicatorState,
    widget_groups::{
        group_summary::GroupSummary,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
        widget_group_interaction::WidgetGroupInteraction,
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
    },
};

const WALLPAPER_GLYPH: Phosphor = Phosphor::Image(Weight::Regular);

pub struct WallpaperGroup {
    lookup: SubscribedSnapshot<Lookup>,
    unsplash_key: Option<PathBuf>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum WallpaperMessage {
    Lookup(Box<SnapshotDelivery<Lookup>>),
}

impl WallpaperGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(secrets: &SecretPaths) -> Self {
        Self {
            lookup: SubscribedSnapshot::default(),
            unsplash_key: secrets.unsplash_access_key.clone(),
            expansion: WidgetGroupExpansion::default(),
        }
    }

    pub fn subscription(&self, buses: Option<&DesktopBuses>) -> Subscription<WallpaperMessage> {
        let Some(unsplash_key) = &self.unsplash_key else {
            return Subscription::none();
        };
        let Some(buses) = buses else {
            return Subscription::none();
        };
        snapshot_subscription::<Lookup>(OnBus {
            buses: buses.clone(),
            inputs: unsplash_key.clone(),
        })
        .map(|sample| WallpaperMessage::Lookup(Box::new(sample)))
    }

    pub fn update(&mut self, message: WallpaperMessage, clock: Clock) -> Task<WallpaperMessage> {
        let WallpaperMessage::Lookup(delivery) = message;
        self.lookup.receive(*delivery, clock);
        Task::none()
    }

    pub fn interact(
        &mut self,
        expansion_message: WidgetGroupExpansionMessage,
    ) -> Task<WallpaperMessage> {
        self.expansion.update(expansion_message);
        Task::none()
    }

    pub fn group_content(&self, clock: Clock) -> WidgetGroupContent<'_> {
        let group_summary = match &self.lookup {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => GroupSummary::summarising(
                Phosphor::Mountains(Weight::Regular).into(),
                held_snapshot.indicator(),
            ),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                GroupSummary::summarising(
                    Phosphor::Mountains(Weight::Regular).into(),
                    troubled_snapshot
                        .shown_over(troubled_snapshot.held_snapshot.indicator(), clock),
                )
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => GroupSummary::summarising(
                Phosphor::Mountains(Weight::Regular).into(),
                placeholder.indicator(WALLPAPER_GLYPH.into()),
            ),
            SubscribedSnapshot::AwaitingFirstSnapshot => GroupSummary::glyph(
                Phosphor::Mountains(Weight::Regular).into(),
                IndicatorState::Pending,
            ),
            SubscribedSnapshot::SourceUnavailable => GroupSummary::glyph(
                Phosphor::Mountains(Weight::Regular).into(),
                IndicatorState::Off,
            ),
        };
        let mut rows = Vec::new();
        if let Some(lookup) = self.lookup.get() {
            if let Some(place) = &lookup.place {
                rows.push(place.indicator().with_hover_card(Box::new(move |_| {
                    self.lookup.hover_card_with(place.hover_card())
                })));
            }
            if let Some(article) = &lookup.article {
                rows.push(article.indicator().with_hover_card(Box::new(move |_| {
                    self.lookup.hover_card_with(article.hover_card())
                })));
            }
            rows.push(lookup.photo.indicator().with_hover_card(Box::new(move |_| {
                self.lookup.hover_card_with(lookup.photo.hover_card())
            })));
        }
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Wallpaper,
            group_summary,
            &self.expansion,
            WidgetGroupInteraction::Wallpaper,
            rows,
        )
    }
}
