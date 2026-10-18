use std::{
    collections::BTreeMap,
    fs,
    future::ready,
    mem,
    ops::Add,
    path::Path,
    time::{Duration, Instant},
};

use bytesize::ByteSize;
use iced::futures::{Stream, StreamExt, stream::select};
use uom::si::{
    f64::{Information, InformationRate, Time},
    information::byte,
    information_rate::{kilobyte_per_second, megabyte_per_second},
    ratio::ratio,
    time::second,
};

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    bus::DesktopBuses,
    net_interfaces::{
        nm::{self, Connectivity},
        nm_readings,
    },
    stream_operators::{StreamOperators, ticked},
};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(2);
const BACKGROUND_CHATTER_KB_PER_S: f64 = 10.0;
const SCALE_START_KB_PER_S: f64 = 100.0;
pub const SCALE_END_MB_PER_S: f64 = 125.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkTrafficDirection {
    Upload,
    Download,
    Combined,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Throughput {
    pub interfaces: Vec<String>,
    pub rate: InformationRate,
    pub peak_rate: InformationRate,
    pub connectivity: Option<Connectivity>,
    pub direction: NetworkTrafficDirection,
}

#[allow(clippy::cast_possible_truncation)]
#[must_use]
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

impl Throughput {
    fn fraction_of_carried(&self, rate: InformationRate) -> f32 {
        if self.interfaces.is_empty() {
            return 0.0;
        }
        fraction_of_scale(rate)
    }

    #[must_use]
    pub fn current_fraction(&self) -> f32 {
        self.fraction_of_carried(self.rate)
    }

    #[must_use]
    pub fn peak_fraction(&self) -> f32 {
        self.fraction_of_carried(self.peak_rate)
    }

    #[must_use]
    pub fn unused(&self) -> bool {
        self.peak_rate <= InformationRate::new::<kilobyte_per_second>(BACKGROUND_CHATTER_KB_PER_S)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Readings {
    pub down: Throughput,
    pub up: Throughput,
    pub total: Throughput,
}

impl ReadingSource for Readings {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "throughput";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(SAMPLE_INTERVAL);

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let uplink_changes = nm_readings(buses.system, nm::UPLINK_PROPERTIES, nm::uplink).map(
            |uplink| match uplink {
                Ok(uplink) => ThroughputEvent::UplinkChanged(uplink),
                Err(failure) => ThroughputEvent::UplinkUnreadable(failure),
            },
        );
        let ticks = ticked(SAMPLE_INTERVAL, || ready(ThroughputEvent::Tick));
        select(uplink_changes, ticks)
            .scan(ThroughputSampler::default(), |sampler, event| {
                let delivered = match event {
                    ThroughputEvent::UplinkUnreadable(failure) => {
                        Some(SourceDelivery::Reading(Err(failure)))
                    }
                    sampling => {
                        let sampled = matches!(sampling, ThroughputEvent::Tick);
                        *sampler = mem::take(sampler).observed(sampling);
                        sampled
                            .then(|| sampler.readings())
                            .flatten()
                            .map(|readings| SourceDelivery::Reading(Ok(readings)))
                    }
                };
                ready(Some(delivered))
            })
            .filter_map(ready)
            .distinct_until_changed()
    }
}

enum ThroughputEvent {
    UplinkChanged(Option<nm::Uplink>),
    UplinkUnreadable(zbus::Error),
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
            ThroughputEvent::UplinkUnreadable(_) => self,
        }
    }

    fn readings(&self) -> Option<Readings> {
        self.latest
            .as_ref()
            .map(|latest| readings(latest, self.rates, self.peak, self.connectivity))
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
            direction: NetworkTrafficDirection::Download,
        },
        up: Throughput {
            interfaces: interfaces.clone(),
            rate: rates.tx,
            peak_rate: peak.tx,
            connectivity,
            direction: NetworkTrafficDirection::Upload,
        },
        total: Throughput {
            interfaces,
            rate: rates.both_directions(),
            peak_rate: peak.both_directions(),
            connectivity,
            direction: NetworkTrafficDirection::Combined,
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
