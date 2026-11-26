use std::path::PathBuf;

use iced::{Subscription, Task};
use sottomano_desktop_sinks::network_connectivity::tailscale_cli_switches::{
    open_admin, switch_exit_offer, switch_exit_routing, switch_tailscale,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        tailscale_exit_nodes::{ExitRoutingSwitch, TailscaleExitNodes},
        tailscale_inbound_access::TailscaleInboundAccess,
        tailscale_policy_file::TailscalePolicyFile,
        tailscale_status::{ExitOfferSwitch, Tailscale, TailscaleSwitch},
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
        group_indicator::GroupIndicator,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
            IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
        indicator_state::IndicatorState,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    panel_menus::network_menu::tailnet_group::tailnet_membership_indicator::{
        TAILNET_MEMBERSHIP_GLYPH, tailscale_indicator,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, subscribed_snapshot::SubscribedSnapshot,
    },
};

const TAILSCALE_EXIT_NODES_GLYPH: Phosphor = Phosphor::SignOut(Weight::Regular);

mod exit_node_choice_indicator;
mod exit_offer_indicator;
mod inbound_access_indicator;
mod own_node_indicator;
mod policy_file_access;
mod policy_file_indicator;
mod tailnet_membership_indicator;
#[cfg(test)]
mod tailnet_switch_tests;

use self::policy_file_access::PolicyFileAccess;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TailnetBusRequest {
    StartPolicyFileFetch,
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

    fn running_tailnet_indicators<'view>(
        &'view self,
        tailscale: &'view Tailscale,
    ) -> Vec<GroupIndicator<'view>> {
        let Tailscale::Running(tailnet) = tailscale else {
            return Vec::new();
        };
        let policy_file_indicators = match self.policy_file_access {
            PolicyFileAccess::ApiToken { .. } => vec![self.policy_file_row()],
            PolicyFileAccess::NoApiToken => Vec::new(),
        };
        [self.inbound_access_row()]
            .into_iter()
            .chain(policy_file_indicators)
            .chain([self.exit_offer_row(tailnet)])
            .collect()
    }

    pub fn group_content(&self, clock: Clock) -> IndicatorGroupContent<'_> {
        let tailscale_listing = self.tailscale.indicator_listing(
            |held_snapshot| {
                let membership_indicator = tailscale_indicator(held_snapshot, clock);
                let membership_indicators = self
                    .membership_switch_indicator(membership_indicator, held_snapshot)
                    .when_shown();
                let own_node_indicators = self.own_node_rows(held_snapshot);
                membership_indicators
                    .into_iter()
                    .chain(own_node_indicators)
                    .chain(self.running_tailnet_indicators(held_snapshot))
                    .collect()
            },
            TAILNET_MEMBERSHIP_GLYPH.into(),
        );
        let exit_node_indicators = self.exit_node_choice_indicators();
        let exit_node_listing = match self.tailscale_exit_nodes.failure() {
            Some(failure_indicator) => IndicatorListing::Incomplete {
                listed_indicators: exit_node_indicators,
                unread_sources: vec![
                    failure_indicator
                        .indicator(TAILSCALE_EXIT_NODES_GLYPH.into())
                        .with_hover_card(Box::new(move |_| {
                            self.tailscale_exit_nodes
                                .sections_described_by(|_| Vec::new())
                        })),
                ],
            },
            None => IndicatorListing::Listed(exit_node_indicators),
        };
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Tailnet,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::Tailnet.title(),
                SimpleIcon::Tailscale.into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            TailnetInteraction::Expansion,
            [tailscale_listing, exit_node_listing]
                .into_iter()
                .collect::<IndicatorListing<'_>>()
                .map_indicators(|tailnet_indicator| {
                    tailnet_indicator.refreshed_by(TailnetInteraction::Refresh)
                }),
        )
    }
}
