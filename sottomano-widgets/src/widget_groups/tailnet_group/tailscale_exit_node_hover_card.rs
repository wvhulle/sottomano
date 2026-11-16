use sottomano_desktop_sources::network_connectivity::{
    tailscale_exit_nodes::{
        ExitNodeChoices, ExitNodeControl, ExitNodeLanAccess, ExitNodeOption, ExitNodeReachability,
    },
    tailscale_status::ExitNodeUse,
};
use sottomano_toml_config::severity::Severity;

use crate::{
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
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

pub(super) struct ExitNodeChoiceRow<'a> {
    pub(super) exit_node_option: &'a ExitNodeOption,
    pub(super) exit_node_choices: &'a ExitNodeChoices,
}

impl<'a> ExitNodeChoiceRow<'a> {
    pub(super) fn indicator(self) -> GroupIndicator<'a> {
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

pub(super) struct ExitNodeLanAccessRow<'a> {
    pub(super) exit_node_choices: &'a ExitNodeChoices,
}

impl<'a> ExitNodeLanAccessRow<'a> {
    pub(super) fn indicator(self) -> GroupIndicator<'a> {
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
