use uom::si::f64::Ratio;

use crate::{
    data::{severity::Severity, source::net_interfaces::mobile::Mobile},
    widget_groups::net_interfaces_group::interface_sections::signal_level,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_fill,
        hardware_section::HardwareEntries,
        phosphor::Phosphor,
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

const CONNECTED: &str = "connected";

impl BarWidget for Mobile {
    fn icon(&self) -> Glyph {
        Phosphor::Broadcast(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        if self.state == CONNECTED {
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
        TooltipBody::sections(&[
            Section::dictionary(
                "Modem",
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
                        .as_deref()
                        .map(|registration| KeyValue::new("Network Registration", registration)),
                ]
                .into_iter()
                .flatten(),
            ),
            if self.state == CONNECTED {
                Section::dictionary(
                    "Address",
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
            } else {
                let failure = self
                    .failed_reason
                    .as_deref()
                    .filter(|_| self.state == "failed")
                    .map(|reason| KeyValue::new("Failure Reason", reason));
                Section::dictionary(
                    "Status",
                    [KeyValue::new("Connection State", self.state.as_str())]
                        .into_iter()
                        .chain(failure),
                )
            },
            Section::dictionary(
                "Data",
                [
                    self.rx
                        .map(|rx| KeyValue::new("Received (RX)", rx.display().iec().to_string())),
                    self.tx
                        .map(|tx| KeyValue::new("Sent (TX)", tx.display().iec().to_string())),
                ]
                .into_iter()
                .flatten(),
            ),
            self.identity.hardware_section(),
        ])
    }
}
