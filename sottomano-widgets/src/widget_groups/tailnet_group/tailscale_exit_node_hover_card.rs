use std::borrow::Cow;

use chrono::Locale;
use sottomano_desktop_sources::network_connectivity::tailscale_exit_nodes::{
    ExitNodeChoices, ExitNodeControl, ExitNodeLanAccess, ExitNodeOption, ExitNodeReachability,
    TailscaleExitNodes,
};
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
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

impl GroupItemReading for TailscaleExitNodes {
    fn name(&self) -> &'static str {
        "Tailscale exit nodes"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::SignOut(Weight::Regular).into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
    }
}

pub(super) struct ExitNodeChoiceRow<'a> {
    pub(super) exit_node_option: &'a ExitNodeOption,
    pub(super) exit_node_choices: &'a ExitNodeChoices,
}

impl GroupItemReading for ExitNodeChoiceRow<'_> {
    fn name(&self) -> &'static str {
        "Tailscale exit node"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        format!("Exit via {}", self.exit_node_option.name).into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::SignOut(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::of_toggle(self.exit_node_option.in_use)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        match self
            .exit_node_choices
            .control_for_option(self.exit_node_option)
        {
            ExitNodeControl::Switchable(_) => None,
            ExitNodeControl::BlockedByOffering => Some("blocked".to_owned()),
            ExitNodeControl::Offline => Some("offline".to_owned()),
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
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
        HoverCardBody::dictionary(
            [
                LabeledValue::new("Exit node", self.exit_node_option.name.as_str()),
                LabeledValue::new("IP", self.exit_node_option.ip.as_str()),
                reachability_entry,
                LabeledValue::new(
                    "In use",
                    if self.exit_node_option.in_use {
                        "yes"
                    } else {
                        "no"
                    },
                ),
                lan_access_entry(self.exit_node_choices),
            ]
            .into_iter()
            .chain(blocked_entry),
        )
    }
}

pub(super) struct ExitNodeLanAccessRow<'a> {
    pub(super) exit_node_choices: &'a ExitNodeChoices,
}

impl GroupItemReading for ExitNodeLanAccessRow<'_> {
    fn name(&self) -> &'static str {
        "LAN while exiting"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::HouseLine(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::of_toggle(
            self.exit_node_choices.lan_access == ExitNodeLanAccess::Allowed,
        )
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([lan_access_entry(self.exit_node_choices)])
    }
}
