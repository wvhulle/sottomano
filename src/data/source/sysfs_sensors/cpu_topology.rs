use std::{cmp::Reverse, collections::HashMap, fs, ops::RangeInclusive};

use super::{CPU_FREQ, read_trimmed};

const INTEL_PERFORMANCE_PMU: &str = "/sys/devices/cpu_core/cpus";
const INTEL_EFFICIENT_PMU: &str = "/sys/devices/cpu_atom/cpus";
const CPU_INFO: &str = "/proc/cpuinfo";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoreKind {
    Performance,
    Balanced,
    Efficient,
    Uniform,
}

impl CoreKind {
    pub fn title(self) -> &'static str {
        match self {
            CoreKind::Performance => "Performance cores",
            CoreKind::Balanced => "Balanced cores",
            CoreKind::Efficient => "Efficient cores",
            CoreKind::Uniform => "Cores",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CpuTopology {
    kinds: HashMap<u32, CoreKind>,
}

impl CpuTopology {
    pub fn kind_of(&self, core: u32) -> CoreKind {
        self.kinds.get(&core).copied().unwrap_or(CoreKind::Uniform)
    }

    pub fn detect() -> Self {
        Self::intel_hybrid()
            .or_else(Self::arm_hybrid)
            .unwrap_or_default()
    }

    fn intel_hybrid() -> Option<Self> {
        let performance = read_trimmed(INTEL_PERFORMANCE_PMU)?;
        let efficient = read_trimmed(INTEL_EFFICIENT_PMU)?;
        Some(Self::from_intel_cpulists(&performance, &efficient))
    }

    fn from_intel_cpulists(performance: &str, efficient: &str) -> Self {
        let performance = cpulist(performance).map(|core| (core, CoreKind::Performance));
        let efficient = cpulist(efficient).map(|core| (core, CoreKind::Efficient));
        CpuTopology {
            kinds: performance.chain(efficient).collect(),
        }
    }

    fn arm_hybrid() -> Option<Self> {
        let cpuinfo = fs::read_to_string(CPU_INFO).ok()?;
        let cores: Vec<CoreMicroarchitecture> = CorePart::parse_all(&cpuinfo)
            .filter_map(CorePart::with_capacity)
            .collect();
        Self::from_microarchitectures(&cores)
    }

    fn from_microarchitectures(cores: &[CoreMicroarchitecture]) -> Option<Self> {
        let mut tiers: Vec<MicroarchitectureTier> = Vec::new();
        for core in cores {
            match tiers.iter_mut().find(|tier| tier.part == core.part) {
                Some(tier) => tier.absorb(core),
                None => tiers.push(MicroarchitectureTier::founded_by(core)),
            }
        }
        if tiers.len() < 2 {
            return None;
        }
        tiers.sort_by_key(|tier| Reverse(tier.capacity));
        let slowest = tiers.len() - 1;
        let kinds = tiers
            .iter()
            .enumerate()
            .flat_map(|(rank, tier)| {
                let kind = match rank {
                    0 => CoreKind::Performance,
                    rank if rank == slowest => CoreKind::Efficient,
                    _ => CoreKind::Balanced,
                };
                tier.cores.iter().map(move |core| (*core, kind))
            })
            .collect();
        Some(CpuTopology { kinds })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CorePart {
    core: u32,
    part: String,
}

impl CorePart {
    fn parse_all(cpuinfo: &str) -> impl Iterator<Item = CorePart> + '_ {
        cpuinfo.split("\n\n").filter_map(Self::parse)
    }

    fn parse(block: &str) -> Option<Self> {
        Some(CorePart {
            core: cpuinfo_field(block, "processor")?.parse().ok()?,
            part: cpuinfo_field(block, "CPU part")?.to_owned(),
        })
    }

    fn with_capacity(self) -> Option<CoreMicroarchitecture> {
        let capacity = read_trimmed(format!("{CPU_FREQ}/cpu{}/cpu_capacity", self.core))?;
        Some(CoreMicroarchitecture {
            core: self.core,
            part: self.part,
            capacity: capacity.parse().ok()?,
        })
    }
}

fn cpuinfo_field<'a>(block: &'a str, name: &str) -> Option<&'a str> {
    block.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == name).then(|| value.trim())
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoreMicroarchitecture {
    core: u32,
    part: String,
    capacity: u32,
}

struct MicroarchitectureTier {
    part: String,
    capacity: u32,
    cores: Vec<u32>,
}

impl MicroarchitectureTier {
    fn founded_by(core: &CoreMicroarchitecture) -> Self {
        MicroarchitectureTier {
            part: core.part.clone(),
            capacity: core.capacity,
            cores: vec![core.core],
        }
    }

