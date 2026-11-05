use std::borrow::Cow;

use chrono::Locale;
use sottomano_config::severity::Severity;
use sottomano_sources::network_connectivity::tailscale_exit_nodes::{
    ExitNodeChoices, ExitNodeControl, ExitNodeLanAccess, ExitNodeOption, ExitNodeReachability,
    TailscaleExitNodes,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

const OFFERING_BLOCKS_USE_TEXT: &str =
    "this machine offers itself as an exit node, see the Tailscale group";

fn lan_access_entry(choices: &ExitNodeChoices) -> LabeledValue {
    LabeledValue::new(
        "Local network",
        match choices.lan_access {
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
        SimpleIcon::Tailscale.into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
    }
}

pub(super) struct ExitNodeRow<'a> {
    pub(super) option: &'a ExitNodeOption,
    pub(super) choices: &'a ExitNodeChoices,
}

impl GroupItemReading for ExitNodeRow<'_> {
    fn name(&self) -> &'static str {
        "Tailscale exit node"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        format!("{} exit node", self.option.name).into()
    }

    fn icon(&self) -> IconGlyph {
        SimpleIcon::Tailscale.into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::of_toggle(self.option.in_use)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        match self.choices.control_for_option(self.option) {
            ExitNodeControl::Switchable(_) => None,
            ExitNodeControl::BlockedByOffering => Some("blocked".to_owned()),
            ExitNodeControl::Offline => Some("offline".to_owned()),
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let reachability_entry = match self.option.reachability {
            ExitNodeReachability::Online => LabeledValue::new("State", "online"),
            ExitNodeReachability::OfflineSince(last_seen) => {
                LabeledValue::new("Last seen", ValueCell::RelativeMoment(last_seen))
            }
            ExitNodeReachability::OfflineNeverSeen => LabeledValue::new("State", "offline"),
        };
        let blocked_entry = matches!(
            self.choices.control_for_option(self.option),
            ExitNodeControl::BlockedByOffering
        )
        .then(|| LabeledValue::new("Blocked", OFFERING_BLOCKS_USE_TEXT).level(Severity::Warning));
        HoverCardBody::dictionary(
            [
                LabeledValue::new("Exit node", self.option.name.as_str()),
                LabeledValue::new("IP", self.option.ip.as_str()),
                reachability_entry,
                LabeledValue::new("In use", if self.option.in_use { "yes" } else { "no" }),
                lan_access_entry(self.choices),
            ]
            .into_iter()
            .chain(blocked_entry),
        )
    }
}

pub(super) struct ExitNodeLanAccessRow<'a> {
    pub(super) choices: &'a ExitNodeChoices,
}

impl GroupItemReading for ExitNodeLanAccessRow<'_> {
    fn name(&self) -> &'static str {
        "Exit node LAN access"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Network(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::of_toggle(self.choices.lan_access == ExitNodeLanAccess::Allowed)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([lan_access_entry(self.choices)])
    }
}
