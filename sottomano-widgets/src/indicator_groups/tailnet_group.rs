use std::path::PathBuf;

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
    glyph_icons::{
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::Section,
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
        tailnet_group::{
            tailscale_access_hover_card::{
                InboundAccessIndicator, OwnNodeIndicator, PolicyFileIndicator,
                tailscale_inbound_access_sections, tailscale_policy_file_sections,
            },
            tailscale_exit_node_hover_card::{ExitNodeChoiceIndicator, ExitNodeLanAccessIndicator},
            tailscale_exit_offer_hover_card::ExitOfferIndicator,
            tailscale_mesh_hover_card::{tailscale_hover_card, tailscale_indicator},
        },
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, subscribed_snapshot::SubscribedSnapshot,
    },
};

const TAILNET_MEMBERSHIP_GLYPH: Phosphor = Phosphor::Plugs(Weight::Regular);
const TAILSCALE_EXIT_NODES_GLYPH: Phosphor = Phosphor::SignOut(Weight::Regular);

mod tailscale_access_hover_card;
mod tailscale_exit_node_hover_card;
mod tailscale_exit_offer_hover_card;
mod tailscale_mesh_hover_card;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TailnetBusRequest {
    StartPolicyFileFetch,
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

    fn sections(&self) -> Vec<Section> {
        match self {
            PolicyFileAccess::NoApiToken => Vec::new(),
            PolicyFileAccess::ApiToken { policy_file, .. } => {
                policy_file.sections_described_by(tailscale_policy_file_sections)
            }
        }
    }
}

