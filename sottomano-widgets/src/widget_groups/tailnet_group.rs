use std::{borrow::Cow, path::PathBuf};

use chrono::Locale;
use iced::{Subscription, Task};
use sottomano_desktop_sinks::network_connectivity::tailscale_cli_switches::{
    open_admin, switch_exit_offer, switch_exit_routing, switch_tailscale,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        tailscale_exit_nodes::{
            ExitNodeChoices, ExitNodeControl, ExitNodeLanAccess, ExitNodeOption, ExitRoutingSwitch,
            TailscaleExitNodes,
        },
        tailscale_inbound_access::TailscaleInboundAccess,
        tailscale_policy_file::TailscalePolicyFile,
        tailscale_status::{
            ExitNodeUse, ExitOfferControl, ExitOfferSwitch, Tailnet, Tailscale, TailscaleSwitch,
        },
    },
    system_protocol_clients::{
        dbus_desktop_buses::{DesktopBuses, OnBus},
        shell_command_output,
    },
};
use sottomano_toml_config::Config;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{
        group_item::GroupItem, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState, toggle_control::SwitchPosition,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{HoverTarget, hover_card_body::Section},
    widget_groups::{
        tailnet_group::{
            tailscale_access_hover_card::{InboundAccessRow, OwnNodeRow, PolicyFileRow},
            tailscale_exit_node_hover_card::{ExitNodeChoiceRow, ExitNodeLanAccessRow},
            tailscale_exit_offer_hover_card::ExitOfferRow,
        },
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, simple_icons::SimpleIcon,
        subscribed_snapshot::SubscribedSnapshot,
    },
};

mod tailscale_access_hover_card;
mod tailscale_exit_node_hover_card;
mod tailscale_exit_offer_hover_card;
mod tailscale_mesh_hover_card;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TailnetTarget {
    Membership,
    OwnNode,
    InboundAccess,
    PolicyFile,
    ExitOffer,
    ExitNodeChoice { exit_node_ip: Cow<'static, str> },
    ExitNodeLanAccess,
    ExitNodeListFailure,
}

impl From<TailnetTarget> for HoverTarget {
    fn from(target: TailnetTarget) -> Self {
        HoverTarget::Tailnet(target)
    }
}

enum PolicyFileAccess {
    NoApiToken,
    ApiToken {
        token_path: PathBuf,
        policy_file: Box<HoverFetchedSnapshot<TailscalePolicyFile>>,
    },
}

impl PolicyFileAccess {
    fn stop_hover_fetch(&mut self) {
        if let PolicyFileAccess::ApiToken { policy_file, .. } = self {
            policy_file.stop();
        }
    }

    fn receive(&mut self, delivery: SnapshotDelivery<TailscalePolicyFile>, clock: Clock) {
        if let PolicyFileAccess::ApiToken { policy_file, .. } = self {
            policy_file.receive(delivery, clock);
        }
    }

    fn sections(&self, clock: Clock) -> Vec<Section> {
        match self {
            PolicyFileAccess::NoApiToken => Vec::new(),
            PolicyFileAccess::ApiToken { policy_file, .. } => policy_file.sections(clock),
        }
    }
}

pub struct TailnetGroup {
    tailscale: SubscribedSnapshot<Tailscale>,
    tailscale_exit_nodes: SubscribedSnapshot<TailscaleExitNodes>,
    inbound_access: HoverFetchedSnapshot<TailscaleInboundAccess>,
    policy_file_access: PolicyFileAccess,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum TailnetMessage {
    Tailscale(SnapshotDelivery<Tailscale>),
    ExitNodesDelivered(SnapshotDelivery<TailscaleExitNodes>),
    InboundAccessFetched(SnapshotDelivery<TailscaleInboundAccess>),
    PolicyFileFetched(SnapshotDelivery<TailscalePolicyFile>),
    Interaction(TailnetInteraction),
}

#[derive(Debug, Clone)]
pub enum TailnetInteraction {
    Expansion(WidgetGroupExpansionMessage),
    SwitchTailscale(TailscaleSwitch),
    SwitchExitOffer(ExitOfferSwitch),
    SwitchExitRouting(ExitRoutingSwitch),
    OpenAdminConsole,
}

fn refreshed_after_switch(
    switch_label: String,
    switch_work: impl Future<Output = Result<(), shell_command_output::Error>> + Send + 'static,
) -> Task<TailnetMessage> {
    Task::future(switch_work).then(move |switched| {
        if let Err(command_error) = switched {
            tracing::warn!("{switch_label}: {command_error}");
        }
        TailnetGroup::refresh()
    })
}

impl TailnetGroup {
    #[must_use]
    pub fn without_policy_file() -> Self {
        Self::with_access(PolicyFileAccess::NoApiToken)
    }

