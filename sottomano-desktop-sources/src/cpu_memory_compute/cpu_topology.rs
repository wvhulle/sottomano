use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap},
    ops::RangeInclusive,
    path::Path,
};

use cache_domain_roles::roles_by_cache_domain;
use sysfs_shared_caches::{LastLevelCache, SharedCache};
use uom::si::f64::{Frequency, Information};

use super::{CPU_FREQ, arm_cpuinfo_core_parts, intel_hybrid_core_designs};
use crate::system_protocol_clients::sysfs_attribute_reads::{read_kilohertz, read_trimmed};

mod cache_domain_roles;
mod sysfs_shared_caches;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CoreRole {
    PerformanceCores,
    MidCapacityCores,
    EfficientCores,
    LowPowerIslandCores,
    StackedCacheCores,
    HighClockCores,
    CompactCores,
    CoreComplex { ordinal: usize },
    AllCores,
}

impl CoreRole {
    #[must_use]
    pub fn title(self) -> String {
        match self {
            CoreRole::PerformanceCores => "Performance cores".to_owned(),
            CoreRole::MidCapacityCores => "Mid-capacity cores".to_owned(),
            CoreRole::EfficientCores => "Efficient cores".to_owned(),
            CoreRole::LowPowerIslandCores => "Low-power efficient cores".to_owned(),
            CoreRole::StackedCacheCores => "Stacked-cache cores".to_owned(),
            CoreRole::HighClockCores => "High-clock cores".to_owned(),
            CoreRole::CompactCores => "Compact cores".to_owned(),
            CoreRole::CoreComplex { ordinal } => format!("Core complex {ordinal}"),
            CoreRole::AllCores => "Cores".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CpuTopology {
    pub core_groups: Vec<CoreGroup>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoreGroup {
    pub role: CoreRole,
    pub physical_cores: Vec<PhysicalCore>,
    pub chiplets: Vec<Chiplet>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalCore {
    pub threads: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Chiplet {
    pub position: usize,
    pub shared_cache: Information,
}

impl CoreGroup {
    #[must_use]
    pub fn new(role: CoreRole, physical_cores: Vec<PhysicalCore>, chiplets: Vec<Chiplet>) -> Self {
        CoreGroup {
            role,
            physical_cores,
            chiplets,
        }
        .ordered()
    }

    #[must_use]
    pub fn contains(&self, logical_cpu: u32) -> bool {
        self.logical_cpus().any(|cpu| cpu == logical_cpu)
    }

    pub fn logical_cpus(&self) -> impl Iterator<Item = u32> + '_ {
        self.physical_cores
            .iter()
            .flat_map(|core| core.threads.iter().copied())
    }
}

impl CpuTopology {
    #[must_use]
    pub fn detect() -> Self {
        let designs = CoreDesign::read_all();
        let online = read_trimmed(format!("{CPU_FREQ}/online")).unwrap_or_default();
        let cpus: Vec<LogicalCpu> = cpulist(&online)
            .map(|index| LogicalCpu::read(index, &designs))
            .collect();
        Self::from(cpus.as_slice())
    }
}

impl From<Vec<CoreGroup>> for CpuTopology {
    fn from(core_groups: Vec<CoreGroup>) -> Self {
        CpuTopology { core_groups }
    }
}

impl From<&[LogicalCpu]> for CpuTopology {
    fn from(cpus: &[LogicalCpu]) -> Self {
        let assigned = if design_spread(cpus) == DesignSpread::Mixed {
            roles_by_exposed_design(cpus)
        } else {
            roles_by_cache_domain(cpus)
        };
        let groups_by_role = assigned.into_iter().fold(
            BTreeMap::<CoreRole, CoreGroup>::new(),
            |mut groups_by_role, assigned| {
                groups_by_role
                    .entry(assigned.role)
                    .or_insert_with(|| CoreGroup {
                        role: assigned.role,
                        physical_cores: Vec::new(),
                        chiplets: Vec::new(),
                    })
                    .admit(&assigned);
                groups_by_role
            },
        );
        CpuTopology {
            core_groups: groups_by_role
                .into_values()
                .map(CoreGroup::ordered)
                .collect(),
        }
    }
}

impl CoreGroup {
    fn admit(&mut self, assigned: &AssignedCpu) {
        let cpu = assigned.cpu;
        match self.physical_cores.iter_mut().find(|core| {
            core.threads
                .iter()
                .any(|thread| cpu.thread_siblings.contains(thread))
        }) {
            Some(core) => core.threads.push(cpu.index),
            None => self.physical_cores.push(PhysicalCore {
                threads: vec![cpu.index],
            }),
        }
        if let ChipletPlacement::OnChiplet(chiplet) = assigned.chiplet
            && !self.chiplets.contains(&chiplet)
        {
            self.chiplets.push(chiplet);
        }
    }

    fn ordered(mut self) -> Self {
        for core in &mut self.physical_cores {
            core.threads.sort_unstable();
        }
        self.physical_cores
            .sort_by(|first, second| first.threads.cmp(&second.threads));
        self.chiplets.sort_by_key(|chiplet| chiplet.position);
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
struct LogicalCpu {
    index: u32,
    design: CoreDesign,
    last_level_cache: LastLevelCache,
    peak_clock: PeakClock,
    thread_siblings: Vec<u32>,
}

impl LogicalCpu {
    fn read(index: u32, designs: &HashMap<u32, CoreDesign>) -> Self {
        let directory = format!("{CPU_FREQ}/cpu{index}");
        LogicalCpu {
            index,
            design: designs
                .get(&index)
                .cloned()
                .unwrap_or(CoreDesign::NotExposed),
            last_level_cache: SharedCache::read_last_level(Path::new(&directory)),
            peak_clock: match read_kilohertz(format!("{directory}/cpufreq/cpuinfo_max_freq")) {
                Some(frequency) => PeakClock::Reported(frequency),
                None => PeakClock::NotExposed,
            },
            thread_siblings: read_trimmed(format!("{directory}/topology/thread_siblings_list"))
                .map_or_else(|| vec![index], |list| cpulist(&list).collect()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PeakClock {
    NotExposed,
    Reported(Frequency),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ChipletPlacement {
    OffChiplet,
    OnChiplet(Chiplet),
}

fn highest_peak_clock<'cpus>(cpus: impl Iterator<Item = &'cpus LogicalCpu>) -> Option<Frequency> {
    cpus.filter_map(|cpu| match cpu.peak_clock {
        PeakClock::Reported(frequency) => Some(frequency),
        PeakClock::NotExposed => None,
    })
    .reduce(Frequency::max)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CoreDesign {
    IntelPerformanceCore,
    IntelEfficientCore,
    ArmCorePart { part: String, capacity: u32 },
    NotExposed,
}

impl CoreDesign {
    fn read_all() -> HashMap<u32, CoreDesign> {
        intel_hybrid_core_designs::read()
            .or_else(arm_cpuinfo_core_parts::read)
            .unwrap_or_default()
    }

    fn arm_part(&self) -> Option<&str> {
        match self {
            CoreDesign::ArmCorePart { part, .. } => Some(part),
            _ => None,
        }
    }
}

struct AssignedCpu<'cpus> {
    pub role: CoreRole,
    cpu: &'cpus LogicalCpu,
    chiplet: ChipletPlacement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesignSpread {
    Mixed,
    Uniform,
}

fn design_spread(cpus: &[LogicalCpu]) -> DesignSpread {
    let all_exposed = cpus.iter().all(|cpu| cpu.design != CoreDesign::NotExposed);
    let first_design = cpus.first().map(|cpu| &cpu.design);
    let mixed = first_design.is_some_and(|first| {
        cpus.iter().any(|cpu| match cpu.design.arm_part() {
            Some(part) => first.arm_part() != Some(part),
            None => cpu.design != *first,
        })
    });
    if all_exposed && mixed {
        DesignSpread::Mixed
    } else {
        DesignSpread::Uniform
    }
}

fn roles_by_exposed_design(cpus: &[LogicalCpu]) -> Vec<AssignedCpu<'_>> {
    let intel_performance_cpus: Vec<u32> = cpus
        .iter()
        .filter(|cpu| cpu.design == CoreDesign::IntelPerformanceCore)
        .map(|cpu| cpu.index)
        .collect();
    let unranked = cpus.iter().filter_map(|cpu| {
        let role = match &cpu.design {
            CoreDesign::IntelPerformanceCore => CoreRole::PerformanceCores,
            CoreDesign::IntelEfficientCore
                if matches!(&cpu.last_level_cache, LastLevelCache::Shared(cache) if !cache
                        .sharing_cpus
                        .iter()
                        .any(|sharing_cpu| intel_performance_cpus.contains(sharing_cpu))) =>
            {
                CoreRole::LowPowerIslandCores
            }
            CoreDesign::IntelEfficientCore => CoreRole::EfficientCores,
            CoreDesign::NotExposed => CoreRole::AllCores,
            CoreDesign::ArmCorePart { .. } => return None,
        };
        Some(AssignedCpu {
            role,
            cpu,
            chiplet: ChipletPlacement::OffChiplet,
        })
    });
    let arm_tiers = ArmPartTier::ranked_by_capacity(cpus);
    let slowest_rank = arm_tiers.len().checked_sub(1);
    let ranked = arm_tiers.into_iter().enumerate().flat_map(|(rank, tier)| {
        let role = match rank {
            0 => CoreRole::PerformanceCores,
            rank if Some(rank) == slowest_rank => CoreRole::EfficientCores,
            _ => CoreRole::MidCapacityCores,
        };
        tier.members.into_iter().map(move |cpu| AssignedCpu {
            role,
            cpu,
            chiplet: ChipletPlacement::OffChiplet,
        })
    });
    unranked.chain(ranked).collect()
}

struct ArmPartTier<'cpus> {
    part: &'cpus str,
    highest_capacity: u32,
    members: Vec<&'cpus LogicalCpu>,
}

impl<'cpus> ArmPartTier<'cpus> {
    fn ranked_by_capacity(cpus: &'cpus [LogicalCpu]) -> Vec<Self> {
        let mut tiers = cpus.iter().fold(Vec::<Self>::new(), |mut tiers, cpu| {
            if let CoreDesign::ArmCorePart { part, capacity } = &cpu.design {
                match tiers.iter_mut().find(|tier| tier.part == part) {
                    Some(tier) => {
                        tier.highest_capacity = tier.highest_capacity.max(*capacity);
                        tier.members.push(cpu);
                    }
                    None => tiers.push(ArmPartTier {
                        part,
                        highest_capacity: *capacity,
                        members: vec![cpu],
                    }),
                }
            }
            tiers
        });
        tiers.sort_by_key(|tier| Reverse(tier.highest_capacity));
        tiers
    }
}

pub(super) fn cpulist(list: &str) -> impl Iterator<Item = u32> + '_ {
    list.split(',').filter_map(cpu_range).flatten()
}

fn cpu_range(range: &str) -> Option<RangeInclusive<u32>> {
    let range = range.trim();
    if range.is_empty() {
        return None;
    }
    let parse_core = |core_text: &str| {
        core_text
            .parse::<u32>()
            .inspect_err(|parse_failure| {
                tracing::warn!(
                    "cpu topology: invalid core {core_text:?} in cpu list range {range:?}: \
                     {parse_failure}"
                );
            })
            .ok()
    };
    if let Some((first, last)) = range.split_once('-') {
        return Some(parse_core(first)?..=parse_core(last)?);
    }
    let core = parse_core(range)?;
    Some(core..=core)
}

#[cfg(test)]
mod cpu_topology_tests;
