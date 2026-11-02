use sottomano_config::severity::Severity;
use sottomano_sources::network_connectivity::modem_manager_mobile::{Mobile, ModemState};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widget_groups::interface_address_sections::{AddressFamily, signal_level},
    widgets::{
        gauge::percent_fill,
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        reading_display_state::ReadingDisplayState,
        tooltip_body::{Section, TooltipBody},
        value_cells::LabeledValue,
    },
};

pub struct ModemSignalReading<'a> {
    pub modem: &'a Mobile,
}

pub struct ModemConnectionSwitch<'a> {
    pub modem: &'a Mobile,
}

pub struct ModemDataReading<'a> {
    pub modem: &'a Mobile,
}

pub struct AllMobileRadiosSwitch {
    pub radio_on: bool,
}

#[must_use]
pub fn modem_radio_on(modem: &Mobile) -> bool {
    modem
        .state
        .is_some_and(|state| !matches!(state, ModemState::Disabled | ModemState::Disabling))
}

#[must_use]
pub fn modem_connected(modem: &Mobile) -> bool {
    modem.state == Some(ModemState::Connected)
}

#[must_use]
pub fn modem_family_address(modem: &Mobile, family: AddressFamily) -> Option<&str> {
    match family {
        AddressFamily::Ipv4 => modem.ipv4.as_deref(),
        AddressFamily::Ipv6 => modem.ipv6.as_deref(),
    }
}

fn modem_switching(modem: &Mobile) -> bool {
    matches!(
        modem.state,
        Some(
            ModemState::Enabling
                | ModemState::Disabling
                | ModemState::Connecting
                | ModemState::Disconnecting,
        )
    )
}

impl GroupItemReading for ModemSignalReading<'_> {
    fn name(&self) -> &'static str {
        "Signal"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Broadcast(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if modem_switching(self.modem) {
            return ReadingDisplayState::Pending;
        }
        if modem_connected(self.modem) {
            return self
                .modem
                .signal
                .map_or(Severity::Normal, signal_level)
                .into();
        }
        ReadingDisplayState::Off
    }

    fn gauge(&self) -> Option<Ratio> {
        self.modem.signal.map(percent_fill)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let signal_suffix = if self.modem.signal_recent {
            ""
        } else {
            " (cached)"
        };
        TooltipBody::sections([
            Section::dictionary(
                [
                    self.modem
                        .operator
                        .as_deref()
                        .map(|operator| LabeledValue::new("Carrier", operator)),
                    (!self.modem.technology.is_empty()).then(|| {
                        LabeledValue::new("Network Type", self.modem.technology.join(", "))
                    }),
                    self.modem.signal.map(|signal| {
                        LabeledValue::new("Signal Strength", format!("{signal}%{signal_suffix}"))
                    }),
                    self.modem.registration.map(|registration| {
                        LabeledValue::new("Network Registration", registration)
                    }),
                ]
                .into_iter()
                .flatten(),
            ),
            Section::from(&self.modem.identity),
        ])
    }
}

impl ModemConnectionSwitch<'_> {
    #[must_use]
    pub fn press_does(&self) -> &'static str {
        if modem_connected(self.modem) {
            "disconnect this modem"
        } else {
            "connect this modem"
        }
    }
}

impl GroupItemReading for ModemConnectionSwitch<'_> {
    fn name(&self) -> &'static str {
        "Connection"
    }

    fn icon(&self) -> IconGlyph {
        if modem_connected(self.modem) {
            Phosphor::ToggleRight(Weight::Regular).into()
        } else {
            Phosphor::ToggleLeft(Weight::Regular).into()
        }
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if modem_switching(self.modem) {
            return ReadingDisplayState::Pending;
        }
        ReadingDisplayState::of_toggle(modem_connected(self.modem))
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let failure = match self.modem.state {
            Some(ModemState::Failed {
                reason: Some(reason),
            }) => Some(LabeledValue::new("Failure Reason", reason)),
            _ => None,
        };
        TooltipBody::dictionary(
            self.modem
                .state
                .map(|state| LabeledValue::new("Connection State", state.label()))
                .into_iter()
                .chain(failure),
        )
    }
}

impl GroupItemReading for ModemDataReading<'_> {
    fn name(&self) -> &'static str {
        "Mobile data"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::ArrowsDownUp(Weight::Regular).into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary(
            [
                self.modem
                    .rx
                    .map(|received| LabeledValue::new("Received (RX)", Quantity::size(received))),
                self.modem
                    .tx
                    .map(|sent| LabeledValue::new("Sent (TX)", Quantity::size(sent))),
            ]
            .into_iter()
            .flatten(),
        )
    }
}

impl AllMobileRadiosSwitch {
    #[must_use]
    pub fn press_does(&self) -> &'static str {
        if self.radio_on {
            "turn all mobile radios off"
        } else {
            "turn all mobile radios on"
        }
    }
}

impl GroupItemReading for AllMobileRadiosSwitch {
    fn name(&self) -> &'static str {
        "All mobile radios"
    }

    fn icon(&self) -> IconGlyph {
        if self.radio_on {
            Phosphor::CellSignalFull(Weight::Regular).into()
        } else {
            Phosphor::CellSignalSlash(Weight::Regular).into()
        }
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::of_toggle(self.radio_on)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let radio_state = if self.radio_on { "on" } else { "off" };
        TooltipBody::dictionary([LabeledValue::new("All mobile radios", radio_state)])
    }
}
