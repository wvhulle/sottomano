use sottomano_desktop_sources::network_connectivity::tailscale_status::{
    ExitNodeOffer, ExitOfferControl, Tailnet,
};
use sottomano_toml_config::severity::Severity;

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};

const USE_BLOCKS_OFFERING_TEXT: &str = "stop using the exit node first";

pub(super) fn exit_offer_text(exit_offer: ExitNodeOffer) -> &'static str {
    match exit_offer {
        ExitNodeOffer::NotOffered => "not offered",
        ExitNodeOffer::AwaitingApproval => "offered, awaiting approval",
        ExitNodeOffer::Approved => "offered and approved",
    }
}

pub(super) struct ExitOfferIndicator<'view> {
    pub(super) tailnet: &'view Tailnet,
}

impl<'view> ExitOfferIndicator<'view> {
    pub(super) fn indicator(self) -> GroupIndicator<'view> {
        let label = match self.tailnet.offer_control() {
            ExitOfferControl::Switchable(_) => ReadoutLabel::Unlabeled,
            ExitOfferControl::BlockedByExitNodeInUse => ReadoutLabel::Labeled("blocked".to_owned()),
        };
        GroupIndicator::glyph_only(
            "Offer as exit node",
            Phosphor::Broadcast(Weight::Regular).into(),
            IndicatorState::from(self.tailnet.exit_offer),
        )
        .with_readout(IndicatorReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        let blocked_entry = matches!(
            self.tailnet.offer_control(),
            ExitOfferControl::BlockedByExitNodeInUse
        )
        .then(|| LabeledValue::new("Blocked", USE_BLOCKS_OFFERING_TEXT).level(Severity::Warning));
        vec![Section::new(
            "Status",
            dictionary_table(
                [LabeledValue::new(
                    "Exit node",
                    exit_offer_text(self.tailnet.exit_offer),
                )]
                .into_iter()
                .chain(blocked_entry),
            ),
        )]
    }
}
