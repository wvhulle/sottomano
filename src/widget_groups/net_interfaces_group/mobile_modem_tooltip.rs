use uom::si::f64::Ratio;

use crate::{
    data::{severity::Severity, source::net_interfaces::mobile::Mobile},
    widget_groups::net_interfaces_group::interface_sections::signal_level,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_fill,
        hardware_section::HardwareRows,
        phosphor::Phosphor,
        tooltip_body::{Section, TooltipBody},
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
            Section::titled(
                "Modem",
                &[
                    ("Carrier", self.operator.clone()),
                    (
                        "Network Type",
                        (!self.technology.is_empty()).then(|| self.technology.join(", ")),
                    ),
                    (
                        "Signal Strength",
                        self.signal
                            .map(|signal| format!("{signal}%{signal_suffix}")),
                    ),
                    ("Network Registration", self.registration.clone()),
                ],
            ),
            if self.state == CONNECTED {
                Section::titled(
                    "Address",
                    &[("IPv4 Address", &self.ipv4), ("IPv6 Address", &self.ipv6)],
                )
            } else {
                Section::titled(
                    "Status",
                    &[
                        ("Connection State", Some(self.state.clone())),
                        (
                            "Failure Reason",
                            self.failed_reason
                                .clone()
                                .filter(|_| self.state == "failed"),
                        ),
                    ],
                )
            },
            Section::titled(
                "Data",
                &[
                    (
                        "Received (RX)",
                        self.rx.map(|rx| rx.display().iec().to_string()),
                    ),
                    (
                        "Sent (TX)",
                        self.tx.map(|tx| tx.display().iec().to_string()),
                    ),
                ],
            ),
            self.identity.hardware_section(),
        ])
    }
}
