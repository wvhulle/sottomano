use std::{
    collections::BTreeMap,
    convert::Infallible,
    fs,
    future::ready,
    marker::PhantomData,
    mem,
    ops::Add,
    path::Path,
    time::{Duration, Instant},
};

use bytesize::ByteSize;
use iced::futures::{Stream, StreamExt, stream::select};
use tokio::time::sleep;
use uom::si::{
    f64::{Information, InformationRate, Time},
    information::byte,
    information_rate::{kilobyte_per_second, megabyte_per_second},
    ratio::ratio,
    time::second,
};
use zbus::Connection;

use crate::{
    data::source::{
        Delivery, PolledSource, Source, emit_delivery_changes,
        net_interfaces::{
            nm::{self, Connectivity},
            nm_readings,
        },
    },
    message_subscriptions::ticked,
};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(2);
const SAMPLE_WINDOW: Duration = Duration::from_secs(1);
const BACKGROUND_CHATTER_KB_PER_S: f64 = 10.0;
const SCALE_START_KB_PER_S: f64 = 100.0;
pub const SCALE_END_MB_PER_S: f64 = 125.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Upload;

#[derive(Debug, Clone, PartialEq)]
pub struct Download;

#[derive(Debug, Clone, PartialEq)]
pub struct Total;

#[derive(Debug, Clone, PartialEq)]
pub struct Throughput<D> {
    pub interfaces: Vec<String>,
    pub rate: InformationRate,
    pub peak_rate: InformationRate,
    pub connectivity: Option<Connectivity>,
    direction: PhantomData<D>,
}

#[allow(clippy::cast_possible_truncation)]
pub fn fraction_of_scale(rate: InformationRate) -> f32 {
    let scale_start = InformationRate::new::<kilobyte_per_second>(SCALE_START_KB_PER_S);
    let scale_end = InformationRate::new::<megabyte_per_second>(SCALE_END_MB_PER_S);
    if rate <= scale_start {
        return 0.0;
    }
    let log_width_of_scale = (scale_end / scale_start).get::<ratio>().ln();
    let log_position_of_rate = (rate / scale_start).get::<ratio>().ln();
    ((log_position_of_rate / log_width_of_scale) as f32).clamp(0.0, 1.0)
}

