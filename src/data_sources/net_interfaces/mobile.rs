//! Mobile broadband via `ModemManager` (`mmcli -J`). Off the bar when no modem
//! is present; dimmed unless a data connection is up; otherwise an icon with a
//! signal gauge and an operator/technology/registration tooltip.

use std::time::Duration;

use serde_json::Value as Json;

use crate::{
    data_sources::{
        BarWidget, Level, Source, TooltipBody, Unit, Value, command::output,
        net_interfaces::signal_level, row,
    },
    widgets::icon::Icon,
};

#[derive(Debug, Clone)]
pub struct Mobile {
    model: String,
    operator: String,
    technology: Vec<String>,
    signal: Option<i64>,
    registration: String,
    state: String,
    ip: Option<String>,
    failed_reason: Option<String>,
    last_error: Option<String>,
}

impl BarWidget for Mobile {
    fn icon(&self) -> Icon {
        Icon::TowerCell
    }

    fn disabled(&self) -> bool {
        self.state != "connected"
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
        let mut rows = vec![
            row("Model", self.model.clone()),
            row("Operator", self.operator.clone()),
            row("Technology", self.technology.join(", ")),
        ];
        if let Some(signal) = self.signal {
            rows.push(row("Signal", format!("{signal}%")));
        }
        rows.push(row("Registration", self.registration.clone()));
        match &self.ip {
            Some(ip) if self.state == "connected" => rows.push(row("IP", ip.clone())),
            _ => rows.push(row("State", self.state.clone())),
        }
        if self.state == "failed"
            && let Some(reason) = &self.failed_reason
        {
            rows.push(row("FailedReason", reason.clone()));
        }
        if let Some(error) = &self.last_error {
            rows.push(row("LastError", error.clone()));
        }
        TooltipBody::Table(rows)
    }
}

/// The `mmcli` object id at the end of a D-Bus path like
/// `/org/freedesktop/ModemManager1/Modem/0`.
fn last_segment(path: &str) -> Option<&str> {
    path.rsplit('/').next().filter(|s| !s.is_empty())
}

fn string_field(value: &Json, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(Json::as_str)
        .filter(|s| !s.is_empty() && *s != "--")
        .map(str::to_string)
}

impl Source for Mobile {
    const NAME: &'static str = "mobile-broadband";
    const INTERVAL: Duration = Duration::from_secs(10);

    fn poll() -> Option<Self> {
        let list: Json = serde_json::from_str(&output("mmcli", &["-L", "-J"])?).ok()?;
        let path = list
            .pointer("/modem-list")
            .and_then(Json::as_array)
            .and_then(|modems| modems.first())
            .and_then(Json::as_str)?;
        let id = last_segment(path)?;
        let modem: Json = serde_json::from_str(&output("mmcli", &["-m", id, "-J"])?).ok()?;

        let generic = modem.pointer("/modem/generic");
        let state =
            string_field(&modem, "/modem/generic/state").unwrap_or_else(|| "unknown".into());
        let signal = generic
            .and_then(|g| g.pointer("/signal-quality/value"))
            .and_then(|v| {
                v.as_str()
                    .and_then(|s| s.parse().ok())
                    .or_else(|| v.as_i64())
            });
        let technology = generic
            .and_then(|g| g.pointer("/access-technologies"))
            .and_then(Json::as_array)
            .map(|techs| {
                techs
                    .iter()
                    .filter_map(Json::as_str)
                    .filter(|s| !s.is_empty() && *s != "--")
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();

        let bearer = modem
            .pointer("/modem/generic/bearers")
            .and_then(Json::as_array)
            .and_then(|bearers| bearers.first())
            .and_then(Json::as_str)
            .and_then(last_segment)
            .and_then(|bearer_id| output("mmcli", &["-b", bearer_id, "-J"]))
            .and_then(|out| serde_json::from_str::<Json>(&out).ok());
        let ip = bearer
            .as_ref()
            .and_then(|b| string_field(b, "/bearer/ipv4-config/address"));
        let last_error = bearer.as_ref().and_then(|b| {
            let name = string_field(b, "/bearer/status/connection-error/name")?;
            let message =
                string_field(b, "/bearer/status/connection-error/message").unwrap_or_default();
            Some(format!("{name}: {message}"))
        });

        Some(Mobile {
            model: string_field(&modem, "/modem/generic/model").unwrap_or_else(|| "—".into()),
            operator: string_field(&modem, "/modem/3gpp/operator-name")
                .unwrap_or_else(|| "—".into()),
            technology,
            signal,
            registration: string_field(&modem, "/modem/3gpp/registration-state")
                .unwrap_or_else(|| "—".into()),
            state,
            ip,
            failed_reason: string_field(&modem, "/modem/generic/state-failed-reason"),
            last_error,
        })
    }
}
