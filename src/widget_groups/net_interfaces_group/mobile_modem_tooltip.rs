use crate::{
    data::source::net_interfaces::mobile::Mobile,
    widget_groups::net_interfaces_group::interface_sections::signal_level,
    widgets::{
        bar_widget::{BarWidget, Glyph, Level, Unit, Value},
        phosphor,
        tooltip_body::{Row, Section, TooltipBody, row, section},
    },
};

const CONNECTED: &str = "connected";

impl BarWidget for Mobile {
    fn icon(&self) -> Glyph {
        phosphor::Broadcast::Regular.into()
    }

    fn disabled(&self) -> bool {
        self.state != CONNECTED
    }

    fn level(&self) -> Level {
        self.signal.map_or(Level::Ok, signal_level)
    }

    fn value(&self) -> Option<Value> {
        self.signal.map(|percent| Value::Gauge {
            percent,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Sections(vec![
            section("Modem", modem_rows(self)),
            if self.state == CONNECTED {
                address_section(self)
            } else {
                status_section(self)
            },
            section("Data", transfer_rows(self)),
        ])
    }
}

fn modem_rows(mobile: &Mobile) -> Vec<Row> {
    let mut rows = Vec::new();
    rows.extend(
        mobile
            .model
            .as_ref()
            .map(|model| row("Modem Model", model.clone())),
    );
    rows.extend(
        mobile
            .operator
            .as_ref()
            .map(|operator| row("Carrier", operator.clone())),
    );
    if !mobile.technology.is_empty() {
        rows.push(row("Network Type", mobile.technology.join(", ")));
    }
    rows.extend(mobile.signal.map(|signal| {
        let suffix = if mobile.signal_recent {
            ""
        } else {
            " (cached)"
        };
        row("Signal Strength", format!("{signal}%{suffix}"))
    }));
    rows.extend(
        mobile
            .registration
            .as_ref()
            .map(|registration| row("Network Registration", registration.clone())),
    );
    rows
}

fn address_section(mobile: &Mobile) -> Section {
    let mut rows = Vec::new();
    rows.extend(
        mobile
            .ipv4
            .as_ref()
            .map(|ip| row("IPv4 Address", ip.clone())),
    );
    rows.extend(
        mobile
            .ipv6
            .as_ref()
            .map(|ip| row("IPv6 Address", ip.clone())),
    );
    section("Address", rows)
}

fn status_section(mobile: &Mobile) -> Section {
    let mut rows = vec![row("Connection State", mobile.state.clone())];
    if mobile.state == "failed" {
        rows.extend(
            mobile
                .failed_reason
                .as_ref()
                .map(|reason| row("Failure Reason", reason.clone())),
        );
    }
    section("Status", rows)
}

fn transfer_rows(mobile: &Mobile) -> Vec<Row> {
    [
        mobile
            .rx
            .map(|rx| row("Received (RX)", rx.display().iec().to_string())),
        mobile
            .tx
            .map(|tx| row("Sent (TX)", tx.display().iec().to_string())),
    ]
    .into_iter()
    .flatten()
    .collect()
}