impl<D> Throughput<D> {
    fn fraction_of_carried(&self, rate: InformationRate) -> f32 {
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
        self.peak_rate <= InformationRate::new::<kilobyte_per_second>(BACKGROUND_CHATTER_KB_PER_S)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Readings {
    pub down: Throughput<Download>,
    pub up: Throughput<Upload>,
    pub total: Throughput<Total>,
}

impl Source for Readings {
    type Error = Infallible;
    type Key = ();

    const NAME: &'static str = "throughput";
}

impl PolledSource for Readings {
    const INTERVAL: Duration = SAMPLE_INTERVAL;

    async fn poll(_key: ()) -> Result<Self, Infallible> {
        let uplink = match Connection::system().await {
            Ok(connection) => nm::uplink(connection).await.ok(),
            Err(_) => None,
        };
        let connectivity = uplink.as_ref().and_then(|uplink| uplink.connectivity);
        let interfaces = uplink
            .map(|uplink| physical(&uplink.activated_interfaces))
            .unwrap_or_default();
        let earlier = capture(&interfaces);
        sleep(SAMPLE_WINDOW).await;
        let current = capture(&interfaces);
        let rates = current.rates_since(&earlier);
        Ok(readings(&current, rates, rates, connectivity))
    }

    fn polls(_key: ()) -> impl Stream<Item = Delivery<Self>> + Send {
        let uplink_changes = nm_readings("throughput uplink", nm::UPLINK_PROPERTIES, nm::uplink)
            .map(ThroughputEvent::UplinkChanged);
        let ticks = ticked(Self::INTERVAL, || ready(ThroughputEvent::Tick));
        emit_delivery_changes(
            select(uplink_changes, ticks)
                .scan(ThroughputSampler::default(), |sampler, event| {
                    let sampled = matches!(event, ThroughputEvent::Tick);
                    *sampler = mem::take(sampler).observed(event);
                    ready(Some(sampled.then(|| sampler.readings()).flatten()))
                })
                .filter_map(ready)
                .map(|readings| Delivery::Reading(Ok(readings))),
        )
    }
}

enum ThroughputEvent {
    UplinkChanged(Option<nm::Uplink>),
    Tick,
}

#[derive(Default)]
struct ThroughputSampler {
    interfaces: Vec<String>,
    connectivity: Option<Connectivity>,
    latest: Option<Sample>,
    rates: Rates,
    peak: Rates,
}

impl ThroughputSampler {
    fn observed(self, event: ThroughputEvent) -> Self {
        match event {
            ThroughputEvent::UplinkChanged(uplink) => Self {
                interfaces: uplink
                    .as_ref()
                    .map(|uplink| physical(&uplink.activated_interfaces))
                    .unwrap_or_default(),
                connectivity: uplink.and_then(|uplink| uplink.connectivity),
                ..self
            },
            ThroughputEvent::Tick => {
                let current = capture(&self.interfaces);
                let rates = self
                    .latest
                    .as_ref()
                    .map_or_else(Rates::default, |previous| current.rates_since(previous));
                Self {
                    latest: Some(current),
                    rates,
                    peak: self.peak.raised_by(rates),
                    ..self
                }
            }
        }
    }

    fn readings(&self) -> Option<Readings> {
        self.latest
            .as_ref()
            .map(|latest| readings(latest, self.rates, self.peak, self.connectivity))
    }
}

impl Source for Throughput<Download> {
    type Error = Infallible;
    type Key = ();

    const NAME: &'static str = "download";
}

impl PolledSource for Throughput<Download> {
    const INTERVAL: Duration = SAMPLE_INTERVAL;

    async fn poll(_key: ()) -> Result<Self, Infallible> {
        Readings::poll(()).await.map(|readings| readings.down)
    }
}

impl Source for Throughput<Upload> {
    type Error = Infallible;
    type Key = ();

    const NAME: &'static str = "upload";
}

impl PolledSource for Throughput<Upload> {
    const INTERVAL: Duration = SAMPLE_INTERVAL;

    async fn poll(_key: ()) -> Result<Self, Infallible> {
        Readings::poll(()).await.map(|readings| readings.up)
    }
}

impl Source for Throughput<Total> {
    type Error = Infallible;
    type Key = ();

    const NAME: &'static str = "total throughput";
}

impl PolledSource for Throughput<Total> {
    const INTERVAL: Duration = SAMPLE_INTERVAL;

    async fn poll(_key: ()) -> Result<Self, Infallible> {
        Readings::poll(()).await.map(|readings| readings.total)
    }
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

#[derive(Default, Clone, Copy)]
struct Rates {
    rx: InformationRate,
    tx: InformationRate,
}

impl Rates {
    fn combine(
        self,
        other: Self,
        join: impl Fn(InformationRate, InformationRate) -> InformationRate,
    ) -> Self {
        Self {
            rx: join(self.rx, other.rx),
            tx: join(self.tx, other.tx),
        }
    }

    fn raised_by(self, other: Self) -> Self {
        self.combine(other, InformationRate::max)
    }

    fn both_directions(self) -> InformationRate {
        self.rx + self.tx
    }
}

impl Add for Rates {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        self.combine(other, Add::add)
    }
}

fn capture(interfaces: &[String]) -> Sample {
    Sample {
        counters: interfaces
            .iter()
            .map(|interface| (interface.clone(), read_counters(interface)))
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

fn readings(
    current: &Sample,
    rates: Rates,
    peak: Rates,
    connectivity: Option<Connectivity>,
) -> Readings {
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
            interfaces: interfaces.clone(),
            rate: rates.tx,
            peak_rate: peak.tx,
            connectivity,
            direction: PhantomData,
        },
        total: Throughput {
            interfaces,
            rate: rates.both_directions(),
            peak_rate: peak.both_directions(),
            connectivity,
            direction: PhantomData,
        },
    }
}

fn per_second(now: ByteSize, was: ByteSize, elapsed: Duration) -> InformationRate {
    let transferred = Information::new::<byte>(now.as_u64().saturating_sub(was.as_u64()) as f64);
    (transferred / Time::new::<second>(elapsed.as_secs_f64())).into()
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

fn physical(interfaces: &[String]) -> Vec<String> {
    interfaces
        .iter()
        .filter(|interface| {
            Path::new("/sys/class/net")
                .join(interface)
                .join("device")
                .exists()
        })
        .cloned()
        .collect()
}
