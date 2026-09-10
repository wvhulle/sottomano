use std::{
    collections::BTreeMap,
    fs,
    marker::PhantomData,
    ops::Add,
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

pub const BACKGROUND_CHATTER: ByteSize = ByteSize::kb(10);
pub const SCALE_START: ByteSize = ByteSize::kb(100);
pub const SCALE_END: ByteSize = ByteSize::mb(125);

pub trait Direction {
    const LABEL: &'static str;
}

#[derive(Debug, Clone)]
pub struct Upload;

impl Direction for Upload {
    const LABEL: &'static str = "Up";
}

#[derive(Debug, Clone)]
pub struct Download;

impl Direction for Download {
    const LABEL: &'static str = "Down";
}

#[derive(Debug, Clone)]
pub struct Throughput<D: Direction> {
    pub interfaces: Vec<String>,
    pub rate: ByteSize,
    pub peak_rate: ByteSize,
    pub connectivity: Connectivity,
    direction: PhantomData<D>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connectivity {
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

    pub fn label(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Limited => "limited",
            Self::None => "none",
            Self::Unknown => "unknown",
        }
    }
}

#[allow(clippy::cast_precision_loss)]
fn ratio(quantity: ByteSize, reference: ByteSize) -> f64 {
    quantity.as_u64() as f64 / reference.as_u64() as f64
}

#[allow(clippy::cast_possible_truncation)]
pub fn fraction_of_scale(rate: ByteSize) -> f32 {
    if rate <= SCALE_START {
        return 0.0;
    }
    let log_width_of_scale = ratio(SCALE_END, SCALE_START).ln();
    let log_position_of_rate = ratio(rate, SCALE_START).ln();
    ((log_position_of_rate / log_width_of_scale) as f32).clamp(0.0, 1.0)
}

impl<D: Direction> Throughput<D> {
    fn fraction_of_carried(&self, rate: ByteSize) -> f32 {
        if self.interfaces.is_empty() {
            return 0.0;
        }
        fraction_of_scale(rate)
    }

    pub fn current_fraction(&self) -> f32 {
        self.fraction_of_carried(self.rate)
    }

    pub fn peak_fraction(&self) -> f32 {
        self.fraction_of_carried(self.peak_rate)
    }

    pub fn unused(&self) -> bool {
        self.peak_rate <= BACKGROUND_CHATTER
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
            let mut session = Session::default();
            while ticks.next().await.is_some() {
                let link = connectivity(connection.as_ref()).await;
                if output.send(session.observe(capture(), link)).await.is_err() {
                    break;
                }
            }
        })
    })
}

#[derive(Default, Clone, Copy)]
struct Counters {
    rx: ByteSize,
    tx: ByteSize,
}

struct Sample {
    counters: BTreeMap<String, Counters>,
    at: Instant,
}

#[derive(Default)]
struct Session {
    previous: Option<Sample>,
    peak: Rates,
}

impl Session {
    fn observe(&mut self, current: Sample, connectivity: Connectivity) -> Readings {
        let rates = self
            .previous
            .as_ref()
            .map_or_else(Rates::default, |previous| current.rates_since(previous));
        self.peak = self.peak.raised_by(rates);
        let readings = readings(&current, rates, self.peak, connectivity);
        self.previous = Some(current);
        readings
    }
}

#[derive(Default, Clone, Copy)]
struct Rates {
    rx: ByteSize,
    tx: ByteSize,
}

impl Rates {
    fn combine(self, other: Self, join: impl Fn(ByteSize, ByteSize) -> ByteSize) -> Self {
        Self {
            rx: join(self.rx, other.rx),
            tx: join(self.tx, other.tx),
        }
    }

    fn raised_by(self, other: Self) -> Self {
        self.combine(other, Ord::max)
    }
}

impl Add for Rates {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        self.combine(other, Add::add)
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

impl Sample {
    fn rates_since(&self, previous: &Self) -> Rates {
        let elapsed = self.at.duration_since(previous.at);
        if elapsed.as_millis() == 0 {
            return Rates::default();
        }
        self.counters
            .iter()
            .filter_map(|(interface, now)| Some((now, previous.counters.get(interface)?)))
            .map(|(now, was)| Rates {
                rx: per_second(now.rx, was.rx, elapsed),
                tx: per_second(now.tx, was.tx, elapsed),
            })
            .fold(Rates::default(), Add::add)
    }
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

fn per_second(now: ByteSize, was: ByteSize, elapsed: Duration) -> ByteSize {
    let transferred = now.as_u64().saturating_sub(was.as_u64());
    let bytes_per_second = u128::from(transferred) * 1000 / elapsed.as_millis();
    ByteSize::b(u64::try_from(bytes_per_second).unwrap_or(u64::MAX))
}

fn read_counters(interface: &str) -> Counters {
    let counter = |file: &str| {
        fs::read_to_string(format!("/sys/class/net/{interface}/statistics/{file}"))
            .ok()
            .and_then(|value| value.trim().parse().ok())
            .map_or(ByteSize::default(), ByteSize::b)
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
    reported_connectivity(connection)
        .await
        .map_or(Connectivity::Unknown, Connectivity::from_nm)
}

async fn reported_connectivity(connection: Option<&Connection>) -> Option<u32> {
    let proxy = NetworkManagerProxy::new(connection?).await.ok()?;
    proxy.connectivity().await.ok()
}
