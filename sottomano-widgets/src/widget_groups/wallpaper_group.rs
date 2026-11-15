mod article_label;
mod photo_credit;
mod place_label;
pub mod wallpaper_lookup;

use std::{convert::identity, path::PathBuf};

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
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
    hover_cards::{HoverTarget, hover_card_body::Section},
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

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
    lookup: SubscribedSnapshot<Lookup>,
    unsplash_key: Option<PathBuf>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum WallpaperMessage {
    Lookup(Box<SnapshotDelivery<Lookup>>),
    Expansion(WidgetGroupExpansionMessage),
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
        .map(|reading| WallpaperMessage::Lookup(Box::new(reading)))
    }

    pub fn update(&mut self, message: WallpaperMessage, clock: Clock) -> Task<WallpaperMessage> {
        match message {
            WallpaperMessage::Lookup(delivery) => self.lookup.receive(*delivery, clock),
            WallpaperMessage::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
            }
        }
        Task::none()
    }

    #[must_use]
    pub fn hover_card_for(&self, target: WallpaperTarget, clock: Clock) -> Vec<Section> {
        match target {
            WallpaperTarget::Lookup => self.lookup.hover_card(clock),
            WallpaperTarget::Place => self
                .lookup
                .get()
                .and_then(|lookup| lookup.place.as_ref())
                .map(|place| self.lookup.hover_card_with(place.hover_card(clock)))
                .unwrap_or_default(),
            WallpaperTarget::Article => self
                .lookup
                .get()
                .and_then(|lookup| lookup.article.as_ref())
                .map(|article| self.lookup.hover_card_with(article.hover_card(clock)))
                .unwrap_or_default(),
            WallpaperTarget::Photo => self
                .lookup
                .get()
                .map(|lookup| self.lookup.hover_card_with(lookup.photo.hover_card(clock)))
                .unwrap_or_default(),
        }
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, WidgetGroupExpansionMessage> {
        let lookup = self.lookup.get();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Wallpaper,
            &self.expansion,
            identity,
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
