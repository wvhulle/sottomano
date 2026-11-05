use sottomano_desktop_sources::network_connectivity::tailscale_status::{
    ExitNodeOffer, ExitOfferControl, Tailnet,
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
        value_cells::LabeledValue,
    },
};

const USE_BLOCKS_OFFERING_TEXT: &str = "this machine is using an exit node, see the VPN group";

pub(super) fn exit_offer_text(exit_offer: ExitNodeOffer) -> &'static str {
    match exit_offer {
        ExitNodeOffer::NotOffered => "not offered",
        ExitNodeOffer::AwaitingApproval => "offered, awaiting approval",
        ExitNodeOffer::Approved => "offered and approved",
    }
}

pub(super) struct ExitOfferRow<'a> {
    pub(super) tailnet: &'a Tailnet,
}

impl GroupItemReading for ExitOfferRow<'_> {
    fn name(&self) -> &'static str {
        "Offer as exit node"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Broadcast(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::of_toggle(self.tailnet.exit_offer.is_offered())
    }

    fn label(&self, _clock: Clock, _locale: chrono::Locale) -> Option<String> {
        match self.tailnet.offer_control() {
            ExitOfferControl::Switchable(_) => None,
            ExitOfferControl::BlockedByExitNodeInUse => Some("blocked".to_owned()),
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let blocked_entry = matches!(
            self.tailnet.offer_control(),
            ExitOfferControl::BlockedByExitNodeInUse
        )
        .then(|| LabeledValue::new("Blocked", USE_BLOCKS_OFFERING_TEXT).level(Severity::Warning));
        HoverCardBody::dictionary(
            [LabeledValue::new(
                "Exit node",
                exit_offer_text(self.tailnet.exit_offer),
            )]
            .into_iter()
            .chain(blocked_entry),
        )
    }
}
