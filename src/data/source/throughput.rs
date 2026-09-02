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

const FLOOR: ByteSize = ByteSize::kb(10);
const CEILING: ByteSize = ByteSize::mb(125);

pub trait Direction {
    const LABEL: &'static str;
    const ICON: Solid;
}

#[derive(Debug, Clone)]
pub struct Upload;

impl Direction for Upload {
    const LABEL: &'static str = "Up";
    const ICON: Solid = Solid::Upload;
}

#[derive(Debug, Clone)]
pub struct Download;

impl Direction for Download {
    const LABEL: &'static str = "Down";
    const ICON: Solid = Solid::Download;
}

#[derive(Debug, Clone)]
pub struct Throughput<D: Direction> {
    interfaces: Vec<String>,
    rate: ByteSize,
    peak_rate: ByteSize,
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

fn format_rate(rate: ByteSize) -> String {
    format!("{}/s", rate.display().iec())
}

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
fn scaled(rate: ByteSize) -> f32 {
    if rate <= FLOOR {
        return 0.0;
    }
    let span = (CEILING.as_u64() as f64 / FLOOR.as_u64() as f64).ln();
    let reach = (rate.as_u64() as f64 / FLOOR.as_u64() as f64).ln();
    ((reach / span) as f32).clamp(0.0, 1.0)
}

#[allow(clippy::cast_possible_truncation)]
fn percent(fraction: f32) -> i64 {
    (fraction * 100.0).round() as i64
}

impl<D: Direction> Throughput<D> {
    fn current_fraction(&self) -> f32 {
        if self.interfaces.is_empty() {
            return 0.0;
        }
        scaled(self.rate)
    }

    fn peak_fraction(&self) -> f32 {
        if self.interfaces.is_empty() {
            return 0.0;
        }
        scaled(self.peak_rate)
    }

    fn load_level(&self) -> Level {
        match percent(self.current_fraction()) {
            95.. => Level::Critical,
            80..95 => Level::Warning,
            _ => Level::Ok,
        }
    }
}

impl<D: Direction> BarWidget for Throughput<D> {
    fn icon(&self) -> Glyph {
        D::ICON.into()
    }

    fn disabled(&self) -> bool {
        self.peak_rate < FLOOR && self.level() == Level::Ok
    }

    fn level(&self) -> Level {
        if self.interfaces.is_empty() {
            return Level::Critical;
        }
        self.connectivity.level().max(self.load_level())
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: percent(self.peak_fraction()),
            unit: Unit::Percent,
        })
    }

    fn live_fraction(&self) -> f32 {
        let peak = self.peak_fraction();
        if peak <= 0.0 {
            return 1.0;
        }
        (self.current_fraction() / peak).clamp(0.0, 1.0)
    }

    fn tooltip(&self) -> TooltipBody {
        if self.interfaces.is_empty() {
            return TooltipBody::rows(vec![row("Connectivity", "none")]);
        }
        TooltipBody::rows(vec![
            row(D::LABEL, format_rate(self.rate)),
            row("Peak", format_rate(self.peak_rate)),
            row("Interfaces", self.interfaces.join(", ")),
            row("Connectivity", self.connectivity.label()),
            row("Capacity", format_rate(CEILING)),
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
            let mut peak = Rates::default();
            while ticks.next().await.is_some() {
                let current = capture();
                let link = connectivity(connection.as_ref()).await;
                let rates = sample_rates(previous.as_ref(), &current);
                peak = peak.raised_by(rates);
                let readings = readings(&current, rates, peak, link);
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

#[derive(Default, Clone, Copy)]
struct Rates {
    rx: ByteSize,
    tx: ByteSize,
}

impl Rates {
    fn raised_by(self, other: Self) -> Self {
        Self {
            rx: self.rx.max(other.rx),
            tx: self.tx.max(other.tx),
        }
    }
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

fn sample_rates(previous: Option<&Sample>, current: &Sample) -> Rates {
    previous
        .map(|prev| current.at.duration_since(prev.at))
        .filter(|elapsed| elapsed.as_millis() > 0)
        .zip(previous)
        .map(|(elapsed, prev)| {
            current
                .counters
                .iter()
                .filter_map(|(interface, now)| Some((now, prev.counters.get(interface)?)))
                .fold(Rates::default(), |total, (now, was)| Rates {
                    rx: total.rx + per_second(now.rx, was.rx, elapsed),
                    tx: total.tx + per_second(now.tx, was.tx, elapsed),
                })
        })
        .unwrap_or_default()
}

fn readings(current: &Sample, rates: Rates, peak: Rates, connectivity: Connectivity) -> Readings {
    let interfaces: Vec<String> = current.counters.keys().cloned().collect();
    Readings {
        down: Throughput {
            interfaces: interfaces.clone(),
            rate: rates.rx,
            peak_rate: peak.rx,
            connectivity,
            direction: PhantomData,
        },
        up: Throughput {
            interfaces,
            rate: rates.tx,
            peak_rate: peak.tx,
            connectivity,
            direction: PhantomData,
        },
    }
}

fn per_second(now: u64, was: u64, elapsed: Duration) -> ByteSize {
    let bytes = u128::from(now.saturating_sub(was)) * 1000 / elapsed.as_millis();
    ByteSize::b(u64::try_from(bytes).unwrap_or(u64::MAX))
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