    fn absorb(&mut self, core: &CoreMicroarchitecture) {
        self.capacity = self.capacity.max(core.capacity);
        self.cores.push(core.core);
    }
}

fn cpulist(list: &str) -> impl Iterator<Item = u32> + '_ {
    list.split(',').filter_map(cpu_range).flatten()
}

fn cpu_range(range: &str) -> Option<RangeInclusive<u32>> {
    let range = range.trim();
    if let Some((first, last)) = range.split_once('-') {
        return Some(first.parse().ok()?..=last.parse().ok()?);
    }
    let core = range.parse().ok()?;
    Some(core..=core)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(topology: &CpuTopology, cores: RangeInclusive<u32>) -> Vec<CoreKind> {
        cores.map(|core| topology.kind_of(core)).collect()
    }

    fn core(core: u32, part: &str, capacity: u32) -> CoreMicroarchitecture {
        CoreMicroarchitecture {
            core,
            part: part.to_owned(),
            capacity,
        }
    }

    #[test]
    fn cpulist_expands_ranges_and_single_cores() {
        assert_eq!(
            cpulist("0-3,8,10-11").collect::<Vec<_>>(),
            [0, 1, 2, 3, 8, 10, 11]
        );
    }

    #[test]
    fn intel_pmu_lists_split_performance_from_efficient() {
        let topology = CpuTopology::from_intel_cpulists("0-3", "4-7");
        assert_eq!(
            kinds(&topology, 0..=7),
            [
                CoreKind::Performance,
                CoreKind::Performance,
                CoreKind::Performance,
                CoreKind::Performance,
                CoreKind::Efficient,
                CoreKind::Efficient,
                CoreKind::Efficient,
                CoreKind::Efficient,
            ]
        );
    }

    #[test]
    fn unknown_cores_are_uniform() {
        assert_eq!(CpuTopology::default().kind_of(3), CoreKind::Uniform);
    }

    #[test]
    fn favored_cores_of_one_microarchitecture_stay_one_tier() {
        let topology = CpuTopology::from_microarchitectures(&[
            core(0, "0xd81", 1005),
            core(1, "0xd81", 1024),
            core(2, "0xd80", 676),
            core(3, "0xd80", 676),
        ])
        .unwrap();
        assert_eq!(
            kinds(&topology, 0..=3),
            [
                CoreKind::Performance,
                CoreKind::Performance,
                CoreKind::Efficient,
                CoreKind::Efficient,
            ]
        );
    }

    #[test]
    fn three_microarchitectures_put_the_middle_one_in_balanced() {
        let topology = CpuTopology::from_microarchitectures(&[
            core(0, "0xd46", 400),
            core(1, "0xd47", 800),
            core(2, "0xd48", 1024),
        ])
        .unwrap();
        assert_eq!(
            kinds(&topology, 0..=2),
            [
                CoreKind::Efficient,
                CoreKind::Balanced,
                CoreKind::Performance
            ]
        );
    }

    #[test]
    fn one_microarchitecture_is_not_hybrid() {
        assert_eq!(
            CpuTopology::from_microarchitectures(&[core(0, "0xd05", 1024), core(1, "0xd05", 1024)]),
            None
        );
    }

    #[test]
    fn cpuinfo_blocks_yield_each_cores_part() {
        let cpuinfo = "processor\t: 0\nCPU part\t: 0xd05\n\nprocessor\t: 1\nCPU part\t: 0xd0b\n";
        assert_eq!(
            CorePart::parse_all(cpuinfo).collect::<Vec<_>>(),
            [
                CorePart {
                    core: 0,
                    part: "0xd05".to_owned()
                },
                CorePart {
                    core: 1,
                    part: "0xd0b".to_owned()
                },
            ]
        );
    }
}
