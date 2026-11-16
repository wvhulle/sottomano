use sottomano_desktop_sources::network_connectivity::{
    modem_manager_mobile::{Mobile, ModemState},
    network_manager_proxies::{LinkPhase, RadioPower},
};
use sottomano_toml_config::severity::Severity;

use crate::{
    group_indicators::{
        gauge::percent_fill,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        reading_row::ReadingRow,
        toggle_control::SwitchPosition,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widget_groups::interface_address_sections::{AddressFamily, signal_level},
    widgets::{
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub struct ModemSignalIndicator<'a> {
    pub modem: &'a Mobile,
}

pub struct ModemConnectionSwitch<'a> {
    pub modem: &'a Mobile,
}

pub struct ModemDataIndicator<'a> {
    pub modem: &'a Mobile,
}

pub struct AllMobileRadiosSwitch {
    pub radio_power: RadioPower,
}

#[must_use]
pub fn modem_family_address(modem: &Mobile, family: AddressFamily) -> Option<&str> {
    match family {
        AddressFamily::Ipv4 => modem.ipv4.as_deref(),
        AddressFamily::Ipv6 => modem.ipv6.as_deref(),
    }
}

impl<'a> ModemSignalIndicator<'a> {
    pub fn reading_row(self) -> ReadingRow<'a> {
        let state = match self.modem.link_state() {
            LinkPhase::Switching => IndicatorState::Pending,
            LinkPhase::Up => self
                .modem
                .signal
                .map_or(Severity::Normal, signal_level)
                .into(),
            LinkPhase::Down => IndicatorState::Off,
        };
        let gauge = match self.modem.signal {
            Some(signal) => {
                let fill = percent_fill(signal);
                ReadoutGauge::Gauged { fill, live: fill }
            }
            None => ReadoutGauge::Ungauged,
        };
        ReadingRow {
            readout: IndicatorReadout {
                label: ReadoutLabel::Unlabeled,
                gauge,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                "Signal",
                Phosphor::Broadcast(Weight::Regular).into(),
                state,
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    pub fn hover_card(&self) -> Vec<Section> {
        let signal_suffix = if self.modem.signal_recent {
            ""
        } else {
            " (cached)"
        };
        vec![
            Section::new(
                "Network",
                dictionary_table(
                    [
                        self.modem
                            .operator
                            .as_deref()
                            .map(|operator| LabeledValue::new("Carrier", operator)),
                        (!self.modem.technology.is_empty()).then(|| {
                            LabeledValue::new("Network Type", self.modem.technology.join(", "))
                        }),
                        self.modem.signal.map(|signal| {
                            LabeledValue::new(
                                "Signal Strength",
                                format!("{signal}%{signal_suffix}"),
                            )
                        }),
                        self.modem.registration.map(|registration| {
                            LabeledValue::new("Network Registration", registration)
                        }),
                    ]
                    .into_iter()
                    .flatten(),
                ),
            ),
            Section::from(self.modem),
        ]
    }
}

impl<'a> ModemConnectionSwitch<'a> {
    #[must_use]
    pub fn press_does(&self) -> &'static str {
        if self.modem.link_state() == LinkPhase::Up {
            "disconnect this modem"
        } else {
            "connect this modem"
        }
    }

    pub fn reading_row(self) -> ReadingRow<'a> {
        let glyph = if self.modem.link_state() == LinkPhase::Up {
            Phosphor::ToggleRight(Weight::Regular).into()
        } else {
            Phosphor::ToggleLeft(Weight::Regular).into()
        };
        ReadingRow::glyph_only(
            "Connection",
            glyph,
            IndicatorState::from(self.modem.link_state()),
            Box::new(move |_| self.hover_card()),
        )
    }

    pub fn hover_card(&self) -> Vec<Section> {
        let failure = match self.modem.state {
            Some(ModemState::Failed {
                reason: Some(reason),
            }) => Some(LabeledValue::new("Failure Reason", reason)),
            _ => None,
        };
        vec![Section::new(
            "Status",
            dictionary_table(
                self.modem
                    .state
                    .map(|state| LabeledValue::new("Connection State", state.label()))
                    .into_iter()
                    .chain(failure),
            ),
        )]
    }
}

impl<'a> ModemDataIndicator<'a> {
    pub fn reading_row(self) -> ReadingRow<'a> {
        ReadingRow::glyph_only(
            "Mobile data",
            Phosphor::ArrowsDownUp(Weight::Regular).into(),
            IndicatorState::Idle,
            Box::new(move |_| self.hover_card()),
        )
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Transferred",
            dictionary_table(
                [
                    self.modem.rx.map(|received| {
                        LabeledValue::new("Received (RX)", Quantity::size(received))
                    }),
                    self.modem
                        .tx
                        .map(|sent| LabeledValue::new("Sent (TX)", Quantity::size(sent))),
                ]
                .into_iter()
                .flatten(),
            ),
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

    pub fn reading_row(self) -> ReadingRow<'static> {
        let glyph = match self.radio_power {
            RadioPower::On => Phosphor::CellSignalFull(Weight::Regular).into(),
            RadioPower::Off => Phosphor::CellSignalSlash(Weight::Regular).into(),
        };
        ReadingRow::glyph_only(
            "All mobile radios",
            glyph,
            IndicatorState::from(SwitchPosition::from(self.radio_power)),
            Box::new(move |_| self.hover_card()),
        )
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
