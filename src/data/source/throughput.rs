use std::{
    collections::BTreeMap,
    fs,
    marker::PhantomData,
    time::{Duration, Instant},
};

use bytesize::ByteSize;
use iced::{
    Subscription,
    futures::{SinkExt, StreamExt, channel::mpsc::Sender},
    stream::channel,
};
use tokio::time::interval;
use tokio_stream::wrappers::IntervalStream;
use zbus::{Connection, proxy};

use crate::data::source::{
    BarWidget, Glyph, Level, TooltipBody, Unit, Value, font_awesome::Solid, row,
};

const FLOOR: f64 = 10_000.0;

pub trait Direction {
    const LABEL: &'static str;
    const ICON: Solid;
    const CAP_BYTES_PER_SEC: f64;
}

#[derive(Debug, Clone)]
pub struct Upload;

impl Direction for Upload {
    const LABEL: &'static str = "Up";
    const ICON: Solid = Solid::Upload;
    const CAP_BYTES_PER_SEC: f64 = 12_500_000.0;
}

#[derive(Debug, Clone)]
pub struct Download;

impl Direction for Download {
    const LABEL: &'static str = "Down";
    const ICON: Solid = Solid::Download;
    const CAP_BYTES_PER_SEC: f64 = 25_000_000.0;
}

#[derive(Debug, Clone)]
pub struct Throughput<D: Direction> {
    interfaces: Vec<String>,
    rate: f64,
    connectivity: Connectivity,
    direction: PhantomData<D>,
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

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn format_rate(bytes_per_sec: f64) -> String {
    let bytes = bytes_per_sec.max(0.0) as u64;
    format!("{}/s", ByteSize::b(bytes).display().iec())
}

impl<D: Direction> Throughput<D> {
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
        if self.interfaces.is_empty() || self.rate < FLOOR {
            return 0;
        }
        let fraction = (self.rate / FLOOR).ln() / (D::CAP_BYTES_PER_SEC / FLOOR).ln();
        #[allow(clippy::cast_possible_truncation)]
        let percent = (fraction * 100.0).round() as i64;
        percent.clamp(0, 100)
    }
}

impl<D: Direction> BarWidget for Throughput<D> {
    fn icon(&self) -> Glyph {
        D::ICON.into()
    }

    fn disabled(&self) -> bool {
        self.idle() && self.level() == Level::Ok
    }

    fn level(&self) -> Level {
        if self.interfaces.is_empty() {
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
        if self.interfaces.is_empty() {
            return TooltipBody::rows(vec![row("Connectivity", "none")]);
        }
        TooltipBody::rows(vec![
            row(D::LABEL, format_rate(self.rate)),
            row("Interfaces", self.interfaces.join(", ")),
            row("Connectivity", self.connectivity.label()),
            row("Capacity", format_rate(D::CAP_BYTES_PER_SEC)),
        ])
    }
}

#[derive(Debug, Clone)]
pub struct Readings {
    pub down: Throughput<Download>,
    pub up: Throughput<Upload>,
}

pub fn subscription() -> Subscription<Readings> {
    Subscription::run_with("throughput", |_| {
        channel(16, |mut output: Sender<Readings>| async move {
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
        })
    })
}

#[derive(Default, Clone, Copy)]
struct Counters {
    rx: u64,
    tx: u64,
}

struct Sample {
    counters: BTreeMap<String, Counters>,
    at: Instant,
}

#[derive(Default)]
struct Rates {
    rx: f64,
    tx: f64,
}

fn capture() -> Sample {
    Sample {
        counters: carrying_interfaces()
            .map(|interface| {
                let counters = read_counters(&interface);
                (interface, counters)
            })
            .collect(),
        at: Instant::now(),
    }
}

fn rate(previous: Option<&Sample>, current: &Sample, connectivity: Connectivity) -> Readings {
    let rates = previous
        .map(|prev| current.at.duration_since(prev.at).as_secs_f64())
        .filter(|seconds| *seconds > 0.0)
        .zip(previous)
        .map(|(seconds, prev)| {
            current
                .counters
                .iter()
                .filter_map(|(interface, now)| Some((now, prev.counters.get(interface)?)))
                .fold(Rates::default(), |total, (now, was)| Rates {
                    rx: total.rx + per_second(now.rx, was.rx, seconds),
                    tx: total.tx + per_second(now.tx, was.tx, seconds),
                })
        })
        .unwrap_or_default();
    let interfaces: Vec<String> = current.counters.keys().cloned().collect();
    Readings {
        down: Throughput {
            interfaces: interfaces.clone(),
            rate: rates.rx,
            connectivity,
            direction: PhantomData,
        },
        up: Throughput {
            interfaces,
            rate: rates.tx,
            connectivity,
            direction: PhantomData,
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

fn carrying_interfaces() -> impl Iterator<Item = String> {
    fs::read_dir("/sys/class/net")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join("device").exists())
        .filter(|path| {
            fs::read_to_string(path.join("operstate")).is_ok_and(|state| state.trim() == "up")
        })
        .filter_map(|path| path.file_name()?.to_str().map(str::to_owned))
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
