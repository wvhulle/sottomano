use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap},
    fs,
    ops::RangeInclusive,
    path::Path,
};

use uom::si::{
    f64::{Frequency, Information},
    information::{kibibyte, mebibyte},
};

use super::{CPU_FREQ, arm_cpuinfo_core_parts, intel_hybrid_core_designs};
use crate::system_protocol_clients::sysfs_attribute_reads::{read_kilohertz, read_trimmed};

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

    #[must_use]
    pub fn physical_cores(&self) -> &[PhysicalCore] {
        &self.physical_cores
    }

    #[must_use]
    pub fn chiplets(&self) -> &[Chiplet] {
        &self.chiplets
    }

    #[must_use]
    pub fn role(&self) -> CoreRole {
        self.role
    }
}

impl CpuTopology {
    #[must_use]
    pub fn from_core_groups(core_groups: Vec<CoreGroup>) -> Self {
        CpuTopology { core_groups }
    }

    #[must_use]
    pub fn core_groups(&self) -> &[CoreGroup] {
        &self.core_groups
    }

    #[must_use]
    pub fn detect() -> Self {
        let designs = CoreDesign::read_all();
        let online = read_trimmed(format!("{CPU_FREQ}/online")).unwrap_or_default();
        let cpus: Vec<LogicalCpu> = cpulist(&online)
            .map(|index| LogicalCpu::read(index, &designs))
            .collect();
        Self::from_logical_cpus(&cpus)
    }

    fn from_logical_cpus(cpus: &[LogicalCpu]) -> Self {
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
        if let Some(chiplet) = assigned.chiplet
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
    last_level_cache: Option<SharedCache>,
    peak_clock: Option<Frequency>,
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
            peak_clock: read_kilohertz(format!("{directory}/cpufreq/cpuinfo_max_freq")),
            thread_siblings: read_trimmed(format!("{directory}/topology/thread_siblings_list"))
                .map_or_else(|| vec![index], |list| cpulist(&list).collect()),
        }
    }
}

fn highest_peak_clock<'a>(cpus: impl Iterator<Item = &'a LogicalCpu>) -> Option<Frequency> {
    cpus.filter_map(|cpu| cpu.peak_clock).reduce(Frequency::max)
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

struct AssignedCpu<'a> {
    pub role: CoreRole,
    cpu: &'a LogicalCpu,
    chiplet: Option<Chiplet>,
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
                if cpu.last_level_cache.as_ref().is_some_and(|cache| {
                    !cache
                        .sharing_cpus
                        .iter()
                        .any(|sharing_cpu| intel_performance_cpus.contains(sharing_cpu))
                }) =>
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
            chiplet: None,
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
            chiplet: None,
        })
    });
    unranked.chain(ranked).collect()
}

struct ArmPartTier<'a> {
    part: &'a str,
    highest_capacity: u32,
    members: Vec<&'a LogicalCpu>,
}

impl<'a> ArmPartTier<'a> {
    fn ranked_by_capacity(cpus: &'a [LogicalCpu]) -> Vec<Self> {
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

fn roles_by_cache_domain(cpus: &[LogicalCpu]) -> Vec<AssignedCpu<'_>> {
    let Some(domains) = CacheDomain::group_every_cpu(cpus) else {
        return cpus
            .iter()
            .map(|cpu| AssignedCpu {
                role: CoreRole::AllCores,
                cpu,
                chiplet: None,
            })
            .collect();
    };
    let [first, rest @ ..] = domains.as_slice() else {
        return Vec::new();
    };
    if rest.is_empty() {
        return first.assigned(CoreRole::AllCores, 0);
    }
    let largest_size = rest
        .iter()
        .map(|domain| domain.cache.size)
        .fold(first.cache.size, Information::max);
    let layout = if domains
        .iter()
        .all(|domain| domain.cache.size == largest_size)
    {
        CacheDomainLayout::EqualComplexes
    } else {
        let peak_where_largest_is = |has_largest: bool| {
            highest_peak_clock(
                domains
                    .iter()
                    .filter(|domain| (domain.cache.size == largest_size) == has_largest)
                    .flat_map(|domain| domain.members.iter().copied()),
            )
        };
        if peak_where_largest_is(true).is_some_and(|largest| {
            peak_where_largest_is(false).is_some_and(|smaller| largest > smaller)
        }) {
            CacheDomainLayout::FullCoresBesideCompact { largest_size }
        } else {
            CacheDomainLayout::StackedCacheBesideHighClock { largest_size }
        }
    };
    domains
        .iter()
        .enumerate()
        .flat_map(|(position, domain)| domain.assigned(layout.role(position, domain), position))
        .collect()
}

struct CacheDomain<'a> {
    cache: &'a SharedCache,
    members: Vec<&'a LogicalCpu>,
}

