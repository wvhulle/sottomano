use std::{
    collections::BTreeMap,
    fs,
    future::ready,
    marker::PhantomData,
    ops::Add,
    path::Path,
    time::{Duration, Instant},
};

use bytesize::ByteSize;
use iced::{
    Subscription,
    futures::{Stream, StreamExt, stream::select},
};
use tokio::time::{MissedTickBehavior, interval};
use tokio_stream::wrappers::IntervalStream;
use uom::si::{
    f64::{Information, InformationRate, Time},
    information::byte,
    information_rate::{kilobyte_per_second, megabyte_per_second},
    ratio::ratio,
    time::second,
};

use crate::data::source::net_interfaces::{
    nm::{self, Connectivity, Uplink},
    nm_readings,
};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const BACKGROUND_CHATTER_KB_PER_S: f64 = 10.0;
const SCALE_START_KB_PER_S: f64 = 100.0;
pub const SCALE_END_MB_PER_S: f64 = 125.0;

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
pub struct Total;

impl Direction for Total {
    const LABEL: &'static str = "Total";
}

#[derive(Debug, Clone)]
pub struct Throughput<D: Direction> {
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

impl<D: Direction> Throughput<D> {
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

#[derive(Debug, Clone)]
pub struct Readings {
    pub down: Throughput<Download>,
    pub up: Throughput<Upload>,
    pub total: Throughput<Total>,
}

pub fn subscription() -> Subscription<Readings> {
    Subscription::run(readings_over_time)
}

fn readings_over_time() -> impl Stream<Item = Readings> {
    let relinks = nm_readings("throughput", nm::uplink)
        .filter_map(ready)
        .map(Pulse::Relink);
    let mut ticks = interval(SAMPLE_INTERVAL);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let elapsed = IntervalStream::new(ticks).map(|_| Pulse::Elapsed);
    select(relinks, elapsed)
        .scan(Session::default(), |session, pulse| {
            ready(Some(session.advance(pulse)))
        })
        .filter_map(ready)
}

enum Pulse {
    Relink(Uplink),
    Elapsed,
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
    interfaces: Vec<String>,
    connectivity: Option<Connectivity>,
    previous: Option<Sample>,
    peak: Rates,
}

impl Session {
    fn advance(&mut self, pulse: Pulse) -> Option<Readings> {
        match pulse {
            Pulse::Relink(uplink) => {
                self.relink(uplink);
                Some(self.sample())
            }
            Pulse::Elapsed => self.is_carrying().then(|| self.sample()),
        }
    }

    fn relink(&mut self, uplink: Uplink) {
        self.interfaces = uplink
            .activated_interfaces
            .into_iter()
            .filter(|interface| is_physical(interface))
            .collect();
        self.connectivity = uplink.connectivity;
    }

    fn is_carrying(&self) -> bool {
        !self.interfaces.is_empty()
    }

    fn sample(&mut self) -> Readings {
        let current = capture(&self.interfaces);
        let rates = self
            .previous
            .as_ref()
            .map_or_else(Rates::default, |previous| current.rates_since(previous));
        self.peak = self.peak.raised_by(rates);
        let readings = readings(&current, rates, self.peak, self.connectivity);
        self.previous = Some(current);
        readings
    }
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

fn is_physical(interface: &str) -> bool {
    Path::new("/sys/class/net")
        .join(interface)
        .join("device")
        .exists()
}
