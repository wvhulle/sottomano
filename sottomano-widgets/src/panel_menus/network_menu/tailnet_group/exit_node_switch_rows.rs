use sottomano_desktop_sources::network_connectivity::{
    tailscale_exit_nodes::{
        ExitNodeChoices, ExitNodeControl, ExitNodeLanAccess, ExitNodeOption, TailscaleExitNodes,
    },
    tailscale_status::{ExitNodeUse, ExitOfferControl, ExitOfferSwitch, Tailnet},
};

use super::{
    TailnetGroup, TailnetInteraction,
    tailscale_exit_node_hover_card::{ExitNodeChoiceIndicator, ExitNodeLanAccessIndicator},
    tailscale_exit_offer_hover_card::ExitOfferIndicator,
};
use crate::group_indicators::{group_indicator::GroupIndicator, toggle_control::SwitchPosition};

impl TailnetGroup {
    pub(super) fn exit_offer_indicator(tailnet: &Tailnet) -> GroupIndicator<'_> {
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

    pub(super) fn exit_node_choice_indicator<'view>(
        exit_node_option: &'view ExitNodeOption,
        exit_node_choices: &'view ExitNodeChoices,
    ) -> GroupIndicator<'view> {
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

    pub(super) fn exit_node_choice_indicators(&self) -> Vec<GroupIndicator<'_>> {
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
}
