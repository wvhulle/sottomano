use sottomano_desktop_sources::network_connectivity::{
    modem_manager_mobile::{
        BearerAddress, BearerTraffic, Mobile, MobileOperator, ModemFailure, ModemState,
        NetworkRegistration, SignalFreshness, SignalQuality,
    },
    network_manager_proxies::{LinkPhase, RadioPower},
};
use sottomano_toml_config::severity::Severity;

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        gauge::percent_fill,
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::interface_address_sections::{AddressFamily, signal_level},
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

pub struct ModemSignalIndicator<'view> {
    pub modem: &'view Mobile,
}

pub struct ModemConnectionSwitch<'view> {
    pub modem: &'view Mobile,
}

pub struct ModemDataIndicator<'view> {
    pub modem: &'view Mobile,
}

pub struct AllMobileRadiosSwitch {
    pub radio_power: RadioPower,
}

#[must_use]
pub fn modem_family_address(modem: &Mobile, family: AddressFamily) -> &BearerAddress {
    match family {
        AddressFamily::Ipv4 => &modem.ipv4,
        AddressFamily::Ipv6 => &modem.ipv6,
    }
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

impl<'view> ModemConnectionSwitch<'view> {
    #[must_use]
    pub fn press_does(&self) -> &'static str {
        if self.modem.link_state() == LinkPhase::Up {
            "disconnect this modem"
        } else {
            "connect this modem"
        }
    }

    pub fn indicator(self) -> GroupIndicator<'view> {
        let glyph = if self.modem.link_state() == LinkPhase::Up {
            Phosphor::ToggleRight(Weight::Regular).into()
        } else {
            Phosphor::ToggleLeft(Weight::Regular).into()
        };
        GroupIndicator::glyph_only(
            "Connection",
            glyph,
            IndicatorState::from(self.modem.link_state()),
        )
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        let mut status_entries = Vec::new();
        if self.modem.state != ModemState::Unreported {
            status_entries.push(LabeledValue::new(
                "Connection State",
                self.modem.state.label(),
            ));
        }
        if let ModemState::Failed { reason } = self.modem.state
            && reason != ModemFailure::Unspecified
        {
            status_entries.push(LabeledValue::new("Failure Reason", reason.label()));
        }
        vec![Section::new("Status", dictionary_table(status_entries))]
    }
}

impl<'view> ModemDataIndicator<'view> {
    pub fn indicator(self) -> GroupIndicator<'view> {
        GroupIndicator::glyph_only(
            "Mobile data",
            Phosphor::ArrowsDownUp(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        let BearerTraffic::Counted { received, sent } = self.modem.traffic else {
            return vec![Section::new("Transferred", dictionary_table(Vec::new()))];
        };
        vec![Section::new(
            "Transferred",
            dictionary_table([
                LabeledValue::new("Received (RX)", Quantity::size(received)),
                LabeledValue::new("Sent (TX)", Quantity::size(sent)),
            ]),
        )]
    }
}

impl AllMobileRadiosSwitch {
    #[must_use]
    pub fn press_does(&self) -> &'static str {
        match self.radio_power {
            RadioPower::On => "turn all mobile radios off",
            RadioPower::Off => "turn all mobile radios on",
        }
    }

    pub fn indicator<'indicator>(self) -> GroupIndicator<'indicator> {
        let glyph = match self.radio_power {
            RadioPower::On => Phosphor::CellSignalFull(Weight::Regular).into(),
            RadioPower::Off => Phosphor::CellSignalSlash(Weight::Regular).into(),
        };
        GroupIndicator::glyph_only(
            "All mobile radios",
            glyph,
            IndicatorState::from(SwitchPosition::from(self.radio_power)),
        )
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([LabeledValue::new(
                "All mobile radios",
                self.radio_power.label(),
            )]),
        )]
    }
}
