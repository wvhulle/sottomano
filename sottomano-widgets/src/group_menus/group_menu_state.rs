use std::{
    collections::BTreeSet,
    mem::take,
    sync::Arc,
    time::{Duration, Instant},
};

use iced::{Task, task};
use sottomano_desktop_sources::sqlite_state_store::StoreError;
use tokio::time::sleep;

use crate::{
    background_work::logged_failure_task,
    group_menus::{
        group_menu_inspection::GroupMenuInspection,
        group_menu_presentation::GroupMenuPresentation,
        group_menu_settings::GroupMenuSettingsTarget,
        group_source_demand::{GroupSourceDemand, GroupSurface},
        sqlite_group_menu_pins,
        sqlite_group_menu_pins::StoredGroupMenuPin,
        sqlite_group_menu_presentation,
    },
    hover_cards::{HoverTarget, hover_card_surfaces::SWITCH_INTENT},
    widget_groups::{
        widget_group_content::{
            ExpansionChange, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity},
    },
    widgets::{
        layer_appearance_controls::{AppearanceMessage, LayerAppearanceControls},
        phosphor::{Phosphor, Weight},
        shell_layer_appearance::ShellLayer,
    },
};

const MENU_SOURCE_LINGER: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupMenuTopic {
    Hardware,
    Network,
    Agenda,
    Media,
    Display,
    Virtualization,
    Services,
    Session,
    Appearance,
    Behaviour,
    Applications,
    ApplicationMenu,
}

