//! Network throughput on the default-route interface, one gauge per direction.
//! odin samples `/sys/class/net/<iface>/statistics` continuously; here each
//! poll self-measures over a short window (it runs off the executor, so the
//! in-poll sleep is free), which keeps it a plain stateless [`Source`].
//!
//! The gauge is log-scaled between a noise floor and a per-direction cap, so a
//! busy link reads high without a trickle pinning it. Offline (no default
//! route) is a critical, empty gauge.

use std::{fs, thread::sleep, time::Duration};

use serde_json::Value as Json;

use crate::{
    data::source::{BarWidget, Level, Source, TooltipBody, Unit, Value, command::output, row},
    widgets::icon::Icon,
};

/// Below this rate the gauge reads empty (odin's 50 kB/s floor).
const FLOOR: f64 = 50_000.0;
/// The measurement window; the rate is the byte delta over it.
const WINDOW: Duration = Duration::from_millis(500);

#[derive(Debug, Clone)]
pub struct Throughput<const UP: bool> {
    interface: Option<String>,
    rate: f64,
    connectivity: Connectivity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Connectivity {
    Full,
    Limited,
    None,
    Unknown,
}

impl Connectivity {
    fn parse(s: &str) -> Self {
        match s.trim() {
            "full" => Self::Full,
            "portal" | "limited" => Self::Limited,
            "none" => Self::None,
            _ => Self::Unknown,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Limited => "limited",
            Self::None => "none",
            Self::Unknown => "unknown",
        }
    }

    fn level(self) -> Level {
        match self {
            Self::Full => Level::Ok,
            Self::Limited | Self::Unknown => Level::Warning,
            Self::None => Level::Critical,
        }
    }
}

/// Per-direction gauge cap: 25 MB/s down, 12.5 MB/s up (odin's values).
const fn cap<const UP: bool>() -> f64 {
    if UP { 12_500_000.0 } else { 25_000_000.0 }
}

/// A byte-rate as a compact `N.N U/s` string.
fn format_rate(bytes_per_sec: f64) -> String {
    const UNITS: [&str; 4] = ["B", "kB", "MB", "GB"];
    let mut value = bytes_per_sec;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value:.0} {}/s", UNITS[unit])
    } else {
        format!("{value:.1} {}/s", UNITS[unit])
    }
}

impl<const UP: bool> Throughput<UP> {
    fn idle(&self) -> bool {
        self.rate < FLOOR
    }

    /// Load severity from the log gauge fill (odin: ≥95 critical, ≥80 warning).
    fn load_level(&self) -> Level {
        match self.gauge_percent() {
            95.. => Level::Critical,
            80..95 => Level::Warning,
            _ => Level::Ok,
        }
    }

    /// Log-scaled gauge fill between [`FLOOR`] and the direction's [`cap`].
    fn gauge_percent(&self) -> i64 {
        if self.interface.is_none() || self.rate < FLOOR {
            return 0;
        }
        let fraction = (self.rate / FLOOR).ln() / (cap::<UP>() / FLOOR).ln();
        #[allow(clippy::cast_possible_truncation)]
        let percent = (fraction * 100.0).round() as i64;
        percent.clamp(0, 100)
    }
}

impl<const UP: bool> BarWidget for Throughput<UP> {
    fn icon(&self) -> Icon {
        if UP { Icon::Upload } else { Icon::Download }
    }

    fn disabled(&self) -> bool {
        // Dim a quiet link, but only when nothing else is wrong.
        self.idle() && self.level() == Level::Ok
    }

    fn level(&self) -> Level {
        if self.interface.is_none() {
            return Level::Critical;
        }
        self.connectivity.level().max(self.load_level())
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.gauge_percent(),
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let Some(interface) = &self.interface else {
            return TooltipBody::Table(vec![row("Connectivity", "none")]);
        };
        let direction = if UP { "Up" } else { "Down" };
        TooltipBody::Table(vec![
            row(direction, format_rate(self.rate)),
            row("Interface", interface.clone()),
            row("Connectivity", self.connectivity.label()),
            row("Capacity", format_rate(cap::<UP>())),
        ])
    }
}

impl<const UP: bool> Source for Throughput<UP> {
    const NAME: &'static str = if UP {
        "throughput-up"
    } else {
        "throughput-down"
    };
    const INTERVAL: Duration = Duration::from_secs(1);

    fn poll() -> Option<Self> {
        let connectivity = output("nmcli", &["networking", "connectivity"])
            .map_or(Connectivity::Unknown, |s| Connectivity::parse(&s));
        let Some(interface) = egress_interface() else {
            return Some(Throughput {
                interface: None,
                rate: 0.0,
                connectivity,
            });
        };
        let counter = |iface: &str| {
            let file = if UP { "tx_bytes" } else { "rx_bytes" };
            fs::read_to_string(format!("/sys/class/net/{iface}/statistics/{file}"))
                .ok()?
                .trim()
                .parse::<u64>()
                .ok()
        };
        let before = counter(&interface)?;
        sleep(WINDOW);
        let after = counter(&interface)?;
        #[allow(clippy::cast_precision_loss)]
        let rate = after.saturating_sub(before) as f64 / WINDOW.as_secs_f64();
        Some(Throughput {
            interface: Some(interface),
            rate,
            connectivity,
        })
    }
}

/// The default-route egress interface via `ip route get` (a routing lookup, no
/// packets). `None` when offline.
fn egress_interface() -> Option<String> {
    let json: Json =
        serde_json::from_str(&output("ip", &["-j", "route", "get", "1.1.1.1"])?).ok()?;
    json.as_array()?
        .first()?
        .get("dev")?
        .as_str()
        .map(str::to_string)
}
