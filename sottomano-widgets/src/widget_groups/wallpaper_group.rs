mod article_label;
mod photo_credit;
mod place_label;
pub mod wallpaper_lookup;

use std::path::PathBuf;

use iced::{Subscription, Task};
use sottomano_config::SecretPaths;
use sottomano_sources::{
    SnapshotDelivery, network_connectivity::internet_connectivity_gate::OnlineSourceInputs,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use self::wallpaper_lookup::Lookup;
use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
    hover_cards::{HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard},
    widget_groups::widget_group_content::{
        WidgetGroup, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
    },
    widgets::latest_snapshot::LatestSnapshot,
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
    lookup: LatestSnapshot<Lookup>,
    unsplash_key: Option<PathBuf>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum WallpaperMessage {
    Lookup(Box<SnapshotDelivery<Lookup>>),
    Expansion(WidgetGroupExpansionMessage),
}

impl WidgetGroup for WallpaperGroup {
    const TITLE: &'static str = "Wallpaper";
}

impl WallpaperGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(secrets: &SecretPaths) -> Self {
        Self {
            lookup: LatestSnapshot::default(),
            unsplash_key: secrets.unsplash_access_key.clone(),
            expansion: WidgetGroupExpansion::default(),
        }
    }

    pub fn subscription(&self, buses: Option<&DesktopBuses>) -> Subscription<WallpaperMessage> {
        match (&self.unsplash_key, buses) {
            (Some(unsplash_key), Some(buses)) => {
                snapshot_subscription::<Lookup>(OnlineSourceInputs {
                    buses: buses.clone(),
                    source_inputs: unsplash_key.clone(),
                })
                .map(|reading| WallpaperMessage::Lookup(Box::new(reading)))
            }
            _ => Subscription::none(),
        }
    }

    pub fn update(&mut self, message: WallpaperMessage, clock: Clock) -> Task<WallpaperMessage> {
        match message {
            WallpaperMessage::Lookup(delivery) => self.lookup.receive(*delivery, clock),
            WallpaperMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
        }
        Task::none()
    }

    #[must_use]
    pub fn hover_card_for(&self, target: WallpaperTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            WallpaperTarget::Lookup => self.lookup.hover_card(clock),
            WallpaperTarget::Place => self
                .lookup
                .hover_card_with(self.lookup.get()?.place.as_ref()?.hover_card(clock)),
            WallpaperTarget::Article => self
                .lookup
                .hover_card_with(self.lookup.get()?.article.as_ref()?.hover_card(clock)),
            WallpaperTarget::Photo => self
                .lookup
                .hover_card_with(self.lookup.get()?.photo.hover_card(clock)),
        }
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, WallpaperMessage> {
        let lookup = self.lookup.get();
        Self::expandable_group(
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
