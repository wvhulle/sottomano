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
    group_items::group_item::GroupItem,
    widget_groups::{
        group_summary::GroupSummary,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
    },
};

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
        .map(|reading| WallpaperMessage::Lookup(Box::new(reading)))
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

    pub fn group_content(
        &self,
        clock: Clock,
    ) -> WidgetGroupContent<'_, WidgetGroupExpansionMessage> {
        let lookup = self.lookup.get();
        let summarised_row = GroupItem::reading(self.lookup.main_row(
            Phosphor::Image(Weight::Regular).into(),
            |snapshot| snapshot.reading_row(),
            clock,
        ));
        let group_summary =
            GroupSummary::showing(Phosphor::Image(Weight::Regular).into(), &summarised_row);
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Wallpaper,
            group_summary,
            &self.expansion,
            identity,
            [
                Some(summarised_row),
                lookup
                    .and_then(|lookup| lookup.place.as_ref())
                    .map(|place| {
                        GroupItem::reading(place.reading_row()).with_hover_card(Box::new(
                            move |_| self.lookup.hover_card_with(place.hover_card()),
                        ))
                    }),
                lookup
                    .and_then(|lookup| lookup.article.as_ref())
                    .map(|article| {
                        GroupItem::reading(article.reading_row()).with_hover_card(Box::new(
                            move |_| self.lookup.hover_card_with(article.hover_card()),
                        ))
                    }),
                lookup.map(|lookup| {
                    GroupItem::reading(lookup.photo.reading_row()).with_hover_card(Box::new(
                        move |_| self.lookup.hover_card_with(lookup.photo.hover_card()),
                    ))
                }),
            ]
            .into_iter()
            .flatten(),
        )
    }
}