    #[must_use]
    pub fn with_policy_file(token_path: PathBuf) -> Self {
        Self::with_access(PolicyFileAccess::ApiToken {
            token_path,
            policy_file: Box::default(),
        })
    }

    fn with_access(policy_file_access: PolicyFileAccess) -> Self {
        Self {
            tailscale: SubscribedSnapshot::default(),
            tailscale_exit_nodes: SubscribedSnapshot::default(),
            inbound_access: HoverFetchedSnapshot::default(),
            policy_file_access,
            expansion: WidgetGroupExpansion::default(),
        }
    }

    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, demand: &GroupSourceDemand) -> Subscription<TailnetMessage> {
        Subscription::batch([
            demand.while_face_shown(
                snapshot_subscription::<Tailscale>(()).map(TailnetMessage::Tailscale),
            ),
            demand.while_face_shown(
                snapshot_subscription::<TailscaleExitNodes>(())
                    .map(TailnetMessage::ExitNodesDelivered),
            ),
        ])
    }

    pub fn refresh() -> Task<TailnetMessage> {
        Task::batch([
            first_delivery::<Tailscale>(()).map(TailnetMessage::Tailscale),
            first_delivery::<TailscaleExitNodes>(()).map(TailnetMessage::ExitNodesDelivered),
        ])
    }

    pub fn update(&mut self, message: TailnetMessage, clock: Clock) -> Task<TailnetMessage> {
        match message {
            TailnetMessage::Tailscale(delivery) => self.tailscale.receive(delivery, clock),
            TailnetMessage::ExitNodesDelivered(delivery) => {
                self.tailscale_exit_nodes.receive(delivery, clock);
            }
            TailnetMessage::InboundAccessFetched(delivery) => {
                self.inbound_access.receive(delivery, clock);
            }
            TailnetMessage::PolicyFileFetched(delivery) => {
                self.policy_file_access.receive(delivery, clock);
            }
            TailnetMessage::Interaction(TailnetInteraction::Expansion(expansion_message)) => {
                self.expansion.update(expansion_message);
            }
            TailnetMessage::Interaction(TailnetInteraction::SwitchTailscale(tailscale_switch)) => {
                return refreshed_after_switch(
                    format!("tailscale {tailscale_switch:?}"),
                    switch_tailscale(tailscale_switch),
                );
            }
            TailnetMessage::Interaction(TailnetInteraction::SwitchExitOffer(exit_offer_switch)) => {
                return refreshed_after_switch(
                    format!("tailscale exit offer {exit_offer_switch:?}"),
                    switch_exit_offer(exit_offer_switch),
                );
            }
            TailnetMessage::Interaction(TailnetInteraction::SwitchExitRouting(
                exit_routing_switch,
            )) => {
                return refreshed_after_switch(
                    format!("tailscale exit routing {exit_routing_switch:?}"),
                    switch_exit_routing(exit_routing_switch),
                );
            }
            TailnetMessage::Interaction(TailnetInteraction::OpenAdminConsole) => {
                use sottomano_desktop_sources::blocking_background_work::blocking;
                return Task::future(blocking("tailscale admin", || {
                    if let Err(open_failure) = open_admin() {
                        tracing::warn!("tailscale admin console: {open_failure}");
                    }
                }))
                .discard();
            }
        }
        Task::none()
    }

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
        buses: Option<&DesktopBuses>,
        clock: Clock,
    ) -> Task<TailnetMessage> {
        self.inbound_access.stop();
        self.policy_file_access.stop_hover_fetch();
        match hovered {
            Some(HoverTarget::Tailnet(TailnetTarget::InboundAccess)) => self
                .inbound_access
                .start_hover_fetch((), TailnetMessage::InboundAccessFetched, config, clock),
            Some(HoverTarget::Tailnet(TailnetTarget::PolicyFile)) => {
                let (
                    PolicyFileAccess::ApiToken {
                        token_path,
                        policy_file,
                    },
                    Some(buses),
                ) = (&mut self.policy_file_access, buses)
                else {
                    return Task::none();
                };
                policy_file.start_hover_fetch(
                    OnBus {
                        buses: buses.clone(),
                        inputs: token_path.clone(),
                    },
                    TailnetMessage::PolicyFileFetched,
                    config,
                    clock,
                )
            }
            _ => Task::none(),
        }
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &TailnetTarget, clock: Clock) -> Vec<Section> {
        match target {
            TailnetTarget::Membership => self.tailscale.hover_card(clock),
            TailnetTarget::ExitOffer => {
                let Some(tailnet) = self.running_tailnet() else {
                    return Vec::new();
                };
                self.tailscale
                    .hover_card_with(ExitOfferRow { tailnet }.hover_card(clock))
            }
            TailnetTarget::OwnNode => {
                let Some(own_node_row) =
                    self.tailscale.get().and_then(OwnNodeRow::running_own_node)
                else {
                    return Vec::new();
                };
                self.tailscale
                    .hover_card_with(own_node_row.hover_card(clock))
            }
            TailnetTarget::InboundAccess => self.inbound_access.sections(clock),
            TailnetTarget::PolicyFile => self.policy_file_access.sections(clock),
            TailnetTarget::ExitNodeChoice { exit_node_ip } => {
                let Some(exit_node_choices) = self.running_exit_node_choices() else {
                    return Vec::new();
                };
                let Some(exit_node_option) = exit_node_choices
                    .options
                    .iter()
                    .find(|exit_node_option| exit_node_option.ip == exit_node_ip.as_ref())
                else {
                    return Vec::new();
                };
                self.tailscale_exit_nodes.hover_card_with(
                    ExitNodeChoiceRow {
                        exit_node_option,
                        exit_node_choices,
                    }
                    .hover_card(clock),
                )
            }
            TailnetTarget::ExitNodeLanAccess => {
                let Some(exit_node_choices) = self.running_exit_node_choices() else {
                    return Vec::new();
                };
                self.tailscale_exit_nodes
                    .hover_card_with(ExitNodeLanAccessRow { exit_node_choices }.hover_card(clock))
            }
            TailnetTarget::ExitNodeListFailure => self.tailscale_exit_nodes.hover_card(clock),
        }
    }

