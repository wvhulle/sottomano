use std::{fs, thread, time::Duration};

use tokio::task::spawn_blocking;
use uom::si::{f64::Ratio, ratio::ratio};

use super::{Error, cpu_clock::CpuClock, cpu_core_index};
use crate::data::source::Source;

const CPU_STAT: &str = "/proc/stat";
const SAMPLE_GAP: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreLoad {
    pub core: u32,
    pub busy: Ratio,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuUsage {
    pub busy: Ratio,
    pub cores: Vec<CoreLoad>,
    pub clock: CpuClock,
}

impl Source for CpuUsage {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "cpu";
    const INTERVAL: Duration = Duration::from_secs(3);

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        spawn_blocking(read_cpu_usage)
            .await
            .map_err(|_| Error::Panicked)?
    }
}

fn read_cpu_usage() -> Result<CpuUsage, Error> {
    let unreadable = || Error::Unreadable(CPU_STAT.to_owned());
    let sample = CpuStatSample::take().ok_or_else(unreadable)?;
    Ok(CpuUsage {
        busy: sample.busy_fraction().ok_or_else(unreadable)?,
        cores: sample.core_loads(),
        clock: CpuClock::current(),
    })
}

pub(crate) fn cpu_busy_frac() -> Option<f64> {
    Some(CpuStatSample::take()?.busy_fraction()?.get::<ratio>())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Jiffies {
    total: i64,
    idle: i64,
}

impl Jiffies {
    fn from_counters<'a>(counters: impl Iterator<Item = &'a str>) -> Option<Self> {
        let counters: Vec<i64> = counters.filter_map(|t| t.parse().ok()).collect();
        Some(Jiffies {
            total: counters.iter().sum(),
            idle: *counters.get(3)?,
        })
    }

    fn busy_fraction_since(&self, earlier: &Jiffies) -> Option<Ratio> {
        let total = self.total - earlier.total;
        let idle = self.idle - earlier.idle;
        (total > 0).then(|| {
            Ratio::new::<ratio>(((total - idle) as f64 / total as f64).clamp(0.0, 1.0))
        })
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
        Some(CoreLoad {
            core: self.core,
            busy: self.jiffies.busy_fraction_since(&before.jiffies)?,
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

    #[test]
    fn parses_overall_and_each_core() {
        let stat = CpuStat::parse(EARLIER).unwrap();
        assert_eq!(
            stat.overall,
            Jiffies {
                total: 1000,
                idle: 800
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
                    busy: Ratio::new::<ratio>(1.0)
                },
                CoreLoad {
                    core: 1,
                    busy: Ratio::new::<ratio>(0.0)
                },
            ]
        );
    }
}
