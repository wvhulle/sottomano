use sottomano_desktop_sources::network_connectivity::tailscale_status::{
    ExitNodeOffer, ExitOfferControl, ExitOfferSwitch, Tailnet,
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

    pub(super) fn exit_offer_row<'view>(
        &'view self,
        tailnet: &'view Tailnet,
    ) -> GroupIndicator<'view> {
        Self::exit_offer_indicator(tailnet).with_hover_card(Box::new(move |_| {
            self.tailscale
                .hover_card_with(ExitOfferIndicator { tailnet }.hover_card())
        }))
    }
}