    fn membership_item(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> Option<GroupItem<'_, TailnetInteraction>> {
        let item = GroupItem::reading(
            self.tailscale.display()?,
            TailnetTarget::Membership,
            clock,
            locale,
        )
        .on_right_press(
            "open the Tailscale admin console",
            TailnetInteraction::OpenAdminConsole,
        );
        Some(match self.tailscale.get().map(Tailscale::switch) {
            Some(TailscaleSwitch::Up) => item.on_switch_press(
                SwitchPosition::Off,
                "turn Tailscale on",
                TailnetInteraction::SwitchTailscale(TailscaleSwitch::Up),
            ),
            Some(TailscaleSwitch::Down) => item.on_switch_press(
                SwitchPosition::On,
                "turn Tailscale off",
                TailnetInteraction::SwitchTailscale(TailscaleSwitch::Down),
            ),
            None => item,
        })
    }

    fn running_tailnet(&self) -> Option<&Tailnet> {
        match self.tailscale.get()? {
            Tailscale::Running(tailnet) => Some(tailnet),
            Tailscale::Stopped | Tailscale::Starting => None,
        }
    }

    fn policy_file_item(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> Option<GroupItem<'_, TailnetInteraction>> {
        self.running_tailnet()?;
        match self.policy_file_access {
            PolicyFileAccess::NoApiToken => None,
            PolicyFileAccess::ApiToken { .. } => Some(GroupItem::reading(
                PolicyFileRow,
                TailnetTarget::PolicyFile,
                clock,
                locale,
            )),
        }
    }