pub struct TailnetGroup {
    tailscale: SubscribedSnapshot<Tailscale>,
    tailscale_exit_nodes: SubscribedSnapshot<TailscaleExitNodes>,
    inbound_access: HoverFetchedSnapshot<TailscaleInboundAccess>,
    policy_file_access: PolicyFileAccess,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum TailnetMessage {
    Tailscale(SnapshotDelivery<Tailscale>),
    ExitNodesDelivered(SnapshotDelivery<TailscaleExitNodes>),
    InboundAccessFetched(SnapshotDelivery<TailscaleInboundAccess>),
    PolicyFileFetched(SnapshotDelivery<TailscalePolicyFile>),
}

#[derive(Debug, Clone)]
pub enum TailnetInteraction {
    Expansion(IndicatorGroupExpansionMessage),
    SwitchTailscale(TailscaleSwitch),
    SwitchExitOffer(ExitOfferSwitch),
    SwitchExitRouting(ExitRoutingSwitch),
    OpenAdminConsole,
    StartInboundAccessFetch,
    StopHoverFetches,
    Refresh,
    OnBuses(TailnetBusRequest),
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
            expansion: IndicatorGroupExpansion::default(),
        }
    }

    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, demand: &GroupSourceDemand) -> Subscription<TailnetMessage> {
        Subscription::batch([
            demand.while_headline_shown(
                snapshot_subscription::<Tailscale>(()).map(TailnetMessage::Tailscale),
            ),
            demand.while_headline_shown(
                snapshot_subscription::<TailscaleExitNodes>(())
                    .map(TailnetMessage::ExitNodesDelivered),
            ),
        ])
    }

    fn refresh() -> Task<TailnetMessage> {
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
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: TailnetInteraction,
        config: &Config,
        clock: Clock,
    ) -> Task<TailnetMessage> {
        match interaction {
            TailnetInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
            }
            TailnetInteraction::SwitchTailscale(tailscale_switch) => {
                return refreshed_after_switch(
                    format!("tailscale {tailscale_switch:?}"),
                    switch_tailscale(tailscale_switch),
                );
            }
            TailnetInteraction::SwitchExitOffer(exit_offer_switch) => {
                return refreshed_after_switch(
                    format!("tailscale exit offer {exit_offer_switch:?}"),
                    switch_exit_offer(exit_offer_switch),
                );
            }
            TailnetInteraction::SwitchExitRouting(exit_routing_switch) => {
                return refreshed_after_switch(
                    format!("tailscale exit routing {exit_routing_switch:?}"),
                    switch_exit_routing(exit_routing_switch),
                );
            }
            TailnetInteraction::OpenAdminConsole => {
                use sottomano_desktop_sources::blocking_background_work::blocking;
                return Task::future(blocking("tailscale admin", || {
                    if let Err(open_failure) = open_admin() {
                        tracing::warn!("tailscale admin console: {open_failure}");
                    }
                }))
                .discard();
            }
            TailnetInteraction::StartInboundAccessFetch => {
                return self.inbound_access.start_hover_fetch(
                    (),
                    TailnetMessage::InboundAccessFetched,
                    config,
                    clock,
                );
            }
            TailnetInteraction::StopHoverFetches => {
                self.inbound_access.stop();
                self.policy_file_access.stop_hover_fetch();
            }
            TailnetInteraction::Refresh => return Self::refresh(),
            TailnetInteraction::OnBuses(_) => {}
        }
        Task::none()
    }

    pub fn on_buses(
        &mut self,
        request: TailnetBusRequest,
        config: &Config,
        buses: &DesktopBuses,
        clock: Clock,
    ) -> Task<TailnetMessage> {
        let TailnetBusRequest::StartPolicyFileFetch = request;
        let PolicyFileAccess::ApiToken {
            token_path,
            policy_file,
        } = &mut self.policy_file_access
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

    fn membership_indicator<'a>(
        &'a self,
        membership_row: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
        membership_row
            .on_right_press(
                "open the Tailscale admin console",
                TailnetInteraction::OpenAdminConsole,
            )
            .with_hover_card(Box::new(move |card_clock| {
                self.tailscale
                    .sections_described_by(|snapshot| tailscale_hover_card(snapshot, card_clock))
            }))
    }

    fn membership_switch_indicator<'a>(
        &'a self,
        membership_row: GroupIndicator<'a>,
        tailscale: &Tailscale,
    ) -> GroupIndicator<'a> {
        let membership_indicator = self.membership_indicator(membership_row);
        match tailscale.switch() {
            TailscaleSwitch::Up => membership_indicator.on_switch_press(
                SwitchPosition::Off,
                "turn Tailscale on",
                TailnetInteraction::SwitchTailscale(TailscaleSwitch::Up),
            ),
            TailscaleSwitch::Down => membership_indicator.on_switch_press(
                SwitchPosition::On,
                "turn Tailscale off",
                TailnetInteraction::SwitchTailscale(TailscaleSwitch::Down),
            ),
        }
    }

    fn running_tailnet(&self) -> Option<&Tailnet> {
        match self.tailscale.get()? {
            Tailscale::Running(tailnet) => Some(tailnet),
            Tailscale::Stopped | Tailscale::Starting => None,
        }
    }

    fn policy_file_indicator(&self) -> GroupIndicator<'_> {
        PolicyFileIndicator::indicator()
            .with_hover_card(Box::new(move |_| self.policy_file_access.sections()))
            .fetching_on_hover(
                TailnetInteraction::OnBuses(TailnetBusRequest::StartPolicyFileFetch),
                TailnetInteraction::StopHoverFetches,
            )
    }

    fn inbound_access_indicator(&self) -> GroupIndicator<'_> {
        InboundAccessIndicator::indicator()
            .with_hover_card(Box::new(move |_| {
                self.inbound_access
                    .sections_described_by(tailscale_inbound_access_sections)
            }))
            .fetching_on_hover(
                TailnetInteraction::StartInboundAccessFetch,
                TailnetInteraction::StopHoverFetches,
            )
    }

    fn exit_offer_indicator(tailnet: &Tailnet) -> GroupIndicator<'_> {
        let indicator = ExitOfferIndicator { tailnet }.indicator();
        match tailnet.offer_control() {
            ExitOfferControl::Switchable(exit_offer_switch @ ExitOfferSwitch::StopOffering) => {
                indicator.on_switch_press(
                    SwitchPosition::On,
                    "stop offering this machine as an exit node",
                    TailnetInteraction::SwitchExitOffer(exit_offer_switch),
                )
            }
            ExitOfferControl::Switchable(exit_offer_switch @ ExitOfferSwitch::StartOffering) => {
                indicator.on_switch_press(
                    SwitchPosition::Off,
                    "offer this machine as an exit node",
                    TailnetInteraction::SwitchExitOffer(exit_offer_switch),
                )
            }
            ExitOfferControl::BlockedByExitNodeInUse => indicator,
        }
    }

    fn running_exit_node_choices(&self) -> Option<&ExitNodeChoices> {
        match self.tailscale_exit_nodes.get()? {
            TailscaleExitNodes::Running(exit_node_choices) => Some(exit_node_choices),
            TailscaleExitNodes::NotRunning => None,
        }
    }

    fn exit_node_choice_indicator<'a>(
        exit_node_option: &'a ExitNodeOption,
        exit_node_choices: &'a ExitNodeChoices,
    ) -> GroupIndicator<'a> {
        let exit_node_indicator = ExitNodeChoiceIndicator {
            exit_node_option,
            exit_node_choices,
        }
        .indicator();
        match exit_node_choices.control_for_option(exit_node_option) {
            ExitNodeControl::Switchable(exit_routing_switch) => exit_node_indicator
                .on_switch_press(
                    exit_node_option.node_use,
                    match exit_node_option.node_use {
                        ExitNodeUse::InUse => "stop using the exit node",
                        ExitNodeUse::Unused => "use the exit node",
                    },
                    TailnetInteraction::SwitchExitRouting(exit_routing_switch),
                ),
            ExitNodeControl::BlockedByOffering | ExitNodeControl::Offline => exit_node_indicator,
        }
    }

    fn exit_node_lan_access_indicator(exit_node_choices: &ExitNodeChoices) -> GroupIndicator<'_> {
        ExitNodeLanAccessIndicator { exit_node_choices }
            .indicator()
            .on_switch_press(
                exit_node_choices.lan_access,
                match exit_node_choices.lan_access {
                    ExitNodeLanAccess::Allowed => "cut local network access through exit nodes",
                    ExitNodeLanAccess::Blocked => "keep local network access through exit nodes",
                },
                TailnetInteraction::SwitchExitRouting(exit_node_choices.lan_access_switch()),
            )
    }

    fn exit_node_choice_indicators(&self) -> Vec<GroupIndicator<'_>> {
        let Some(exit_node_choices) = self.running_exit_node_choices() else {
            return Vec::new();
        };
        let mut exit_node_indicators: Vec<_> = exit_node_choices
            .options
            .iter()
            .map(|exit_node_option| {
                Self::exit_node_choice_indicator(exit_node_option, exit_node_choices)
                    .with_hover_card(Box::new(move |_| {
                        self.tailscale_exit_nodes.hover_card_with(
                            ExitNodeChoiceIndicator {
                                exit_node_option,
                                exit_node_choices,
                            }
                            .hover_card(),
                        )
                    }))
            })
            .collect();
        if !exit_node_choices.options.is_empty() {
            exit_node_indicators.push(
                Self::exit_node_lan_access_indicator(exit_node_choices).with_hover_card(Box::new(
                    move |_| {
                        self.tailscale_exit_nodes.hover_card_with(
                            ExitNodeLanAccessIndicator { exit_node_choices }.hover_card(),
                        )
                    },
                )),
            );
        }
        exit_node_indicators
    }

    pub fn group_content(&self, clock: Clock) -> IndicatorGroupContent<'_> {
        let mut rows = Vec::new();
        if !self.tailscale.awaits_first_delivery() {
            let membership_indicator = self.tailscale.indicator(
                |held_snapshot| tailscale_indicator(held_snapshot, clock),
                TAILNET_MEMBERSHIP_GLYPH.into(),
            );
            if membership_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(match self.tailscale.get() {
                    Some(held_snapshot) => {
                        self.membership_switch_indicator(membership_indicator, held_snapshot)
                    }
                    None => self.membership_indicator(membership_indicator),
                });
            }
        }
        if let Some(own_node_indicator) = self
            .tailscale
            .get()
            .and_then(OwnNodeIndicator::running_own_node)
        {
            rows.push(
                own_node_indicator
                    .indicator()
                    .with_hover_card(Box::new(move |_| {
                        self.tailscale
                            .hover_card_with(own_node_indicator.hover_card())
                    })),
            );
        }
        if let Some(tailnet) = self.running_tailnet() {
            rows.push(self.inbound_access_indicator());
            if matches!(self.policy_file_access, PolicyFileAccess::ApiToken { .. }) {
                rows.push(self.policy_file_indicator());
            }
            rows.push(
                Self::exit_offer_indicator(tailnet).with_hover_card(Box::new(move |_| {
                    self.tailscale
                        .hover_card_with(ExitOfferIndicator { tailnet }.hover_card())
                })),
            );
        }
        rows.extend(self.exit_node_choice_indicators());
        if let Some(failure_indicator) = self.tailscale_exit_nodes.failure() {
            rows.push(
                failure_indicator
                    .indicator(TAILSCALE_EXIT_NODES_GLYPH.into())
                    .with_hover_card(Box::new(move |_| {
                        self.tailscale_exit_nodes
                            .sections_described_by(|_| Vec::new())
                    })),
            );
        }
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Tailnet,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::Tailnet.title(),
                SimpleIcon::Tailscale.into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            TailnetInteraction::Expansion,
            rows.into_iter().map(|tailnet_indicator| {
                tailnet_indicator.refreshed_by(TailnetInteraction::Refresh)
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::network_connectivity::{
        tailscale_exit_nodes::ExitNodeReachability,
        tailscale_status::{ExitNodeOffer, KeyExpiry, Peer, PeerPath},
    };

    use super::*;
    use crate::{
        group_indicators::indicator_gestures::IndicatorGesture,
        indicator_groups::indicator_group_interaction::IndicatorGroupInteraction,
    };

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
    fn the_offer_indicator_is_inert_while_an_exit_node_is_in_use() {
        let tailnet = tailnet_offering(ExitNodeOffer::NotOffered, ExitNodeUse::InUse);
        assert!(matches!(
            TailnetGroup::exit_offer_indicator(&tailnet)
                .gestures
                .clone()
                .left,
            IndicatorGesture::Inert
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

    fn lists_policy_file_indicator(tailnet_group: &TailnetGroup) -> bool {
        tailnet_group
            .group_content(Clock::real())
            .indicator_keys()
            .iter()
            .any(|indicator_key| indicator_key.name == "Policy file")
    }

    #[test]
    fn the_policy_file_indicator_needs_an_api_token() {
        let without_token = group_running_tailnet(TailnetGroup::without_policy_file());
        assert!(!lists_policy_file_indicator(&without_token));
        let with_token =
            group_running_tailnet(TailnetGroup::with_policy_file(PathBuf::from("token")));
        assert!(lists_policy_file_indicator(&with_token));
    }

    #[test]
    fn an_approved_offer_can_be_stopped() {
        let tailnet = tailnet_offering(ExitNodeOffer::Approved, ExitNodeUse::Unused);
        assert!(matches!(
            TailnetGroup::exit_offer_indicator(&tailnet)
                .gestures
                .clone()
                .left,
            IndicatorGesture::Performs {
                message: IndicatorGroupInteraction::Tailnet(TailnetInteraction::SwitchExitOffer(
                    ExitOfferSwitch::StopOffering
                )),
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
    fn an_online_exit_node_indicator_can_be_pressed() {
        assert!(!matches!(
            TailnetGroup::exit_node_choice_indicator(
                &reachable_exit_node(ExitNodeReachability::Online),
                &exit_node_choices_while(ExitNodeOffer::NotOffered)
            )
            .gestures
            .clone()
            .left,
            IndicatorGesture::Inert
        ));
    }

    #[test]
    fn an_exit_node_indicator_is_inert_while_this_machine_advertises() {
        assert!(matches!(
            TailnetGroup::exit_node_choice_indicator(
                &reachable_exit_node(ExitNodeReachability::Online),
                &exit_node_choices_while(ExitNodeOffer::Approved)
            )
            .gestures
            .clone()
            .left,
            IndicatorGesture::Inert
        ));
    }

    #[test]
    fn an_offline_exit_node_indicator_is_inert() {
        assert!(matches!(
            TailnetGroup::exit_node_choice_indicator(
                &reachable_exit_node(ExitNodeReachability::OfflineNeverSeen),
                &exit_node_choices_while(ExitNodeOffer::NotOffered)
            )
            .gestures
            .clone()
            .left,
            IndicatorGesture::Inert
        ));
    }
}
