mod article_label;
mod photo_credit;
mod place_label;
pub mod wallpaper_lookup;

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
};
use sottomano_toml_config::{SecretPaths, secrets::StoredSecret};

use self::{
    article_label::WallpaperArticle, place_label::WallpaperPlace, wallpaper_lookup::Lookup,
};
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

const WALLPAPER_GLYPH: Phosphor = Phosphor::Image(Weight::Regular);

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
        let mut lead_indicators = Vec::new();
        if !self.lookup.awaits_first_delivery() {
            lead_indicators.push(self.lookup.indicator(
                |held_snapshot| held_snapshot.indicator(),
                WALLPAPER_GLYPH.into(),
            ));
        }
        let mut rows = Vec::new();
        if let Some(lookup) = self.lookup.get() {
            if let WallpaperPlace::Named(place) = &lookup.place {
                rows.push(place.indicator().with_hover_card(Box::new(move |_| {
                    self.lookup.hover_card_with(place.hover_card())
                })));
            }
            if let WallpaperArticle::Found(article) = &lookup.article {
                rows.push(article.indicator().with_hover_card(Box::new(move |_| {
                    self.lookup.hover_card_with(article.hover_card())
                })));
            }
            rows.push(lookup.photo.indicator().with_hover_card(Box::new(move |_| {
                self.lookup.hover_card_with(lookup.photo.hover_card())
            })));
        }
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Wallpaper,
            headline_indicator,
            &self.expansion,
            IndicatorGroupInteraction::Wallpaper,
            IndicatorListing::Listed(lead_indicators.into_iter().chain(rows).collect()),
        )
    }
}
