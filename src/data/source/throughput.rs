use std::{
    fs,
    time::{Duration, Instant},
};

use iced::{
    Subscription,
    futures::{SinkExt, StreamExt},
};
use tokio::time::interval;
use tokio_stream::wrappers::IntervalStream;
use zbus::{Connection, proxy};

use crate::data::source::{
    BarWidget, Glyph, Level, TooltipBody, Unit, Value, font_awesome::Solid, row,
};

const FLOOR: f64 = 50_000.0;

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
    fn from_nm(state: u32) -> Self {
        match state {
            4 => Self::Full,
            2 | 3 => Self::Limited,
            1 => Self::None,
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

const fn cap<const UP: bool>() -> f64 {
    if UP { 12_500_000.0 } else { 25_000_000.0 }
}

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

    fn load_level(&self) -> Level {
        match self.gauge_percent() {
            95.. => Level::Critical,
            80..95 => Level::Warning,
            _ => Level::Ok,
        }
    }

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
    fn icon(&self) -> Glyph {
        if UP {
            Solid::Upload.into()
        } else {
            Solid::Download.into()
        }
    }

    fn disabled(&self) -> bool {
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

#[derive(Debug, Clone)]
pub struct Readings {
    pub down: Throughput<false>,
    pub up: Throughput<true>,
}

pub fn subscription() -> Subscription<Readings> {
    Subscription::run_with("throughput", |_| {
        iced::stream::channel(
            16,
            |mut output: iced::futures::channel::mpsc::Sender<Readings>| async move {
                let connection = Connection::system().await.ok();
                let mut ticks = IntervalStream::new(interval(Duration::from_secs(1)));
                let mut previous: Option<Sample> = None;
                while ticks.next().await.is_some() {
                    let current = capture();
                    let link = connectivity(connection.as_ref()).await;
                    let readings = rate(previous.as_ref(), &current, link);
                    previous = Some(current);
                    if output.send(readings).await.is_err() {
                        break;
                    }
                }
            },
        )
    })
}

#[derive(Default)]
struct Counters {
    rx: u64,
    tx: u64,
}

struct Sample {
    interface: Option<String>,
    counters: Counters,
    at: Instant,
}

#[derive(Default)]
struct Rates {
    rx: f64,
    tx: f64,
}

fn capture() -> Sample {
    let interface = egress_interface();
    let counters = interface.as_deref().map(read_counters).unwrap_or_default();
    Sample {
        interface,
        counters,
        at: Instant::now(),
    }
}

fn rate(previous: Option<&Sample>, current: &Sample, connectivity: Connectivity) -> Readings {
    let rates = previous
        .filter(|prev| current.interface.is_some() && prev.interface == current.interface)
        .and_then(|prev| {
            let seconds = current.at.duration_since(prev.at).as_secs_f64();
            (seconds > 0.0).then(|| Rates {
                rx: per_second(current.counters.rx, prev.counters.rx, seconds),
                tx: per_second(current.counters.tx, prev.counters.tx, seconds),
            })
        })
        .unwrap_or_default();
    Readings {
        down: Throughput {
            interface: current.interface.clone(),
            rate: rates.rx,
            connectivity,
        },
        up: Throughput {
            interface: current.interface.clone(),
            rate: rates.tx,
            connectivity,
        },
    }
}

#[allow(clippy::cast_precision_loss)]
fn per_second(now: u64, was: u64, seconds: f64) -> f64 {
    now.saturating_sub(was) as f64 / seconds
}

fn read_counters(interface: &str) -> Counters {
    let counter = |file: &str| {
        fs::read_to_string(format!("/sys/class/net/{interface}/statistics/{file}"))
            .ok()
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(0)
    };
    Counters {
        rx: counter("rx_bytes"),
        tx: counter("tx_bytes"),
    }
}

struct DefaultRoute {
    interface: String,
    metric: u32,
}

fn egress_interface() -> Option<String> {
    let table = fs::read_to_string("/proc/net/route").ok()?;
    table
        .lines()
        .skip(1)
        .filter_map(parse_route)
        .min_by_key(|route| route.metric)
        .map(|route| route.interface)
}

fn parse_route(line: &str) -> Option<DefaultRoute> {
    let mut fields = line.split_whitespace();
    let interface = fields.next()?;
    let destination = fields.next()?;
    let metric = fields.nth(4)?.parse().ok()?;
    (destination == "00000000").then(|| DefaultRoute {
        interface: interface.to_owned(),
        metric,
    })
}

#[proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait NetworkManager {
    #[zbus(property)]
    fn connectivity(&self) -> zbus::Result<u32>;
}

async fn connectivity(connection: Option<&Connection>) -> Connectivity {
    let Some(connection) = connection else {
        return Connectivity::Unknown;
    };
    match NetworkManagerProxy::new(connection).await {
        Ok(proxy) => proxy
            .connectivity()
            .await
            .map_or(Connectivity::Unknown, Connectivity::from_nm),
        Err(_) => Connectivity::Unknown,
    }
}
