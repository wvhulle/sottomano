use sottomano_desktop_sources::network_connectivity::{
    tailscale_exit_nodes::{
        ExitNodeChoices, ExitNodeControl, ExitNodeLanAccess, ExitNodeOption, ExitNodeReachability,
        TailscaleExitNodes,
    },
    tailscale_status::ExitNodeUse,
};
use sottomano_toml_config::severity::Severity;

use super::{TailnetGroup, TailnetInteraction};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::{LabeledValue, ValueCell},
};

const OFFERING_BLOCKS_USE_TEXT: &str = "stop offering this machine as an exit node first";

fn lan_access_entry(exit_node_choices: &ExitNodeChoices) -> LabeledValue {
    LabeledValue::new(
        "Local network",
        match exit_node_choices.lan_access {
            ExitNodeLanAccess::Allowed => "stays reachable through an exit node",
            ExitNodeLanAccess::Blocked => "unreachable through an exit node",
        },
    )
}

pub(super) struct ExitNodeChoiceIndicator<'view> {
    pub(super) exit_node_option: &'view ExitNodeOption,
    pub(super) exit_node_choices: &'view ExitNodeChoices,
}

impl<'view> ExitNodeChoiceIndicator<'view> {
    pub(super) fn indicator(self) -> GroupIndicator<'view> {
        let label = match self
            .exit_node_choices
            .control_for_option(self.exit_node_option)
        {
            ExitNodeControl::Switchable(_) => ReadoutLabel::Unlabeled,
            ExitNodeControl::BlockedByOffering => ReadoutLabel::Labeled("blocked".to_owned()),
            ExitNodeControl::Offline => ReadoutLabel::Labeled("offline".to_owned()),
        };
        GroupIndicator::glyph_only(
            format!("Exit via {}", self.exit_node_option.name),
            Phosphor::SignOut(Weight::Regular).into(),
            IndicatorState::from(SwitchPosition::from(self.exit_node_option.node_use)),
        )
        .with_readout(IndicatorReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        let reachability_entry = match self.exit_node_option.reachability {
            ExitNodeReachability::Online => LabeledValue::new("State", "online"),
            ExitNodeReachability::OfflineSince(last_seen) => {
                LabeledValue::new("Last seen", ValueCell::RelativeMoment(last_seen))
            }
            ExitNodeReachability::OfflineNeverSeen => LabeledValue::new("State", "offline"),
        };
        let blocked_entry = matches!(
            self.exit_node_choices
                .control_for_option(self.exit_node_option),
            ExitNodeControl::BlockedByOffering
        )
        .then(|| LabeledValue::new("Blocked", OFFERING_BLOCKS_USE_TEXT).level(Severity::Warning));
        vec![Section::new(
            "Status",
            dictionary_table(
                [
                    LabeledValue::new("Exit node", self.exit_node_option.name.as_str()),
                    LabeledValue::new("IP", self.exit_node_option.ip.as_str()),
                    reachability_entry,
                    LabeledValue::new(
                        "In use",
                        match self.exit_node_option.node_use {
                            ExitNodeUse::InUse => "yes",
                            ExitNodeUse::Unused => "no",
                        },
                    ),
                    lan_access_entry(self.exit_node_choices),
                ]
                .into_iter()
                .chain(blocked_entry),
            ),
        )]
    }
}

pub(super) struct ExitNodeLanAccessIndicator<'view> {
    pub(super) exit_node_choices: &'view ExitNodeChoices,
}

impl<'view> ExitNodeLanAccessIndicator<'view> {
    pub(super) fn indicator(self) -> GroupIndicator<'view> {
        GroupIndicator::glyph_only(
            "LAN while exiting",
            Phosphor::HouseLine(Weight::Regular).into(),
            IndicatorState::from(SwitchPosition::from(self.exit_node_choices.lan_access)),
        )
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([lan_access_entry(self.exit_node_choices)]),
        )]
    }
}

impl TailnetGroup {
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