impl<'a> CacheDomain<'a> {
    fn group_every_cpu(cpus: &'a [LogicalCpu]) -> Option<Vec<Self>> {
        cpus.iter()
            .try_fold(Vec::<Self>::new(), |mut domains, cpu| {
                let cache = cpu.last_level_cache.as_ref()?;
                match domains.iter_mut().find(|domain| domain.cache == cache) {
                    Some(domain) => domain.members.push(cpu),
                    None => domains.push(CacheDomain {
                        cache,
                        members: vec![cpu],
                    }),
                }
                Some(domains)
            })
    }

    fn assigned(&self, role: CoreRole, position: usize) -> Vec<AssignedCpu<'a>> {
        let chiplet = Chiplet {
            position,
            shared_cache: self.cache.size,
        };
        self.members
            .iter()
            .map(|cpu| AssignedCpu {
                role,
                cpu,
                chiplet: Some(chiplet),
            })
            .collect()
    }
}

enum CacheDomainLayout {
    EqualComplexes,
    StackedCacheBesideHighClock { largest_size: Information },
    FullCoresBesideCompact { largest_size: Information },
}

impl CacheDomainLayout {
    fn role(&self, position: usize, domain: &CacheDomain) -> CoreRole {
        match self {
            CacheDomainLayout::EqualComplexes => CoreRole::CoreComplex {
                ordinal: position + 1,
            },
            CacheDomainLayout::StackedCacheBesideHighClock { largest_size } => {
                if domain.cache.size == *largest_size {
                    CoreRole::StackedCacheCores
                } else {
                    CoreRole::HighClockCores
                }
            }
            CacheDomainLayout::FullCoresBesideCompact { largest_size } => {
                if domain.cache.size == *largest_size {
                    CoreRole::PerformanceCores
                } else {
                    CoreRole::CompactCores
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct SharedCache {
    level: u32,
    size: Information,
    sharing_cpus: Vec<u32>,
}

impl SharedCache {
    fn read_last_level(cpu_directory: &Path) -> Option<Self> {
        fs::read_dir(cpu_directory.join("cache"))
            .inspect_err(|listing_failure| {
                tracing::debug!(
                    "cpu topology: cannot list caches of {}: {listing_failure}",
                    cpu_directory.display()
                );
            })
            .ok()?
            .filter_map(|cache_entry| {
                cache_entry
                    .inspect_err(|entry_failure| {
                        tracing::debug!(
                            "cpu topology: skipping unreadable cache entry of {}: {entry_failure}",
                            cpu_directory.display()
                        );
                    })
                    .ok()
            })
            .filter_map(|index| Self::read(&index.path()))
            .max_by_key(|cache| cache.level)
    }

    fn read(index_directory: &Path) -> Option<Self> {
        Some(SharedCache {
            level: read_trimmed(index_directory.join("level"))?
                .parse()
                .inspect_err(|parse_failure| {
                    tracing::warn!(
                        "cpu topology: invalid cache level in {}: {parse_failure}",
                        index_directory.display()
                    );
                })
                .ok()?,
            size: parse_cache_size(&read_trimmed(index_directory.join("size"))?)?,
            sharing_cpus: cpulist(&read_trimmed(index_directory.join("shared_cpu_list"))?)
                .collect(),
        })
    }
}

fn parse_cache_size(text: &str) -> Option<Information> {
    let parsed_size = if let Some(kibibytes) = text.strip_suffix('K') {
        kibibytes.parse().map(Information::new::<kibibyte>)
    } else if let Some(mebibytes) = text.strip_suffix('M') {
        mebibytes.parse().map(Information::new::<mebibyte>)
    } else {
        tracing::warn!("cpu topology: cache size {text:?} lacks a K or M unit");
        return None;
    };
    parsed_size
        .inspect_err(|parse_failure| {
            tracing::warn!("cpu topology: invalid cache size {text:?}: {parse_failure}");
        })
        .ok()
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
mod tests {
    use uom::si::frequency::megahertz;

    use super::*;

    fn cache(level: u32, mebibytes: f64, sharing_cpus: RangeInclusive<u32>) -> SharedCache {
        SharedCache {
            level,
            size: Information::new::<mebibyte>(mebibytes),
            sharing_cpus: sharing_cpus.collect(),
        }
    }

    fn cpus(
        indices: RangeInclusive<u32>,
        design: &CoreDesign,
        last_level_cache: &SharedCache,
        peak_megahertz: f64,
    ) -> Vec<LogicalCpu> {
        indices
            .map(|index| LogicalCpu {
                index,
                design: design.clone(),
                last_level_cache: Some(last_level_cache.clone()),
                peak_clock: Some(Frequency::new::<megahertz>(peak_megahertz)),
                thread_siblings: vec![index],
            })
            .collect()
    }

    fn arm_part(part: &str, capacity: u32) -> CoreDesign {
        CoreDesign::ArmCorePart {
            part: part.to_owned(),
            capacity,
        }
    }

    fn roles(topology: &CpuTopology) -> Vec<(CoreRole, Vec<u32>)> {
        topology
            .core_groups()
            .iter()
            .map(|group| (group.role, group.logical_cpus().collect()))
            .collect()
    }

    #[test]
    fn cpulist_expands_ranges_and_single_cores() {
        assert_eq!(
            cpulist("0-3,8,10-11").collect::<Vec<_>>(),
            [0, 1, 2, 3, 8, 10, 11]
        );
    }

    #[test]
    fn intel_efficient_cores_sharing_the_l3_stay_efficient() {
        let l3 = cache(3, 30.0, 0..=7);
        let topology = CpuTopology::from_logical_cpus(
            &[
                cpus(0..=3, &CoreDesign::IntelPerformanceCore, &l3, 5200.0),
                cpus(4..=7, &CoreDesign::IntelEfficientCore, &l3, 3900.0),
            ]
            .concat(),
        );
        assert_eq!(
            roles(&topology),
            [
                (CoreRole::PerformanceCores, vec![0, 1, 2, 3]),
                (CoreRole::EfficientCores, vec![4, 5, 6, 7]),
            ]
        );
    }

    #[test]
    fn intel_efficient_cores_outside_the_l3_form_a_low_power_island() {
        let topology = CpuTopology::from_logical_cpus(
            &[
                cpus(
                    0..=3,
                    &CoreDesign::IntelPerformanceCore,
                    &cache(3, 12.0, 0..=3),
                    4800.0,
                ),
                cpus(
                    4..=7,
                    &CoreDesign::IntelEfficientCore,
                    &cache(2, 4.0, 4..=7),
                    3700.0,
                ),
            ]
            .concat(),
        );
        assert_eq!(
            roles(&topology),
            [
                (CoreRole::PerformanceCores, vec![0, 1, 2, 3]),
                (CoreRole::LowPowerIslandCores, vec![4, 5, 6, 7]),
            ]
        );
    }

    #[test]
    fn ryzen_with_one_complex_is_all_cores() {
        let topology = CpuTopology::from_logical_cpus(&cpus(
            0..=15,
            &CoreDesign::NotExposed,
            &cache(3, 32.0, 0..=15),
            5400.0,
        ));
        assert_eq!(roles(&topology), [(CoreRole::AllCores, (0..=15).collect())]);
    }

    #[test]
    fn ryzen_with_equal_chiplets_splits_into_numbered_complexes() {
        let first_chiplet = cache(3, 32.0, 0..=3);
        let second_chiplet = cache(3, 32.0, 4..=7);
        let topology = CpuTopology::from_logical_cpus(
            &[
                cpus(0..=3, &CoreDesign::NotExposed, &first_chiplet, 5700.0),
                cpus(4..=7, &CoreDesign::NotExposed, &second_chiplet, 5600.0),
            ]
            .concat(),
        );
        assert_eq!(
            roles(&topology),
            [
                (CoreRole::CoreComplex { ordinal: 1 }, vec![0, 1, 2, 3]),
                (CoreRole::CoreComplex { ordinal: 2 }, vec![4, 5, 6, 7]),
            ]
        );
    }

    #[test]
    fn ryzen_x3d_separates_the_stacked_cache_chiplet_from_the_high_clock_one() {
        let topology = CpuTopology::from_logical_cpus(
            &[
                cpus(
                    0..=3,
                    &CoreDesign::NotExposed,
                    &cache(3, 96.0, 0..=3),
                    5250.0,
                ),
                cpus(
                    4..=7,
                    &CoreDesign::NotExposed,
                    &cache(3, 32.0, 4..=7),
                    5750.0,
                ),
            ]
            .concat(),
        );
        assert_eq!(
            roles(&topology),
            [
                (CoreRole::StackedCacheCores, vec![0, 1, 2, 3]),
                (CoreRole::HighClockCores, vec![4, 5, 6, 7]),
            ]
        );
    }

    #[test]
    fn ryzen_x3d_reporting_equal_peak_clocks_still_has_a_stacked_cache_chiplet() {
        let topology = CpuTopology::from_logical_cpus(
            &[
                cpus(
                    0..=3,
                    &CoreDesign::NotExposed,
                    &cache(3, 96.0, 0..=3),
                    5760.0,
                ),
                cpus(
                    4..=7,
                    &CoreDesign::NotExposed,
                    &cache(3, 32.0, 4..=7),
                    5760.0,
                ),
            ]
            .concat(),
        );
        assert_eq!(
            roles(&topology),
            [
                (CoreRole::StackedCacheCores, vec![0, 1, 2, 3]),
                (CoreRole::HighClockCores, vec![4, 5, 6, 7]),
            ]
        );
    }

    #[test]
    fn ryzen_ai_separates_full_cores_from_compact_ones() {
        let topology = CpuTopology::from_logical_cpus(
            &[
                cpus(
                    0..=3,
                    &CoreDesign::NotExposed,
                    &cache(3, 16.0, 0..=3),
                    5100.0,
                ),
                cpus(
                    4..=11,
                    &CoreDesign::NotExposed,
                    &cache(3, 8.0, 4..=11),
                    3300.0,
                ),
            ]
            .concat(),
        );
        assert_eq!(
            roles(&topology),
            [
                (CoreRole::PerformanceCores, vec![0, 1, 2, 3]),
                (CoreRole::CompactCores, (4..=11).collect()),
            ]
        );
    }

    #[test]
    fn favored_cores_of_one_arm_part_stay_one_tier() {
        let l3 = cache(3, 2.0, 0..=3);
        let topology = CpuTopology::from_logical_cpus(
            &[
                cpus(0..=0, &arm_part("0xd81", 1005), &l3, 3000.0),
                cpus(1..=1, &arm_part("0xd81", 1024), &l3, 3100.0),
                cpus(2..=3, &arm_part("0xd80", 676), &l3, 2000.0),
            ]
            .concat(),
        );
        assert_eq!(
            roles(&topology),
            [
                (CoreRole::PerformanceCores, vec![0, 1]),
                (CoreRole::EfficientCores, vec![2, 3]),
            ]
        );
    }

    #[test]
    fn three_arm_parts_put_the_middle_one_in_mid_capacity() {
        let l3 = cache(3, 2.0, 0..=2);
        let topology = CpuTopology::from_logical_cpus(
            &[
                cpus(0..=0, &arm_part("0xd46", 400), &l3, 1800.0),
                cpus(1..=1, &arm_part("0xd47", 800), &l3, 2400.0),
                cpus(2..=2, &arm_part("0xd48", 1024), &l3, 3000.0),
            ]
            .concat(),
        );
        assert_eq!(
            roles(&topology),
            [
                (CoreRole::PerformanceCores, vec![2]),
                (CoreRole::MidCapacityCores, vec![1]),
                (CoreRole::EfficientCores, vec![0]),
            ]
        );
    }

    #[test]
    fn one_arm_part_is_all_cores() {
        let l3 = cache(2, 1.0, 0..=1);
        let topology =
            CpuTopology::from_logical_cpus(&cpus(0..=1, &arm_part("0xd05", 1024), &l3, 1800.0));
        assert_eq!(roles(&topology), [(CoreRole::AllCores, vec![0, 1])]);
    }

    #[test]
    fn cache_sizes_parse_kibibyte_and_mebibyte_suffixes() {
        assert_eq!(
            parse_cache_size("12288K"),
            Some(Information::new::<mebibyte>(12.0))
        );
        assert_eq!(
            parse_cache_size("4M"),
            Some(Information::new::<mebibyte>(4.0))
        );
    }
}
