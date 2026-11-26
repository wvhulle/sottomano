mod article_indicator;
mod photo_credit_indicator;
mod place_indicator;
pub mod wallpaper_lookup;

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
};
use sottomano_toml_config::{SecretPaths, secrets::StoredSecret};

use self::wallpaper_lookup::Lookup;
use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
            IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
        indicator_group_interaction::IndicatorGroupInteraction,
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

pub(super) const WALLPAPER_GLYPH: Phosphor = Phosphor::Image(Weight::Regular);

pub struct WallpaperGroup {
    lookup: SubscribedSnapshot<Lookup>,
    unsplash_key: StoredSecret,
    expansion: IndicatorGroupExpansion,
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
            expansion: IndicatorGroupExpansion::default(),
        }
    }

    pub fn subscription(&self, buses: Option<&DesktopBuses>) -> Subscription<WallpaperMessage> {
        let StoredSecret::Present(unsplash_key) = &self.unsplash_key else {
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
        expansion_message: IndicatorGroupExpansionMessage,
    ) -> Task<WallpaperMessage> {
        self.expansion.update(expansion_message);
        Task::none()
    }

    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        let headline_indicator = self
            .lookup
            .indicator(
                |held_snapshot| held_snapshot.indicator(),
                WALLPAPER_GLYPH.into(),
            )
            .with_headline_glyph(Phosphor::Mountains(Weight::Regular).into());
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Wallpaper,
            headline_indicator,
            &self.expansion,
            IndicatorGroupInteraction::Wallpaper,
            [
                self.lookup_listing(),
                IndicatorListing::Listed(
                    self.place_rows()
                        .into_iter()
                        .chain(self.article_rows())
                        .chain(self.photo_credit_rows())
                        .collect(),
                ),
            ]
            .into_iter()
            .collect(),
        )
    }
}