impl GroupMenuTopic {
    pub const ALL: [GroupMenuTopic; 12] = [
        GroupMenuTopic::Hardware,
        GroupMenuTopic::Network,
        GroupMenuTopic::Agenda,
        GroupMenuTopic::Media,
        GroupMenuTopic::Display,
        GroupMenuTopic::Virtualization,
        GroupMenuTopic::Services,
        GroupMenuTopic::Session,
        GroupMenuTopic::Appearance,
        GroupMenuTopic::Behaviour,
        GroupMenuTopic::Applications,
        GroupMenuTopic::ApplicationMenu,
    ];

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            GroupMenuTopic::Hardware => "Hardware",
            GroupMenuTopic::Network => "Network",
            GroupMenuTopic::Agenda => "Agenda",
            GroupMenuTopic::Media => "Media",
            GroupMenuTopic::Display => "Display",
            GroupMenuTopic::Virtualization => "Virtualization",
            GroupMenuTopic::Services => "Services",
            GroupMenuTopic::Session => "Session",
            GroupMenuTopic::Appearance => "Appearance",
            GroupMenuTopic::Behaviour => "Behaviour",
            GroupMenuTopic::Applications => "Applications",
            GroupMenuTopic::ApplicationMenu => "Application menu",
        }
    }

    #[must_use]
    pub const fn glyph(self) -> Phosphor {
        match self {
            GroupMenuTopic::Hardware => Phosphor::Cpu(Weight::Regular),
            GroupMenuTopic::Network => Phosphor::Network(Weight::Regular),
            GroupMenuTopic::Agenda => Phosphor::CalendarBlank(Weight::Regular),
            GroupMenuTopic::Media => Phosphor::MusicNotes(Weight::Regular),
            GroupMenuTopic::Display => Phosphor::Monitor(Weight::Regular),
            GroupMenuTopic::Virtualization => Phosphor::Cube(Weight::Regular),
            GroupMenuTopic::Services => Phosphor::Stack(Weight::Regular),
            GroupMenuTopic::Session => Phosphor::UserCircle(Weight::Regular),
            GroupMenuTopic::Appearance => Phosphor::SlidersHorizontal(Weight::Regular),
            GroupMenuTopic::Behaviour => Phosphor::ToggleRight(Weight::Regular),
            GroupMenuTopic::Applications => Phosphor::SquaresFour(Weight::Regular),
            GroupMenuTopic::ApplicationMenu => Phosphor::AppWindow(Weight::Regular),
        }
    }

    const fn storage_key(self) -> &'static str {
        match self {
            GroupMenuTopic::Hardware => "hardware",
            GroupMenuTopic::Network => "network",
            GroupMenuTopic::Agenda => "agenda",
            GroupMenuTopic::Media => "media",
            GroupMenuTopic::Display => "display",
            GroupMenuTopic::Virtualization => "virtualization",
            GroupMenuTopic::Services => "services",
            GroupMenuTopic::Session => "session",
            GroupMenuTopic::Appearance => "appearance",
            GroupMenuTopic::Behaviour => "behaviour",
            GroupMenuTopic::Applications => "applications",
            GroupMenuTopic::ApplicationMenu => "application_menu",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuPinState {
    Pinned,
    Unpinned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupMenuPin {
    Group(WidgetGroupIdentity),
    Item {
        group: WidgetGroupIdentity,
        item_key: String,
    },
}

impl GroupMenuPin {
    fn stored_in(self, menu: GroupMenuTopic) -> StoredGroupMenuPin {
        let menu_topic = menu.storage_key().to_owned();
        match self {
            GroupMenuPin::Group(group) => StoredGroupMenuPin::Group {
                menu_topic,
                group_family: group.family.stored_name().to_owned(),
                group_instance: group.instance.stored_name().to_owned(),
            },
            GroupMenuPin::Item { group, item_key } => StoredGroupMenuPin::Item {
                menu_topic,
                group_family: group.family.stored_name().to_owned(),
                group_instance: group.instance.stored_name().to_owned(),
                item_key,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelPinDuration {
    Permanent,
    UntilAlertEnds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertPinState {
    Raised,
    Dismissed,
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
    RevealMenuGroup {
        menu: GroupMenuTopic,
        group_identity: WidgetGroupIdentity,
    },
    IntendRevealMenuGroup {
        menu: GroupMenuTopic,
        group_identity: WidgetGroupIdentity,
    },
    CancelRevealIntent {
        group_identity: WidgetGroupIdentity,
    },
    RevealIntentElapsed,
    ToggleMenuGroup {
        menu: GroupMenuTopic,
        group_identity: WidgetGroupIdentity,
    },
    InspectRow {
        menu: GroupMenuTopic,
        target: HoverTarget,
    },
    MenuScrolled {
        menu: GroupMenuTopic,
        scrolled_px: f32,
    },
    ItemListScrolled {
        menu: GroupMenuTopic,
        scrolled_px: f32,
    },
    SettingsExpansion(WidgetGroupExpansionMessage),
    Appearance(AppearanceMessage),
    PresentationLoaded(GroupMenuPresentation),
    SwitchPresentation(GroupMenuPresentation),
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelRaise {
    WholeGroup {
        menu: GroupMenuTopic,
        group: WidgetGroupIdentity,
    },
    AlertingItem {
        menu: GroupMenuTopic,
        group: WidgetGroupIdentity,
        item_key: String,
    },
}

impl PanelRaise {
    fn stored(self) -> StoredGroupMenuPin {
        match self {
            PanelRaise::WholeGroup { menu, group } => GroupMenuPin::Group(group).stored_in(menu),
            PanelRaise::AlertingItem {
                menu,
                group,
                item_key,
            } => GroupMenuPin::Item { group, item_key }.stored_in(menu),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenGroupMenu {
    pub topic: GroupMenuTopic,
    pub families: Vec<WidgetGroupFamily>,
}

#[derive(Debug, Clone)]
struct RecentlyOpenMenu {
    menu: OpenGroupMenu,
    last_open_at: Instant,
}

#[derive(Default)]
enum GroupRevealIntent {
    #[default]
    Idle,
    Pending {
        menu: GroupMenuTopic,
        group_identity: WidgetGroupIdentity,
        _intent_timer: task::Handle,
    },
}

pub struct GroupMenuState {
    reveal_intent: GroupRevealIntent,
    pub layer: LayerAppearanceControls,
    settings_expansion: WidgetGroupExpansion,
    presentation: GroupMenuPresentation,
    pinned: BTreeSet<StoredGroupMenuPin>,
    inspection: GroupMenuInspection,
    recently_open_menus: Vec<RecentlyOpenMenu>,
    panel_raises: BTreeSet<StoredGroupMenuPin>,
    dismissed_alert_pins: BTreeSet<StoredGroupMenuPin>,
}

impl Default for GroupMenuState {
    fn default() -> Self {
        Self {
            reveal_intent: GroupRevealIntent::Idle,
            layer: LayerAppearanceControls::new(ShellLayer::Menus, |target| {
                GroupMenuSettingsTarget::Appearance(target).into()
            }),
            settings_expansion: WidgetGroupExpansion::default(),
            presentation: GroupMenuPresentation::default(),
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
    pub fn pin_state(&self, menu: GroupMenuTopic, entry: GroupMenuPin) -> MenuPinState {
        if self.pinned.contains(&entry.stored_in(menu)) {
            MenuPinState::Pinned
        } else {
            MenuPinState::Unpinned
        }
    }

    #[must_use]
    pub fn alert_pin_state(&self, menu: GroupMenuTopic, entry: GroupMenuPin) -> AlertPinState {
        if self.dismissed_alert_pins.contains(&entry.stored_in(menu)) {
            AlertPinState::Dismissed
        } else {
            AlertPinState::Raised
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
    pub fn source_demand(&self, family: WidgetGroupFamily) -> GroupSourceDemand {
        let stored_family = family.stored_name();
        let menu_open = self
            .recently_open_menus
            .iter()
            .any(|recent| recent.menu.families.contains(&family));
        let mut panel_pins = self.pinned.iter().chain(&self.panel_raises);
        let group_on_panel = panel_pins.clone().any(|pin| match pin {
            StoredGroupMenuPin::Group { group_family, .. } => group_family == stored_family,
            StoredGroupMenuPin::Item { .. } => false,
        });
        let item_on_panel = panel_pins.any(|pin| match pin {
            StoredGroupMenuPin::Item { group_family, .. } => group_family == stored_family,
            StoredGroupMenuPin::Group { .. } => false,
        });
        let surface = if menu_open || item_on_panel {
            GroupSurface::AllRows
        } else if group_on_panel {
            GroupSurface::WidgetGroupFace
        } else {
            GroupSurface::Hidden
        };
        GroupSourceDemand::showing(surface)
    }

    #[must_use]
    pub(crate) const fn settings_expansion(&self) -> &WidgetGroupExpansion {
        &self.settings_expansion
    }

    pub fn collapse_settings(&mut self) {
        if self.settings_expansion.collapse() == ExpansionChange::JustCollapsed {
            self.layer.close();
        }
    }

    #[must_use]
    pub const fn presentation(&self) -> GroupMenuPresentation {
        self.presentation
    }

    #[must_use]
    pub fn scrolled_px(&self, menu: GroupMenuTopic) -> f32 {
        self.inspection.scrolled_px(menu)
    }

    #[must_use]
    pub fn item_list_scrolled_px(&self, menu: GroupMenuTopic) -> f32 {
        self.inspection.item_list_scrolled_px(menu)
    }

    #[must_use]
    pub fn open_groups(&self, menu: GroupMenuTopic) -> &[WidgetGroupIdentity] {
        self.inspection.open_groups(menu)
    }

    #[must_use]
    pub fn inspected_target(&self, menu: GroupMenuTopic) -> Option<&HoverTarget> {
        self.inspection.inspected_target(menu)
    }

    fn adopt_presentation(&mut self, presentation: GroupMenuPresentation) {
        self.presentation = presentation;
        self.inspection =
            take(&mut self.inspection).adopting_open_limit(presentation.open_group_limit());
    }

    fn intend_group_reveal(
        &mut self,
        menu: GroupMenuTopic,
        group_identity: WidgetGroupIdentity,
    ) -> Task<GroupMenuMessage> {
        if self.open_groups(menu).contains(&group_identity) {
            self.reveal_intent = GroupRevealIntent::Idle;
            return Task::none();
        }
        let (intent_elapsed, handle) = Task::future(async { sleep(SWITCH_INTENT).await })
            .map(|()| GroupMenuMessage::RevealIntentElapsed)
            .abortable();
        self.reveal_intent = GroupRevealIntent::Pending {
            menu,
            group_identity,
            _intent_timer: handle.abort_on_drop(),
        };
        intent_elapsed
    }

    fn cancel_group_reveal(&mut self, group_identity: &WidgetGroupIdentity) {
        if matches!(
            &self.reveal_intent,
            GroupRevealIntent::Pending { group_identity: pending_identity, .. }
                if pending_identity == group_identity
        ) {
            self.reveal_intent = GroupRevealIntent::Idle;
        }
    }

    fn reveal_intended_group(&mut self) {
        if let GroupRevealIntent::Pending {
            menu,
            group_identity,
            ..
        } = take(&mut self.reveal_intent)
        {
            self.inspection = take(&mut self.inspection).revealing_group(menu, group_identity);
        }
    }

    pub fn update(&mut self, message: GroupMenuMessage) -> Task<GroupMenuMessage> {
        match message {
            GroupMenuMessage::PinsLoaded(pinned) => {
                self.pinned = pinned;
                Task::none()
            }
            GroupMenuMessage::TogglePin { menu, entry } => {
                let pin = entry.stored_in(menu);
                let pin_state = if self.pinned.remove(&pin) {
                    MenuPinState::Unpinned
                } else {
                    self.pinned.insert(pin.clone());
                    MenuPinState::Pinned
                };
                Task::perform(
                    sqlite_group_menu_pins::save(pin, pin_state),
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
            GroupMenuMessage::RevealMenuGroup {
                menu,
                group_identity,
            } => {
                self.reveal_intent = GroupRevealIntent::Idle;
                self.inspection = take(&mut self.inspection).revealing_group(menu, group_identity);
                Task::none()
            }
            GroupMenuMessage::IntendRevealMenuGroup {
                menu,
                group_identity,
            } => self.intend_group_reveal(menu, group_identity),
            GroupMenuMessage::CancelRevealIntent { group_identity } => {
                self.cancel_group_reveal(&group_identity);
                Task::none()
            }
            GroupMenuMessage::RevealIntentElapsed => {
                self.reveal_intended_group();
                Task::none()
            }
            GroupMenuMessage::ToggleMenuGroup {
                menu,
                group_identity,
            } => {
                self.inspection = take(&mut self.inspection).toggling_group(
                    menu,
                    group_identity,
                    self.presentation.open_group_limit(),
                );
                Task::none()
            }
            GroupMenuMessage::MenuScrolled { menu, scrolled_px } => {
                self.inspection = take(&mut self.inspection).scrolled_to(menu, scrolled_px);
                Task::none()
            }
            GroupMenuMessage::ItemListScrolled { menu, scrolled_px } => {
                self.inspection =
                    take(&mut self.inspection).item_list_scrolled_to(menu, scrolled_px);
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
            GroupMenuMessage::InspectRow { menu, target } => {
                self.inspection = take(&mut self.inspection).inspecting_row(menu, target);
                Task::none()
            }
            GroupMenuMessage::Closed => {
                self.reveal_intent = GroupRevealIntent::Idle;
                self.inspection = GroupMenuInspection::Closed;
                Task::none()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        AlertPinState, GroupMenuMessage, GroupMenuPin, GroupMenuState, GroupMenuTopic,
        GroupSourceDemand, MENU_SOURCE_LINGER, MenuPinState, OpenGroupMenu, PanelRaise,
        WidgetGroupFamily, WidgetGroupIdentity,
    };
    use crate::{
        clock::Clock,
        widget_groups::{
            widget_group_content::ExpansionState, widget_group_identity::WidgetGroupInstance,
        },
    };

    const GPU_GROUP: WidgetGroupIdentity = WidgetGroupIdentity::sole(WidgetGroupFamily::Gpu);

    fn gpu_load_item() -> GroupMenuPin {
        GroupMenuPin::Item {
            group: GPU_GROUP,
            item_key: "gpu_load".to_owned(),
        }
    }

    fn toggle_in_hardware(entry: GroupMenuPin) -> GroupMenuMessage {
        GroupMenuMessage::TogglePin {
            menu: GroupMenuTopic::Hardware,
            entry,
        }
    }

    fn hardware_menu_open() -> OpenGroupMenu {
        OpenGroupMenu {
            topic: GroupMenuTopic::Hardware,
            families: vec![WidgetGroupFamily::Gpu],
        }
    }

    fn gpu_demand(group_menus: &GroupMenuState) -> GroupSourceDemand {
        group_menus.source_demand(WidgetGroupFamily::Gpu)
    }

    fn collapsed_on_panel() -> GroupSourceDemand {
        GroupSourceDemand::on_panel()
    }

    fn player_on_bus(player: &'static str, bus: &'static str) -> WidgetGroupIdentity {
        WidgetGroupIdentity {
            family: WidgetGroupFamily::Media,
            instance: WidgetGroupInstance::Session {
                durable: player.into(),
                session: bus.into(),
            },
        }
    }

    #[test]
    fn toggling_twice_unpins_the_group() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(GPU_GROUP)));
        assert_eq!(
            group_menus.pin_state(GroupMenuTopic::Hardware, GroupMenuPin::Group(GPU_GROUP)),
            MenuPinState::Pinned
        );
        assert_eq!(
            group_menus.pin_state(GroupMenuTopic::Behaviour, GroupMenuPin::Group(GPU_GROUP)),
            MenuPinState::Unpinned
        );
        let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(GPU_GROUP)));
        assert_eq!(
            group_menus.pin_state(GroupMenuTopic::Hardware, GroupMenuPin::Group(GPU_GROUP)),
            MenuPinState::Unpinned
        );
    }

    #[test]
    fn pinning_an_item_leaves_its_group_unpinned() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(gpu_load_item()));
        assert_eq!(
            group_menus.pin_state(GroupMenuTopic::Hardware, gpu_load_item()),
            MenuPinState::Pinned
        );
        assert_eq!(
            group_menus.pin_state(GroupMenuTopic::Hardware, GroupMenuPin::Group(GPU_GROUP)),
            MenuPinState::Unpinned
        );
    }

    #[test]
    fn unpinning_an_expanded_group_drops_its_demand() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(GPU_GROUP)));
        assert_eq!(gpu_demand(&group_menus), collapsed_on_panel());
        let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(GPU_GROUP)));
        assert_ne!(
            gpu_demand(&group_menus).expanded_when(ExpansionState::Expanded),
            collapsed_on_panel().expanded_when(ExpansionState::Expanded)
        );
    }

    #[test]
    fn promoted_and_pinned_groups_have_the_same_demand() {
        let mut group_menus = GroupMenuState::default();
        group_menus.track_source_demand(
            None,
            vec![PanelRaise::WholeGroup {
                menu: GroupMenuTopic::Hardware,
                group: GPU_GROUP,
            }],
            Clock::real().instant,
        );
        assert_eq!(gpu_demand(&group_menus), collapsed_on_panel());
        group_menus.track_source_demand(None, Vec::new(), Clock::real().instant);
        assert_ne!(gpu_demand(&group_menus), collapsed_on_panel());
    }

    fn alerting_gpu_item() -> PanelRaise {
        PanelRaise::AlertingItem {
            menu: GroupMenuTopic::Hardware,
            group: GPU_GROUP,
            item_key: "hot gpu".to_owned(),
        }
    }

    fn alerting_gpu_pin() -> GroupMenuPin {
        GroupMenuPin::Item {
            group: GPU_GROUP,
            item_key: "hot gpu".to_owned(),
        }
    }

    #[test]
    fn a_dismissed_alert_pin_stays_dismissed_while_its_item_keeps_alerting() {
        let mut group_menus = GroupMenuState::default();
        group_menus.track_source_demand(None, vec![alerting_gpu_item()], Clock::real().instant);
        let _ = group_menus.update(GroupMenuMessage::DismissAlertPin {
            menu: GroupMenuTopic::Hardware,
            entry: alerting_gpu_pin(),
        });
        group_menus.track_source_demand(None, vec![alerting_gpu_item()], Clock::real().instant);
        assert_eq!(
            group_menus.alert_pin_state(GroupMenuTopic::Hardware, alerting_gpu_pin()),
            AlertPinState::Dismissed
        );
    }

    #[test]
    fn a_dismissed_alert_pin_rises_again_after_its_alert_ends() {
        let mut group_menus = GroupMenuState::default();
        group_menus.track_source_demand(None, vec![alerting_gpu_item()], Clock::real().instant);
        let _ = group_menus.update(GroupMenuMessage::DismissAlertPin {
            menu: GroupMenuTopic::Hardware,
            entry: alerting_gpu_pin(),
        });
        group_menus.track_source_demand(None, Vec::new(), Clock::real().instant);
        group_menus.track_source_demand(None, vec![alerting_gpu_item()], Clock::real().instant);
        assert_eq!(
            group_menus.alert_pin_state(GroupMenuTopic::Hardware, alerting_gpu_pin()),
            AlertPinState::Raised
        );
    }

    #[test]
    fn pinning_one_sibling_item_leaves_the_other() {
        let sibling_item = |item_key: &str| GroupMenuPin::Item {
            group: GPU_GROUP,
            item_key: item_key.to_owned(),
        };
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(sibling_item(
            "WireguardTunnel(WireguardTunnelTarget { interface: \"wg0\" })",
        )));
        assert_eq!(
            group_menus.pin_state(
                GroupMenuTopic::Hardware,
                sibling_item("WireguardTunnel(WireguardTunnelTarget { interface: \"wg0\" })")
            ),
            MenuPinState::Pinned
        );
        assert_eq!(
            group_menus.pin_state(
                GroupMenuTopic::Hardware,
                sibling_item("WireguardTunnel(WireguardTunnelTarget { interface: \"wg1\" })")
            ),
            MenuPinState::Unpinned
        );
    }

    #[test]
    fn pinned_item_demand_is_limited_to_its_own_family() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(gpu_load_item()));
        assert_ne!(
            gpu_demand(&group_menus),
            group_menus.source_demand(WidgetGroupFamily::Compute)
        );
    }

    #[test]
    fn a_pinned_instance_demands_the_source_its_family_shares() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(
            WidgetGroupIdentity::durable(WidgetGroupFamily::EthernetPorts, "enp3s0"),
        )));
        assert_eq!(
            group_menus.source_demand(WidgetGroupFamily::EthernetPorts),
            collapsed_on_panel()
        );
        assert_ne!(
            group_menus.source_demand(WidgetGroupFamily::WifiAdapters),
            collapsed_on_panel()
        );
    }

    #[test]
    fn demand_outlasts_the_open_menu_for_the_linger_only() {
        let mut group_menus = GroupMenuState::default();
        let opened_at = Clock::real().instant;
        group_menus.track_source_demand(Some(hardware_menu_open()), Vec::new(), opened_at);
        assert_ne!(gpu_demand(&group_menus), collapsed_on_panel());
        group_menus.track_source_demand(None, Vec::new(), opened_at + Duration::from_secs(1));
        assert_ne!(
            gpu_demand(&group_menus),
            GroupMenuState::default().source_demand(WidgetGroupFamily::Gpu)
        );
        group_menus.track_source_demand(None, Vec::new(), opened_at + MENU_SOURCE_LINGER);
        assert_eq!(
            gpu_demand(&group_menus),
            GroupMenuState::default().source_demand(WidgetGroupFamily::Gpu)
        );
    }

    #[test]
    fn a_pin_follows_its_source_not_its_position() {
        let first_port = WidgetGroupIdentity::durable(WidgetGroupFamily::EthernetPorts, "enp3s0");
        let second_port = WidgetGroupIdentity::durable(WidgetGroupFamily::EthernetPorts, "enp4s0");
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(second_port.clone())));
        assert_eq!(
            group_menus.pin_state(GroupMenuTopic::Hardware, GroupMenuPin::Group(second_port)),
            MenuPinState::Pinned
        );
        assert_eq!(
            group_menus.pin_state(GroupMenuTopic::Hardware, GroupMenuPin::Group(first_port)),
            MenuPinState::Unpinned
        );
    }

    #[test]
    fn namesake_players_share_a_pin() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(player_on_bus(
            "Firefox", ":1.40",
        ))));
        assert_eq!(
            group_menus.pin_state(
                GroupMenuTopic::Hardware,
                GroupMenuPin::Group(player_on_bus("Firefox", ":1.41"))
            ),
            MenuPinState::Pinned
        );
        assert_eq!(
            group_menus.pin_state(
                GroupMenuTopic::Hardware,
                GroupMenuPin::Group(player_on_bus("Spotify", ":1.42"))
            ),
            MenuPinState::Unpinned
        );
    }

    #[test]
    fn closing_forgets_the_open_group() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(GroupMenuMessage::RevealMenuGroup {
            menu: GroupMenuTopic::Hardware,
            group_identity: GPU_GROUP,
        });
        assert_eq!(
            group_menus.open_groups(GroupMenuTopic::Hardware),
            [GPU_GROUP]
        );
        let _ = group_menus.update(GroupMenuMessage::Closed);
        assert!(group_menus.open_groups(GroupMenuTopic::Hardware).is_empty());
    }

    fn state_intending_gpu_reveal() -> GroupMenuState {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(GroupMenuMessage::IntendRevealMenuGroup {
            menu: GroupMenuTopic::Hardware,
            group_identity: GPU_GROUP,
        });
        group_menus
    }

    #[test]
    fn an_intended_reveal_opens_the_group_only_once_its_timer_elapses() {
        let mut group_menus = state_intending_gpu_reveal();
        assert!(group_menus.open_groups(GroupMenuTopic::Hardware).is_empty());
        let _ = group_menus.update(GroupMenuMessage::RevealIntentElapsed);
        assert_eq!(
            group_menus.open_groups(GroupMenuTopic::Hardware),
            [GPU_GROUP]
        );
    }

    #[test]
    fn leaving_the_intended_row_cancels_its_reveal() {
        let mut group_menus = state_intending_gpu_reveal();
        let _ = group_menus.update(GroupMenuMessage::CancelRevealIntent {
            group_identity: GPU_GROUP,
        });
        let _ = group_menus.update(GroupMenuMessage::RevealIntentElapsed);
        assert!(group_menus.open_groups(GroupMenuTopic::Hardware).is_empty());
    }

    #[test]
    fn leaving_another_row_keeps_the_intended_reveal() {
        let mut group_menus = state_intending_gpu_reveal();
        let _ = group_menus.update(GroupMenuMessage::CancelRevealIntent {
            group_identity: WidgetGroupIdentity::sole(WidgetGroupFamily::Compute),
        });
        let _ = group_menus.update(GroupMenuMessage::RevealIntentElapsed);
        assert_eq!(
            group_menus.open_groups(GroupMenuTopic::Hardware),
            [GPU_GROUP]
        );
    }

    #[test]
    fn a_press_reveals_at_once_and_closing_drops_an_intended_reveal() {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(GroupMenuMessage::RevealMenuGroup {
            menu: GroupMenuTopic::Hardware,
            group_identity: GPU_GROUP,
        });
        assert_eq!(
            group_menus.open_groups(GroupMenuTopic::Hardware),
            [GPU_GROUP]
        );
        let mut closed_menus = state_intending_gpu_reveal();
        let _ = closed_menus.update(GroupMenuMessage::Closed);
        let _ = closed_menus.update(GroupMenuMessage::RevealIntentElapsed);
        assert!(
            closed_menus
                .open_groups(GroupMenuTopic::Hardware)
                .is_empty()
        );
    }

    #[test]
    fn an_open_group_survives_pinning_an_earlier_group() {
        let compute = WidgetGroupIdentity::sole(WidgetGroupFamily::Compute);
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(GroupMenuMessage::RevealMenuGroup {
            menu: GroupMenuTopic::Hardware,
            group_identity: GPU_GROUP,
        });
        let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(compute)));
        assert_eq!(
            group_menus.open_groups(GroupMenuTopic::Hardware),
            [GPU_GROUP]
        );
    }
}
