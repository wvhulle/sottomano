use sottomano_desktop_sources::network_connectivity::{
    modem_manager_mobile::{
        Mobile, MobileOperator, NetworkRegistration, SignalFreshness, SignalQuality,
    },
    network_manager_proxies::LinkPhase,
};
use sottomano_toml_config::severity::Severity;

use super::MobileModemsGroup;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        gauge::percent_fill,
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    panel_menus::network_menu::interface_address_sections::signal_level,
    widgets::value_cells::LabeledValue,
};
pub struct ModemSignalIndicator<'view> {
    pub modem: &'view Mobile,
}

impl<'view> ModemSignalIndicator<'view> {
    pub fn indicator(self) -> GroupIndicator<'view> {
        let (signal_severity, gauge) = match self.modem.signal {
            SignalQuality::Measured { percent, .. } => {
                let fill = percent_fill(percent);
                (
                    signal_level(percent),
                    ReadoutGauge::Gauged { fill, live: fill },
                )
            }
            SignalQuality::Unmeasured => (Severity::Normal, ReadoutGauge::Ungauged),
        };
        let state = match self.modem.link_state() {
            LinkPhase::Switching => IndicatorState::Pending,
            LinkPhase::Up => signal_severity.into(),
            LinkPhase::Down => IndicatorState::Off,
        };
        GroupIndicator::glyph_only("Signal", Phosphor::Broadcast(Weight::Regular).into(), state)
            .with_readout(IndicatorReadout {
                label: ReadoutLabel::Unlabeled,
                gauge,
                badge: ReadoutBadge::Unbadged,
            })
            .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        let mut network_entries = Vec::new();
        if let MobileOperator::Named(operator) = &self.modem.operator {
            network_entries.push(LabeledValue::new("Carrier", operator.as_str()));
        }
        if !self.modem.technology.is_empty() {
            network_entries.push(LabeledValue::new(
                "Network Type",
                self.modem.technology.join(", "),
            ));
        }
        if let SignalQuality::Measured { percent, freshness } = self.modem.signal {
            let signal_suffix = match freshness {
                SignalFreshness::Recent => "",
                SignalFreshness::Cached => " (cached)",
            };
            network_entries.push(LabeledValue::new(
                "Signal Strength",
                format!("{percent}%{signal_suffix}"),
            ));
        }
        if self.modem.registration != NetworkRegistration::Unreported {
            network_entries.push(LabeledValue::new(
                "Network Registration",
                self.modem.registration.label(),
            ));
        }
        vec![
            Section::new("Network", dictionary_table(network_entries)),
            Section::from(self.modem),
        ]
    }
}

impl MobileModemsGroup {
    pub(super) fn modem_signal_row<'view>(
        &'view self,
        modem: &'view Mobile,
    ) -> GroupIndicator<'view> {
        ModemSignalIndicator { modem }
            .indicator()
            .with_hover_card(Box::new(move |_| {
                self.mobile_modems
                    .hover_card_with(ModemSignalIndicator { modem }.hover_card())
            }))
    }
}
