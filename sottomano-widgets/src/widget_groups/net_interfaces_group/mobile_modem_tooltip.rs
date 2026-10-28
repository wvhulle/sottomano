use sottomano_config::severity::Severity;
use sottomano_sources::network_connectivity::modem_manager_mobile::{Mobile, ModemState};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widget_groups::net_interfaces_group::interface_sections::signal_level,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_fill,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody},
        value_cells::LabeledValue,
        widget_state::WidgetState,
    },
};

impl BarWidget for Mobile {
    fn name(&self) -> &'static str {
        "Mobile modem"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Broadcast(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        match self.state {
            Some(ModemState::Connected) => {
                self.signal.map_or(Severity::Normal, signal_level).into()
            }
            Some(
                ModemState::Enabling
                | ModemState::Disabling
                | ModemState::Connecting
                | ModemState::Disconnecting,
            ) => WidgetState::Pending,
            _ => WidgetState::Off,
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        self.signal.map(percent_fill)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let signal_suffix = if self.signal_recent { "" } else { " (cached)" };
        TooltipBody::sections([
            Section::dictionary(
                [
                    self.operator
                        .as_deref()
                        .map(|operator| LabeledValue::new("Carrier", operator)),
                    (!self.technology.is_empty())
                        .then(|| LabeledValue::new("Network Type", self.technology.join(", "))),
                    self.signal.map(|signal| {
                        LabeledValue::new("Signal Strength", format!("{signal}%{signal_suffix}"))
                    }),
                    self.registration.map(|registration| {
                        LabeledValue::new("Network Registration", registration)
                    }),
                ]
                .into_iter()
                .flatten(),
            )
            .titled("Modem"),
            if self.state == Some(ModemState::Connected) {
                Section::dictionary(
                    [
                        self.ipv4
                            .as_deref()
                            .map(|address| LabeledValue::new("IPv4 Address", address)),
                        self.ipv6
                            .as_deref()
                            .map(|address| LabeledValue::new("IPv6 Address", address)),
                    ]
                    .into_iter()
                    .flatten(),
                )
                .titled("Address")
            } else {
                let failure = match self.state {
                    Some(ModemState::Failed {
                        reason: Some(reason),
                    }) => Some(LabeledValue::new("Failure Reason", reason)),
                    _ => None,
                };
                Section::dictionary(
                    self.state
                        .map(|state| LabeledValue::new("Connection State", state.label()))
                        .into_iter()
                        .chain(failure),
                )
                .titled("Status")
            },
            Section::dictionary(
                [
                    self.rx
                        .map(|rx| LabeledValue::new("Received (RX)", Quantity::size(rx))),
                    self.tx
                        .map(|tx| LabeledValue::new("Sent (TX)", Quantity::size(tx))),
                ]
                .into_iter()
                .flatten(),
            )
            .titled("Data"),
            Section::from(&self.identity),
        ])
    }
}
