use std::{fs, thread, time::Duration};

use futures::Stream;
use uom::si::{f64::Ratio, ratio::ratio};

use super::{
    cpu_core_deep_idle::{CoreDeepIdle, IdleSnapshot},
    cpu_core_index,
    cpu_topology::CpuTopology,
};
use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, blocking_background_work::blocking_poll,
    polled_delivery_after_setup, polled_delivery_stream,
    system_protocol_clients::sysfs_attribute_reads::Error,
};

const CPU_STAT: &str = "/proc/stat";
const SAMPLE_GAP: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreLoad {
    pub core: u32,
    pub busy: Ratio,
    pub split: LoadSplit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuUsage {
    pub busy: Ratio,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuCoreLoads {
    pub cores: Vec<CoreLoad>,
    pub deep_idle: Vec<CoreDeepIdle>,
    pub topology: CpuTopology,
}

impl ReadingSource for CpuUsage {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "cpu";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(3));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl CpuUsage {
    async fn poll(_key: ()) -> Result<Self, Error> {
        blocking_poll(read_cpu_usage).await
    }
}

impl ReadingSource for CpuCoreLoads {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "cpu cores";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(3));

    fn delivery_stream((): ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_after_setup(blocking_poll(|| Ok(CpuTopology::detect())), Self::poll)
    }
}

impl CpuCoreLoads {
    async fn poll(topology: CpuTopology) -> Result<Self, Error> {
        blocking_poll(move || read_core_loads(topology)).await
    }
}

fn unreadable() -> Error {
    Error::Unreadable(CPU_STAT.to_owned())
}

fn read_cpu_usage() -> Result<CpuUsage, Error> {
    let sample = CpuStatSample::take().ok_or_else(unreadable)?;
    Ok(CpuUsage {
        busy: sample.busy_fraction().ok_or_else(unreadable)?,
    })
}

fn read_core_loads(topology: CpuTopology) -> Result<CpuCoreLoads, Error> {
    let earlier_idle = IdleSnapshot::read();
    let sample = CpuStatSample::take().ok_or_else(unreadable)?;
    let later_idle = IdleSnapshot::read();
    Ok(CpuCoreLoads {
        cores: sample.core_loads(),
        deep_idle: later_idle.deep_idle_since(&earlier_idle),
        topology,
    })
}

#[must_use]
pub fn cpu_busy_frac() -> Option<f64> {
    Some(CpuStatSample::take()?.busy_fraction()?.get::<ratio>())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoadSplit {
    pub user: Ratio,
    pub system: Ratio,
    pub iowait: Ratio,
    pub irq: Ratio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Jiffies {
    user: i64,
    system: i64,
    idle: i64,
    iowait: i64,
    irq: i64,
    total: i64,
}

impl Jiffies {
    fn from_counters<'a>(counters: impl Iterator<Item = &'a str>) -> Option<Self> {
        let counters: Vec<i64> = counters.filter_map(|t| t.parse().ok()).collect();
        let counter = |position: usize| counters.get(position).copied();
        Some(Jiffies {
            user: counter(0)? + counter(1)?,
            system: counter(2)?,
            idle: counter(3)?,
            iowait: counter(4)?,
            irq: counter(5)? + counter(6)?,
            total: counters.iter().sum(),
        })
    }

    fn elapsed_since(&self, earlier: &Jiffies) -> Option<Jiffies> {
        let elapsed = Jiffies {
            user: self.user - earlier.user,
            system: self.system - earlier.system,
            idle: self.idle - earlier.idle,
            iowait: self.iowait - earlier.iowait,
            irq: self.irq - earlier.irq,
            total: self.total - earlier.total,
        };
        (elapsed.total > 0).then_some(elapsed)
    }

    fn share(&self, part: i64) -> Ratio {
        Ratio::new::<ratio>((part as f64 / self.total as f64).clamp(0.0, 1.0))
    }

    fn busy(&self) -> Ratio {
        self.share(self.total - self.idle)
    }

    fn split(&self) -> LoadSplit {
        LoadSplit {
            user: self.share(self.user),
            system: self.share(self.system),
            iowait: self.share(self.iowait),
            irq: self.share(self.irq),
        }
    }

    fn busy_fraction_since(&self, earlier: &Jiffies) -> Option<Ratio> {
        Some(self.elapsed_since(earlier)?.busy())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CoreJiffies {
    core: u32,
    jiffies: Jiffies,
}

impl CoreJiffies {
    fn parse(line: &str) -> Option<Self> {
        let mut tokens = line.split_whitespace();
        let core = cpu_core_index(tokens.next()?)?;
        Some(CoreJiffies {
            core,
            jiffies: Jiffies::from_counters(tokens)?,
        })
    }

    fn load_since(&self, earlier: &CpuStat) -> Option<CoreLoad> {
        let before = earlier.cores.iter().find(|c| c.core == self.core)?;
        let elapsed = self.jiffies.elapsed_since(&before.jiffies)?;
        Some(CoreLoad {
            core: self.core,
            busy: elapsed.busy(),
            split: elapsed.split(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CpuStat {
    overall: Jiffies,
    cores: Vec<CoreJiffies>,
}

impl CpuStat {
    fn read() -> Option<Self> {
        Self::parse(&fs::read_to_string(CPU_STAT).ok()?)
    }

    fn parse(content: &str) -> Option<Self> {
        let mut overall = content.lines().next()?.split_whitespace();
        overall.next().filter(|label| *label == "cpu")?;
        Some(CpuStat {
            overall: Jiffies::from_counters(overall)?,
            cores: content.lines().filter_map(CoreJiffies::parse).collect(),
        })
    }
}

struct CpuStatSample {
    earlier: CpuStat,
    later: CpuStat,
}

impl CpuStatSample {
    fn take() -> Option<Self> {
        let earlier = CpuStat::read()?;
        thread::sleep(SAMPLE_GAP);
        let later = CpuStat::read()?;
        Some(CpuStatSample { earlier, later })
    }

    fn busy_fraction(&self) -> Option<Ratio> {
        self.later
            .overall
            .busy_fraction_since(&self.earlier.overall)
    }

    fn core_loads(&self) -> Vec<CoreLoad> {
        self.later
            .cores
            .iter()
            .filter_map(|core| core.load_since(&self.earlier))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EARLIER: &str = "\
cpu  100 0 100 800 0 0 0 0 0 0
cpu0 50 0 50 400 0 0 0 0 0 0
cpu1 50 0 50 400 0 0 0 0 0 0
intr 12345
";

    const LATER: &str = "\
cpu  190 0 110 900 0 0 0 0 0 0
cpu0 140 0 60 400 0 0 0 0 0 0
cpu1 50 0 50 500 0 0 0 0 0 0
intr 12400
";

    fn sample() -> CpuStatSample {
        CpuStatSample {
            earlier: CpuStat::parse(EARLIER).unwrap(),
            later: CpuStat::parse(LATER).unwrap(),
        }
    }

    fn split(user: f64, system: f64) -> LoadSplit {
        LoadSplit {
            user: Ratio::new::<ratio>(user),
            system: Ratio::new::<ratio>(system),
            iowait: Ratio::new::<ratio>(0.0),
            irq: Ratio::new::<ratio>(0.0),
        }
    }

    #[test]
    fn parses_overall_and_each_core() {
        let stat = CpuStat::parse(EARLIER).unwrap();
        assert_eq!(
            stat.overall,
            Jiffies {
                user: 100,
                system: 100,
                idle: 800,
                iowait: 0,
                irq: 0,
                total: 1000,
            }
        );
        assert_eq!(
            stat.cores.iter().map(|c| c.core).collect::<Vec<_>>(),
            [0, 1]
        );
    }

    #[test]
    fn overall_busy_fraction_spans_all_cores() {
        assert_eq!(sample().busy_fraction(), Some(Ratio::new::<ratio>(0.5)));
    }

    #[test]
    fn core_loads_follow_each_cores_own_counters() {
        assert_eq!(
            sample().core_loads(),
            [
                CoreLoad {
                    core: 0,
                    busy: Ratio::new::<ratio>(1.0),
                    split: split(0.9, 0.1),
                },
                CoreLoad {
                    core: 1,
                    busy: Ratio::new::<ratio>(0.0),
                    split: split(0.0, 0.0),
                },
            ]
        );
    }

    #[test]
    fn iowait_and_irq_are_split_out_of_busy_time() {
        let earlier = Jiffies::from_counters("0 0 0 0 0 0 0".split(' ')).unwrap();
        let later = Jiffies::from_counters("10 0 10 50 20 5 5".split(' ')).unwrap();
        let elapsed = later.elapsed_since(&earlier).unwrap();
        assert_eq!(elapsed.busy(), Ratio::new::<ratio>(0.5));
        assert_eq!(
            elapsed.split(),
            LoadSplit {
                user: Ratio::new::<ratio>(0.1),
                system: Ratio::new::<ratio>(0.1),
                iowait: Ratio::new::<ratio>(0.2),
                irq: Ratio::new::<ratio>(0.1),
            }
        );
    }
}
