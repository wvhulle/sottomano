use std::{
    collections::BTreeSet,
    sync::Arc,
    time::{Duration, Instant},
};

use corredo::popovers::{NestedMenuAction, NestedMenuPresentation};
use iced::Task;
use sottomano_desktop_sources::sqlite_state_store::StoreError;
use sottomano_toml_config::severity::Severity;

use crate::{
    background_work::logged_failure_task,
    group_indicators::group_indicator::GroupIndicator,
    group_menus::{
        group_menu_inspection::GroupMenuInspection,
        group_source_demand::{GroupSourceDemand, GroupSurface},
        sqlite_group_menu_pins,
        sqlite_group_menu_pins::{PinChange, StoredGroupMenuPin},
        sqlite_group_menu_presentation,
    },
    hover_cards::{IndicatorKey, PanelPinOrigin},
    indicator_groups::{
        indicator_group_content::{
            ExpansionChange, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
    },
    widgets::{
        layer_appearance_controls::{AppearanceMessage, LayerAppearanceControls},
        shell_layer_appearance::ShellLayer,
    },
};

#[cfg(test)]
mod group_menu_state_tests;
mod group_menu_topic;

pub use group_menu_topic::GroupMenuTopic;

const MENU_SOURCE_LINGER: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupMenuPin {
    Group(IndicatorGroupIdentity),
    Item(IndicatorKey),
}

impl GroupMenuPin {
    fn stored_in(self, menu: GroupMenuTopic) -> StoredGroupMenuPin {
        let menu_topic = menu.storage_key().to_owned();
        match self {
            GroupMenuPin::Group(group) => StoredGroupMenuPin::Group {
                menu_topic,
                group_family: <&'static str>::from(group.family).to_owned(),
                group_instance: group.instance.stored_name().to_owned(),
            },
            GroupMenuPin::Item(IndicatorKey { group, name }) => StoredGroupMenuPin::Item {
                menu_topic,
                group_family: <&'static str>::from(group.family).to_owned(),
                group_instance: group.instance.stored_name().to_owned(),
                indicator_name: name.into_owned(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelPinDuration {
    Permanent,
    UntilAlertEnds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelPin {
    pub menu: GroupMenuTopic,
    pub entry: GroupMenuPin,
    pub duration: PanelPinDuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertPromotion {
    Promoted,
    Unpromoted,
}

pub(crate) enum IndicatorPin {
    Unpinnable,
    Unpinned(GroupMenuPin),
    Pinned(PanelPin),
}

#[derive(Debug, Clone)]
pub enum GroupMenuMessage {
    PinsLoaded(BTreeSet<StoredGroupMenuPin>),
    TogglePin {
        menu: GroupMenuTopic,
        entry: GroupMenuPin,
    },
    DismissAlertPin {
        menu: GroupMenuTopic,
        entry: GroupMenuPin,
    },
    PinSaved(Result<(), Arc<StoreError>>),
    Menu {
        menu: GroupMenuTopic,
        action: NestedMenuAction<IndicatorGroupIdentity>,
    },
    InspectRow {
        menu: GroupMenuTopic,
        indicator: IndicatorKey,
    },
    SettingsExpansion(IndicatorGroupExpansionMessage),
    Appearance(AppearanceMessage),
    PresentationLoaded(NestedMenuPresentation),
    SwitchPresentation(NestedMenuPresentation),
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelRaise {
    pub menu: GroupMenuTopic,
    pub entry: GroupMenuPin,
}

impl PanelRaise {
    fn stored(self) -> StoredGroupMenuPin {
        self.entry.stored_in(self.menu)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenGroupMenu {
    pub topic: GroupMenuTopic,
    pub families: Vec<IndicatorGroupFamily>,
}

#[derive(Debug, Clone)]
struct RecentlyOpenMenu {
    menu: OpenGroupMenu,
    last_open_at: Instant,
}

pub struct GroupMenuState {
    pub layer: LayerAppearanceControls,
    pub(crate) settings_expansion: IndicatorGroupExpansion,
    pub presentation: NestedMenuPresentation,
    pinned: BTreeSet<StoredGroupMenuPin>,
    pub inspection: GroupMenuInspection,
    recently_open_menus: Vec<RecentlyOpenMenu>,
    panel_raises: BTreeSet<StoredGroupMenuPin>,
    dismissed_alert_pins: BTreeSet<StoredGroupMenuPin>,
}

impl Default for GroupMenuState {
    fn default() -> Self {
        Self {
            layer: LayerAppearanceControls::new(ShellLayer::Menus),
            settings_expansion: IndicatorGroupExpansion::default(),
            presentation: NestedMenuPresentation::default(),
            pinned: BTreeSet::new(),
            inspection: GroupMenuInspection::default(),
            recently_open_menus: Vec::new(),
            panel_raises: BTreeSet::new(),
            dismissed_alert_pins: BTreeSet::new(),
        }
    }
}

impl GroupMenuState {
    pub fn load() -> Task<GroupMenuMessage> {
        Task::batch([
            LayerAppearanceControls::load(ShellLayer::Menus).map(GroupMenuMessage::Appearance),
            Task::perform(sqlite_group_menu_pins::load(), GroupMenuMessage::PinsLoaded),
            Task::perform(
                sqlite_group_menu_presentation::load(),
                GroupMenuMessage::PresentationLoaded,
            ),
        ])
    }

    #[must_use]
    pub fn panel_pin(
        &self,
        menu: GroupMenuTopic,
        entry: GroupMenuPin,
        alert_promotion: AlertPromotion,
    ) -> PanelPinOrigin {
        let stored_pin = entry.clone().stored_in(menu);
        let duration = if self.pinned.contains(&stored_pin) {
            PanelPinDuration::Permanent
        } else if alert_promotion == AlertPromotion::Promoted
            && !self.dismissed_alert_pins.contains(&stored_pin)
        {
            PanelPinDuration::UntilAlertEnds
        } else {
            return PanelPinOrigin::Unpinned;
        };
        PanelPinOrigin::Pinned(Box::new(PanelPin {
            menu,
            entry,
            duration,
        }))
    }

    pub(crate) fn indicator_pin(
        &self,
        menu: GroupMenuTopic,
        group: &IndicatorGroupIdentity,
        indicator: &GroupIndicator<'_>,
        promotion_severity: Severity,
    ) -> IndicatorPin {
        let Some(entry) = indicator.menu_pin(group) else {
            return IndicatorPin::Unpinnable;
        };
        match self.panel_pin(
            menu,
            entry.clone(),
            indicator.alert_promotion(promotion_severity),
        ) {
            PanelPinOrigin::Pinned(pin) => IndicatorPin::Pinned(*pin),
            PanelPinOrigin::Unpinned => IndicatorPin::Unpinned(entry),
        }
    }

    pub fn track_source_demand(
        &mut self,
        open_menu: Option<OpenGroupMenu>,
        panel_raises: Vec<PanelRaise>,
        now: Instant,
    ) {
        self.panel_raises = panel_raises.into_iter().map(PanelRaise::stored).collect();
        self.dismissed_alert_pins
            .retain(|dismissed_pin| self.panel_raises.contains(dismissed_pin));
        self.track_open_menu(open_menu, now);
    }

    pub fn track_open_menu(&mut self, open_menu: Option<OpenGroupMenu>, now: Instant) {
        let open_topic = open_menu.as_ref().map(|menu| menu.topic);
        self.recently_open_menus.retain(|recent| {
            Some(recent.menu.topic) != open_topic
                && now
                    .checked_duration_since(recent.last_open_at)
                    .expect("a menu was last open at an earlier shell clock reading")
                    < MENU_SOURCE_LINGER
        });
        if let Some(menu) = open_menu {
            self.recently_open_menus.push(RecentlyOpenMenu {
                menu,
                last_open_at: now,
            });
        }
    }

    #[must_use]
    pub fn source_demand(&self, family: IndicatorGroupFamily) -> GroupSourceDemand {
        let stored_family = <&'static str>::from(family);
        let menu_open = self
            .recently_open_menus
            .iter()
            .any(|recent| recent.menu.families.contains(&family));
        let mut panel_pins = self.pinned.iter().chain(&self.panel_raises);
        let group_on_panel = panel_pins.clone().any(|pin| match pin {
            StoredGroupMenuPin::Group { group_family, .. } => group_family == stored_family,
            StoredGroupMenuPin::Item { .. } => false,
        });
        let indicator_on_panel = panel_pins.any(|pin| match pin {
            StoredGroupMenuPin::Item { group_family, .. } => group_family == stored_family,
            StoredGroupMenuPin::Group { .. } => false,
        });
        let surface = if menu_open || indicator_on_panel {
            GroupSurface::AllRows
        } else if group_on_panel {
            GroupSurface::SummaryOnly
        } else {
            GroupSurface::Hidden
        };
        GroupSourceDemand::showing(surface)
    }

    pub fn collapse_settings(&mut self) {
        if self.settings_expansion.collapse() == ExpansionChange::JustCollapsed {
            self.layer.close();
        }
    }

    fn adopt_presentation(&mut self, presentation: NestedMenuPresentation) {
        self.presentation = presentation;
        self.inspection.adopt(presentation);
    }

    pub fn update(&mut self, message: GroupMenuMessage) -> Task<GroupMenuMessage> {
        match message {
            GroupMenuMessage::PinsLoaded(pinned) => {
                self.pinned = pinned;
                Task::none()
            }
            GroupMenuMessage::TogglePin { menu, entry } => {
                let pin = entry.stored_in(menu);
                let pin_change = if self.pinned.remove(&pin) {
                    PinChange::Unpin
                } else {
                    self.pinned.insert(pin.clone());
                    PinChange::Pin
                };
                Task::perform(
                    sqlite_group_menu_pins::save(pin, pin_change),
                    |store_outcome| GroupMenuMessage::PinSaved(store_outcome.map_err(Arc::new)),
                )
            }
            GroupMenuMessage::DismissAlertPin { menu, entry } => {
                self.dismissed_alert_pins.insert(entry.stored_in(menu));
                Task::none()
            }
            GroupMenuMessage::PinSaved(store_outcome) => {
                if let Err(store_error) = store_outcome {
                    tracing::warn!("group menu pins: the pin change was not saved: {store_error}");
                }
                Task::none()
            }
            GroupMenuMessage::Menu { menu, action } => {
                self.inspection.perform(menu, action);
                Task::none()
            }
            GroupMenuMessage::SettingsExpansion(expansion) => {
                if self.settings_expansion.update(expansion) == ExpansionChange::JustCollapsed {
                    self.layer.close();
                }
                Task::none()
            }
            GroupMenuMessage::Appearance(appearance) => self
                .layer
                .update(appearance)
                .map(GroupMenuMessage::Appearance),
            GroupMenuMessage::PresentationLoaded(presentation) => {
                self.adopt_presentation(presentation);
                Task::none()
            }
            GroupMenuMessage::SwitchPresentation(presentation) => {
                self.adopt_presentation(presentation);
                logged_failure_task(
                    "group menu presentation: save".to_owned(),
                    sqlite_group_menu_presentation::save(presentation),
                )
            }
            GroupMenuMessage::InspectRow { menu, indicator } => {
                self.inspection.inspect_row(menu, indicator);
                Task::none()
            }
            GroupMenuMessage::Closed => {
                self.inspection.close();
                Task::none()
            }
        }
    }
}