    fn exit_offer_item(
        tailnet: &Tailnet,
        clock: Clock,
        locale: Locale,
    ) -> GroupItem<'_, TailnetInteraction> {
        let item = GroupItem::reading(
            ExitOfferRow { tailnet },
            TailnetTarget::ExitOffer,
            clock,
            locale,
        );
        match tailnet.offer_control() {
            ExitOfferControl::Switchable(exit_offer_switch @ ExitOfferSwitch::StopOffering) => item
                .on_switch_press(
                    SwitchPosition::On,
                    "stop offering this machine as an exit node",
                    TailnetInteraction::SwitchExitOffer(exit_offer_switch),
                ),
            ExitOfferControl::Switchable(exit_offer_switch @ ExitOfferSwitch::StartOffering) => {
                item.on_switch_press(
                    SwitchPosition::Off,
                    "offer this machine as an exit node",
                    TailnetInteraction::SwitchExitOffer(exit_offer_switch),
                )
            }
            ExitOfferControl::BlockedByExitNodeInUse => item,
        }
    }

    fn running_exit_node_choices(&self) -> Option<&ExitNodeChoices> {
        match self.tailscale_exit_nodes.get()? {
            TailscaleExitNodes::Running(exit_node_choices) => Some(exit_node_choices),
            TailscaleExitNodes::NotRunning => None,
        }
    }

    fn exit_node_choice_item<'a>(
        exit_node_option: &'a ExitNodeOption,
        exit_node_choices: &'a ExitNodeChoices,
        clock: Clock,
        locale: Locale,
    ) -> GroupItem<'a, TailnetInteraction> {
        let exit_node_item = GroupItem::reading(
            ExitNodeChoiceRow {
                exit_node_option,
                exit_node_choices,
            },
            TailnetTarget::ExitNodeChoice {
                exit_node_ip: Cow::Owned(exit_node_option.ip.clone()),
            },
            clock,
            locale,
        );
        let exit_node_item = match exit_node_option.node_use {
            ExitNodeUse::InUse => exit_node_item.previewed_on_handle(),
            ExitNodeUse::Unused => exit_node_item,
        };
        match exit_node_choices.control_for_option(exit_node_option) {
            ExitNodeControl::Switchable(exit_routing_switch) => exit_node_item.on_switch_press(
                exit_node_option.node_use,
                match exit_node_option.node_use {
                    ExitNodeUse::InUse => "stop using the exit node",
                    ExitNodeUse::Unused => "use the exit node",
                },
                TailnetInteraction::SwitchExitRouting(exit_routing_switch),
            ),
            ExitNodeControl::BlockedByOffering | ExitNodeControl::Offline => exit_node_item,
        }
    }

    fn exit_node_lan_access_item(
        exit_node_choices: &ExitNodeChoices,
        clock: Clock,
        locale: Locale,
    ) -> GroupItem<'_, TailnetInteraction> {
        GroupItem::reading(
            ExitNodeLanAccessRow { exit_node_choices },
            TailnetTarget::ExitNodeLanAccess,
            clock,
            locale,
        )
        .on_switch_press(
            exit_node_choices.lan_access,
            match exit_node_choices.lan_access {
                ExitNodeLanAccess::Allowed => "cut local network access through exit nodes",
                ExitNodeLanAccess::Blocked => "keep local network access through exit nodes",
            },
            TailnetInteraction::SwitchExitRouting(exit_node_choices.lan_access_switch()),
        )
    }

    fn exit_node_choice_items(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> Vec<GroupItem<'_, TailnetInteraction>> {
        let Some(exit_node_choices) = self.running_exit_node_choices() else {
            return Vec::new();
        };
        exit_node_choices
            .options
            .iter()
            .map(|exit_node_option| {
                Self::exit_node_choice_item(exit_node_option, exit_node_choices, clock, locale)
            })
            .chain(
                (!exit_node_choices.options.is_empty())
                    .then(|| Self::exit_node_lan_access_item(exit_node_choices, clock, locale)),
            )
            .collect()
    }

    pub fn group_content(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> WidgetGroupContent<'_, TailnetInteraction> {
        let own_node_item = self
            .tailscale
            .get()
            .and_then(OwnNodeRow::running_own_node)
            .map(|own_node_row| {
                GroupItem::reading(own_node_row, TailnetTarget::OwnNode, clock, locale)
            });
        let inbound_access_item = self.running_tailnet().map(|_| {
            GroupItem::reading(
                InboundAccessRow,
                TailnetTarget::InboundAccess,
                clock,
                locale,
            )
        });
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Tailnet,
            &self.expansion,
            TailnetInteraction::Expansion,
            self.membership_item(clock, locale)
                .into_iter()
                .chain(own_node_item)
                .chain(inbound_access_item)
                .chain(self.policy_file_item(clock, locale))
                .chain(
                    self.running_tailnet()
                        .map(|tailnet| Self::exit_offer_item(tailnet, clock, locale)),
                )
                .chain(self.exit_node_choice_items(clock, locale))
                .chain(self.tailscale_exit_nodes.failure().map(|failure_reading| {
                    GroupItem::reading(
                        failure_reading,
                        TailnetTarget::ExitNodeListFailure,
                        clock,
                        locale,
                    )
                })),
        )
        .face(WidgetGroupFace::Glyph {
            glyph: SimpleIcon::Tailscale.into(),
            state: ReadingDisplayState::Idle,
        })
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::network_connectivity::{
        tailscale_exit_nodes::ExitNodeReachability,
        tailscale_status::{ExitNodeOffer, KeyExpiry, Peer, PeerPath},
    };

    use super::*;
    use crate::group_items::group_item_gestures::GroupItemGesture;

    fn tailnet_offering(exit_offer: ExitNodeOffer, exit_node_use: ExitNodeUse) -> Tailnet {
        Tailnet {
            tailnet_name: "example.ts.net".to_owned(),
            own_node: Peer {
                name: "workstation".to_owned(),
                ip: "100.1.1.9".to_owned(),
                os: "linux".to_owned(),
                path: PeerPath::Idle,
                received_bytes: 0,
                sent_bytes: 0,
                tags: Vec::new(),
            },
            online: Vec::new(),
            total_peers: 0,
            own_capabilities: Vec::new(),
            exit_offer,
            exit_node_use,
            key_expiry: KeyExpiry::Disabled,
            health_warnings: Vec::new(),
        }
    }

    #[test]
    fn the_offer_row_is_inert_while_an_exit_node_is_in_use() {
        let clock = Clock::real();
        let locale = Locale::default();
        let tailnet = tailnet_offering(ExitNodeOffer::NotOffered, ExitNodeUse::InUse);
        assert!(matches!(
            TailnetGroup::exit_offer_item(&tailnet, clock, locale)
                .gestures()
                .left,
            GroupItemGesture::Inert
        ));
    }

    fn group_running_tailnet(tailnet_group: TailnetGroup) -> TailnetGroup {
        let mut running_group = tailnet_group;
        let _ = running_group.update(
            TailnetMessage::Tailscale(SnapshotDelivery::live_push(Ok(Tailscale::Running(
                Box::new(tailnet_offering(
                    ExitNodeOffer::NotOffered,
                    ExitNodeUse::Unused,
                )),
            )))),
            Clock::real(),
        );
        running_group
    }

    #[test]
    fn the_policy_file_row_needs_an_api_token() {
        let clock = Clock::real();
        let locale = Locale::default();
        let without_token = group_running_tailnet(TailnetGroup::without_policy_file());
        assert!(without_token.policy_file_item(clock, locale).is_none());
        let with_token =
            group_running_tailnet(TailnetGroup::with_policy_file(PathBuf::from("token")));
        assert!(with_token.policy_file_item(clock, locale).is_some());
    }

    #[test]
    fn an_approved_offer_can_be_stopped() {
        let clock = Clock::real();
        let locale = Locale::default();
        let tailnet = tailnet_offering(ExitNodeOffer::Approved, ExitNodeUse::Unused);
        assert!(matches!(
            TailnetGroup::exit_offer_item(&tailnet, clock, locale)
                .gestures()
                .left,
            GroupItemGesture::Performs {
                message: TailnetInteraction::SwitchExitOffer(ExitOfferSwitch::StopOffering),
                ..
            }
        ));
    }

    fn reachable_exit_node(reachability: ExitNodeReachability) -> ExitNodeOption {
        ExitNodeOption {
            name: "zbook".to_owned(),
            ip: "100.1.1.1".to_owned(),
            reachability,
            node_use: ExitNodeUse::Unused,
        }
    }

    fn exit_node_choices_while(exit_offer: ExitNodeOffer) -> ExitNodeChoices {
        ExitNodeChoices {
            options: Vec::new(),
            offer: exit_offer,
            lan_access: ExitNodeLanAccess::Blocked,
        }
    }

    #[test]
    fn an_online_exit_node_row_can_be_pressed() {
        let clock = Clock::real();
        let locale = Locale::default();
        assert!(!matches!(
            TailnetGroup::exit_node_choice_item(
                &reachable_exit_node(ExitNodeReachability::Online),
                &exit_node_choices_while(ExitNodeOffer::NotOffered),
                clock,
                locale
            )
            .gestures()
            .left,
            GroupItemGesture::Inert
        ));
    }

    #[test]
    fn an_exit_node_row_is_inert_while_this_machine_advertises() {
        let clock = Clock::real();
        let locale = Locale::default();
        assert!(matches!(
            TailnetGroup::exit_node_choice_item(
                &reachable_exit_node(ExitNodeReachability::Online),
                &exit_node_choices_while(ExitNodeOffer::Approved),
                clock,
                locale
            )
            .gestures()
            .left,
            GroupItemGesture::Inert
        ));
    }

    #[test]
    fn an_offline_exit_node_row_is_inert() {
        let clock = Clock::real();
        let locale = Locale::default();
        assert!(matches!(
            TailnetGroup::exit_node_choice_item(
                &reachable_exit_node(ExitNodeReachability::OfflineNeverSeen),
                &exit_node_choices_while(ExitNodeOffer::NotOffered),
                clock,
                locale
            )
            .gestures()
            .left,
            GroupItemGesture::Inert
        ));
    }
}
