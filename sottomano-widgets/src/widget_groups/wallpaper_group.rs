mod article_label;
mod photo_credit;
mod place_label;
pub mod wallpaper_lookup;

use std::path::PathBuf;

use iced::{Subscription, Task};
use sottomano_config::SecretPaths;
use sottomano_sources::SnapshotDelivery;

use self::wallpaper_lookup::Lookup;
use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        latest_snapshot::LatestSnapshot,
        tooltip_body::tooltip_source_delivery::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, WidgetGroup},
    },
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
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum WallpaperMessage {
    Lookup(Box<SnapshotDelivery<Lookup>>),
    Expansion(ExpansionMessage),
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
            expansion: GroupExpansion::default(),
        }
    }

    pub fn subscription(&self) -> Subscription<WallpaperMessage> {
        match &self.unsplash_key {
            Some(unsplash_key) => snapshot_subscription::<Lookup>(unsplash_key.clone())
                .map(|reading| WallpaperMessage::Lookup(Box::new(reading))),
            None => Subscription::none(),
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
