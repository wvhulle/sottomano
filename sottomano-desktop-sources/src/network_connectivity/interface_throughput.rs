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
use futures::{Stream, StreamExt, stream::select};
use uom::si::{
    f64::{Information, InformationRate, Time},
    information::byte,
    information_rate::{kilobyte_per_second, megabyte_per_second},
    ratio::ratio,
    time::second,
};

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling, SnapshotDeliveryStreams},
    network_connectivity::{
        network_manager_proxies, network_manager_proxies::Metering, nm_readings,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

const SAMPLE_POLLING: PeriodicPolling = PeriodicPolling::every(Duration::from_secs(2));
const BACKGROUND_CHATTER_KB_PER_S: f64 = 10.0;
const SCALE_START_KB_PER_S: f64 = 100.0;
pub const SCALE_END_MB_PER_S: f64 = 125.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkTrafficDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Throughput {
    pub interfaces: Vec<String>,
    pub rate: InformationRate,
    pub peak_rate: InformationRate,
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
    pub fn link_usage(&self) -> LinkUsage {
        if self.peak_rate
            <= InformationRate::new::<kilobyte_per_second>(BACKGROUND_CHATTER_KB_PER_S)
        {
            LinkUsage::Idle
        } else {
            LinkUsage::Carrying
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceThroughput {
    pub down: Throughput,
    pub up: Throughput,
    pub metering: Metering,
    pub cumulative_counters: BTreeMap<String, InterfaceByteCounters>,
}

impl SnapshotSource for InterfaceThroughput {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Network throughput";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let uplink_changes = nm_readings(
            buses.system,
            network_manager_proxies::UPLINK_PROPERTIES,
            network_manager_proxies::uplink,
        )
        .map(|uplink| match uplink {
            Ok(uplink) => ThroughputEvent::UplinkChanged(uplink),
            Err(failure) => ThroughputEvent::UplinkUnreadable(failure),
        });
        let ticks = SAMPLE_POLLING.poll_ticks().map(ThroughputEvent::Tick);
        select(uplink_changes, ticks)
            .scan(ThroughputSampler::default(), |sampler, event| {
                let delivered = match event {
                    ThroughputEvent::UplinkUnreadable(failure) => {
                        Some(SnapshotDelivery::live_push(Err(failure)))
                    }
                    ThroughputEvent::Tick(polling) => {
                        *sampler = mem::take(sampler).observed(ThroughputEvent::Tick(polling));
                        sampler
                            .readings()
                            .map(|readings| SnapshotDelivery::SnapshotDelivered {
                                snapshot_fetch: Ok(readings),
                                delivery_cadence: DeliveryCadence::PeriodicPolling(polling),
                            })
                    }
                    ThroughputEvent::UplinkChanged(uplink) => {
                        *sampler =
                            mem::take(sampler).observed(ThroughputEvent::UplinkChanged(uplink));
                        None
                    }
                };
                ready(Some(delivered))
            })
            .filter_map(ready)
            .repeats_as_unchanged()
    }
}

enum ThroughputEvent {
    UplinkChanged(Option<network_manager_proxies::Uplink>),
    UplinkUnreadable(zbus::Error),
    Tick(PeriodicPolling),
}

#[derive(Default)]
struct ThroughputSampler {
    interfaces: Vec<String>,
    metering: Metering,
    latest: Option<Sample>,
    rates: Rates,
    peak: Rates,
}

impl ThroughputSampler {
    fn observed(self, event: ThroughputEvent) -> Self {
        match event {
            ThroughputEvent::UplinkChanged(Some(uplink)) => Self {
                interfaces: physical(&uplink.activated_interfaces),
                metering: uplink.metering,
                ..self
            },
            ThroughputEvent::UplinkChanged(None) => Self {
                interfaces: Vec::default(),
                metering: Metering::Unmetered,
                ..self
            },
            ThroughputEvent::Tick(_) => {
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

    fn readings(&self) -> Option<InterfaceThroughput> {
        self.latest
            .as_ref()
            .map(|latest| readings(latest, self.rates, self.peak, self.metering))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct InterfaceByteCounters {
    pub received: ByteSize,
    pub sent: ByteSize,
}

struct Sample {
    counters: BTreeMap<String, InterfaceByteCounters>,
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
        let elapsed = self
            .at
            .checked_duration_since(previous.at)
            .expect("samples are taken in order");
        if elapsed.as_millis() == 0 {
            return Rates::default();
        }
        self.counters
            .iter()
            .filter_map(|(interface, now)| {
                Some((interface, now, previous.counters.get(interface)?))
            })
            .filter_map(|(interface, now, was)| {
                let interface_rates = per_second(now.received, was.received, elapsed)
                    .zip(per_second(now.sent, was.sent, elapsed))
                    .map(|(rx, tx)| Rates { rx, tx });
                if interface_rates.is_none() {
                    tracing::debug!(
                        "interface throughput: {interface} byte counters went back after a reset, \
                         skipping this sample"
                    );
                }
                interface_rates
            })
            .fold(Rates::default(), Add::add)
    }
}

fn readings(
    current: &Sample,
    rates: Rates,
    peak: Rates,
    metering: Metering,
) -> InterfaceThroughput {
    let interfaces: Vec<String> = current.counters.keys().cloned().collect();
    InterfaceThroughput {
        down: Throughput {
            interfaces: interfaces.clone(),
            rate: rates.rx,
            peak_rate: peak.rx,
            direction: NetworkTrafficDirection::Download,
        },
        up: Throughput {
            interfaces,
            rate: rates.tx,
            peak_rate: peak.tx,
            direction: NetworkTrafficDirection::Upload,
        },
        metering,
        cumulative_counters: current.counters.clone(),
    }
}

fn per_second(now: ByteSize, was: ByteSize, elapsed: Duration) -> Option<InformationRate> {
    let transferred_bytes = now.as_u64().checked_sub(was.as_u64())?;
    let transferred = Information::new::<byte>(transferred_bytes as f64);
    Some((transferred / Time::new::<second>(elapsed.as_secs_f64())).into())
}

fn read_counters(interface: &str) -> InterfaceByteCounters {
    let counter = |file: &str| {
        let counter_path = format!("/sys/class/net/{interface}/statistics/{file}");
        match fs::read_to_string(&counter_path)
            .map(|counter_text| counter_text.trim().parse::<u64>())
        {
            Ok(Ok(byte_count)) => ByteSize::b(byte_count),
            Ok(Err(parse_failure)) => {
                tracing::warn!(
                    "interface throughput: invalid counter in {counter_path}: {parse_failure}"
                );
                ByteSize::default()
            }
            Err(read_failure) => {
                tracing::debug!("interface throughput: cannot read {counter_path}: {read_failure}");
                ByteSize::default()
            }
        }
    };
    InterfaceByteCounters {
        received: counter("rx_bytes"),
        sent: counter("tx_bytes"),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkUsage {
    Idle,
    Carrying,
}
