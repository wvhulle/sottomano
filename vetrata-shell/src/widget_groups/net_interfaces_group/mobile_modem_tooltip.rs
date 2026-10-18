use uom::si::f64::Ratio;
use vetrata_config::severity::Severity;

use crate::{
    data::source::net_interfaces::mobile::{Mobile, ModemState},
    widget_groups::net_interfaces_group::interface_sections::signal_level,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_fill,
        phosphor::Phosphor,
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for Mobile {
    fn name(&self) -> &'static str {
        "Mobile modem"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Broadcast(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        if self.state == Some(ModemState::Connected) {
            self.signal.map_or(Severity::Normal, signal_level).into()
        } else {
            WidgetState::Off
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        self.signal.map(percent_fill)
    }

    fn tooltip(&self) -> TooltipBody {
        let signal_suffix = if self.signal_recent { "" } else { " (cached)" };
        TooltipBody::sections([
            Section::dictionary(
                [
                    self.operator
                        .as_deref()
                        .map(|operator| KeyValue::new("Carrier", operator)),
                    (!self.technology.is_empty())
                        .then(|| KeyValue::new("Network Type", self.technology.join(", "))),
                    self.signal.map(|signal| {
                        KeyValue::new("Signal Strength", format!("{signal}%{signal_suffix}"))
                    }),
                    self.registration
                        .map(|registration| KeyValue::new("Network Registration", registration)),
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
                            .map(|address| KeyValue::new("IPv4 Address", address)),
                        self.ipv6
                            .as_deref()
                            .map(|address| KeyValue::new("IPv6 Address", address)),
                    ]
                    .into_iter()
                    .flatten(),
                )
                .titled("Address")
            } else {
                let failure = match self.state {
                    Some(ModemState::Failed {
                        reason: Some(reason),
                    }) => Some(KeyValue::new("Failure Reason", reason)),
                    _ => None,
                };
                Section::dictionary(
                    self.state
                        .map(|state| KeyValue::new("Connection State", state.label()))
                        .into_iter()
                        .chain(failure),
                )
                .titled("Status")
            },
            Section::dictionary(
                [
                    self.rx
                        .map(|rx| KeyValue::new("Received (RX)", rx.display().iec().to_string())),
                    self.tx
                        .map(|tx| KeyValue::new("Sent (TX)", tx.display().iec().to_string())),
                ]
                .into_iter()
                .flatten(),
            )
            .titled("Data"),
            Section::hardware_identity(self.identity.hardware_entries()),
        ])
    }
}
