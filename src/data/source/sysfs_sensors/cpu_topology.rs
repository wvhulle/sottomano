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

use super::{CPU_FREQ, read_kilohertz, read_trimmed};

const INTEL_PERFORMANCE_PMU: &str = "/sys/devices/cpu_core/cpus";
const INTEL_EFFICIENT_PMU: &str = "/sys/devices/cpu_atom/cpus";
const CPU_INFO: &str = "/proc/cpuinfo";

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
    fn title(self) -> String {
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

    fn description(self) -> &'static str {
        match self {
            CoreRole::PerformanceCores => {
                "Larger cores with higher clocks and power draw, preferred for demanding work."
            }
            CoreRole::MidCapacityCores => {
                "Mid-sized cores between the performance and efficient ones in clock and power \
                 draw."
            }
            CoreRole::EfficientCores => {
                "Smaller cores with lower clocks and power draw, suited to background work."
            }
            CoreRole::LowPowerIslandCores => {
                "Efficient cores outside the performance cores' cache, so the rest of the chip can \
                 sleep while these handle light work."
            }
            CoreRole::StackedCacheCores => {
                "A chiplet with extra cache stacked on top: more data stays on-chip at the cost of \
                 a lower peak clock, which suits games and other cache-hungry work."
            }
            CoreRole::HighClockCores => {
                "A chiplet without stacked cache that clocks higher, suited to work that needs \
                 speed more than cache."
            }
            CoreRole::CompactCores => {
                "Denser cores with less cache and a lower peak clock, trading speed for area and \
                 power."
            }
            CoreRole::CoreComplex { .. } => {
                "Cores on one chiplet share its cache; work that moves to another complex leaves \
                 that cache behind."
            }
            CoreRole::AllCores => "All cores are of one design.",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CpuTopology {
    core_groups: Vec<CoreGroup>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoreGroup {
    role: CoreRole,
    logical_cpus: Vec<u32>,
}

impl CoreGroup {
    pub fn title(&self) -> String {
        self.role.title()
    }

    pub fn contains(&self, logical_cpu: u32) -> bool {
        self.logical_cpus.contains(&logical_cpu)
    }

    pub fn caption(&self) -> &'static str {
        self.role.description()
    }
}

impl CpuTopology {
    pub fn core_groups(&self) -> &[CoreGroup] {
        &self.core_groups
    }

    pub fn detect() -> Self {
        let designs = CoreDesign::read_all();
        let online = read_trimmed(format!("{CPU_FREQ}/online")).unwrap_or_default();
        let cpus: Vec<LogicalCpu> = cpulist(&online)
            .map(|index| LogicalCpu::read(index, &designs))
            .collect();
        Self::from_logical_cpus(&cpus)
    }

    fn from_logical_cpus(cpus: &[LogicalCpu]) -> Self {
        let assigned = if designs_tell_cores_apart(cpus) {
            roles_by_exposed_design(cpus)
        } else {
            roles_by_cache_domain(cpus)
        };
        let members_by_role = assigned.into_iter().fold(
            BTreeMap::<CoreRole, Vec<u32>>::new(),
            |mut members_by_role, assigned| {
                members_by_role
                    .entry(assigned.role)
                    .or_default()
                    .push(assigned.index);
                members_by_role
            },
        );
        CpuTopology {
            core_groups: members_by_role
                .into_iter()
                .map(|(role, mut logical_cpus)| {
                    logical_cpus.sort_unstable();
                    CoreGroup { role, logical_cpus }
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct LogicalCpu {
    index: u32,
    design: CoreDesign,
    last_level_cache: Option<SharedCache>,
    peak_clock: Option<Frequency>,
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
        }
    }

    fn shares_cache_with_any(&self, others: &[u32]) -> bool {
        self.last_level_cache
            .as_ref()
            .is_some_and(|cache| cache.sharing_cpus.iter().any(|cpu| others.contains(cpu)))
    }
}

fn highest_peak_clock<'a>(cpus: impl Iterator<Item = &'a LogicalCpu>) -> Option<Frequency> {
    cpus.filter_map(|cpu| cpu.peak_clock).reduce(Frequency::max)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CoreDesign {
    IntelPerformanceCore,
    IntelEfficientCore,
    ArmCorePart { part: String, capacity: u32 },
    NotExposed,
}

impl CoreDesign {
    fn read_all() -> HashMap<u32, CoreDesign> {
        Self::intel_hybrid_designs()
            .or_else(Self::arm_core_parts)
            .unwrap_or_default()
    }

    fn intel_hybrid_designs() -> Option<HashMap<u32, CoreDesign>> {
        let performance = read_trimmed(INTEL_PERFORMANCE_PMU)?;
        let efficient = read_trimmed(INTEL_EFFICIENT_PMU)?;
        Some(Self::from_intel_cpulists(&performance, &efficient))
    }

    fn from_intel_cpulists(performance: &str, efficient: &str) -> HashMap<u32, CoreDesign> {
        let performance = cpulist(performance).map(|cpu| (cpu, CoreDesign::IntelPerformanceCore));
        let efficient = cpulist(efficient).map(|cpu| (cpu, CoreDesign::IntelEfficientCore));
        performance.chain(efficient).collect()
    }

    fn arm_core_parts() -> Option<HashMap<u32, CoreDesign>> {
        let cpuinfo = fs::read_to_string(CPU_INFO).ok()?;
        Some(
            CorePart::parse_all(&cpuinfo)
                .filter_map(|core_part| Some((core_part.core, core_part.design_with_capacity()?)))
                .collect(),
        )
    }

    fn is_exposed(&self) -> bool {
        !matches!(self, CoreDesign::NotExposed)
    }

    fn arm_part(&self) -> Option<&str> {
        match self {
            CoreDesign::ArmCorePart { part, .. } => Some(part),
            _ => None,
        }
    }

    fn same_design_as(&self, other: &CoreDesign) -> bool {
        match self.arm_part() {
            Some(part) => other.arm_part() == Some(part),
            None => self == other,
        }
    }
}

struct AssignedCpu {
    role: CoreRole,
    index: u32,
}

fn designs_tell_cores_apart(cpus: &[LogicalCpu]) -> bool {
    let all_exposed = cpus.iter().all(|cpu| cpu.design.is_exposed());
    let first_design = cpus.first().map(|cpu| &cpu.design);
    let mixed =
        first_design.is_some_and(|first| cpus.iter().any(|cpu| !cpu.design.same_design_as(first)));
    all_exposed && mixed
}

fn roles_by_exposed_design(cpus: &[LogicalCpu]) -> Vec<AssignedCpu> {
    let intel_performance_cpus: Vec<u32> = cpus
        .iter()
        .filter(|cpu| cpu.design == CoreDesign::IntelPerformanceCore)
        .map(|cpu| cpu.index)
        .collect();
    let unranked = cpus.iter().filter_map(|cpu| {
        let role = match &cpu.design {
            CoreDesign::IntelPerformanceCore => CoreRole::PerformanceCores,
            CoreDesign::IntelEfficientCore
                if cpu.last_level_cache.is_some()
                    && !cpu.shares_cache_with_any(&intel_performance_cpus) =>
            {
                CoreRole::LowPowerIslandCores
            }
            CoreDesign::IntelEfficientCore => CoreRole::EfficientCores,
            CoreDesign::NotExposed => CoreRole::AllCores,
            CoreDesign::ArmCorePart { .. } => return None,
        };
        Some(AssignedCpu {
            role,
            index: cpu.index,
        })
    });
    let arm_tiers = ArmPartTier::ranked_by_capacity(cpus);
    let slowest_rank = arm_tiers.len().saturating_sub(1);
    let ranked = arm_tiers.into_iter().enumerate().flat_map(|(rank, tier)| {
        let role = match rank {
            0 => CoreRole::PerformanceCores,
            rank if rank == slowest_rank => CoreRole::EfficientCores,
            _ => CoreRole::MidCapacityCores,
        };
        tier.members
            .into_iter()
            .map(move |index| AssignedCpu { role, index })
    });
    unranked.chain(ranked).collect()
}

struct ArmPartTier<'a> {
    part: &'a str,
    highest_capacity: u32,
    members: Vec<u32>,
}

impl<'a> ArmPartTier<'a> {
    fn ranked_by_capacity(cpus: &'a [LogicalCpu]) -> Vec<Self> {
        let mut tiers = cpus.iter().fold(Vec::<Self>::new(), |mut tiers, cpu| {
            if let CoreDesign::ArmCorePart { part, capacity } = &cpu.design {
                match tiers.iter_mut().find(|tier| tier.part == part) {
                    Some(tier) => {
                        tier.highest_capacity = tier.highest_capacity.max(*capacity);
                        tier.members.push(cpu.index);
                    }
                    None => tiers.push(ArmPartTier {
                        part,
                        highest_capacity: *capacity,
                        members: vec![cpu.index],
                    }),
                }
            }
            tiers
        });
        tiers.sort_by_key(|tier| Reverse(tier.highest_capacity));
        tiers
    }
}

fn roles_by_cache_domain(cpus: &[LogicalCpu]) -> Vec<AssignedCpu> {
    let all_cores = || -> Vec<AssignedCpu> {
        cpus.iter()
            .map(|cpu| AssignedCpu {
                role: CoreRole::AllCores,
                index: cpu.index,
            })
            .collect()
    };
    let Some(domains) = CacheDomain::group_every_cpu(cpus) else {
        return all_cores();
    };
    let [first, rest @ ..] = domains.as_slice() else {
        return all_cores();
    };
    if rest.is_empty() {
        return all_cores();
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
        let largest_cache_clocks_lower = peak_where_largest_is(true).is_some_and(|largest| {
            peak_where_largest_is(false).is_some_and(|smaller| smaller > largest)
        });
        if largest_cache_clocks_lower {
            CacheDomainLayout::StackedCacheBesideHighClock { largest_size }
        } else {
            CacheDomainLayout::FullCoresBesideCompact { largest_size }
        }
    };
    domains
        .iter()
        .enumerate()
        .flat_map(|(position, domain)| {
            let role = layout.role(position, domain);
            domain.members.iter().map(move |cpu| AssignedCpu {
                role,
                index: cpu.index,
            })
        })
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
            .ok()?
            .filter_map(Result::ok)
            .filter_map(|index| Self::read(&index.path()))
            .max_by_key(|cache| cache.level)
    }

    fn read(index_directory: &Path) -> Option<Self> {
        Some(SharedCache {
            level: read_trimmed(index_directory.join("level"))?.parse().ok()?,
            size: parse_cache_size(&read_trimmed(index_directory.join("size"))?)?,
            sharing_cpus: cpulist(&read_trimmed(index_directory.join("shared_cpu_list"))?)
                .collect(),
        })
    }
}

fn parse_cache_size(text: &str) -> Option<Information> {
    if let Some(kibibytes) = text.strip_suffix('K') {
        return Some(Information::new::<kibibyte>(kibibytes.parse().ok()?));
    }
    let mebibytes = text.strip_suffix('M')?;
    Some(Information::new::<mebibyte>(mebibytes.parse().ok()?))
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

    fn design_with_capacity(&self) -> Option<CoreDesign> {
        let capacity = read_trimmed(format!("{CPU_FREQ}/cpu{}/cpu_capacity", self.core))?;
        Some(CoreDesign::ArmCorePart {
            part: self.part.clone(),
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
            .map(|group| (group.role, group.logical_cpus.clone()))
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
    fn intel_pmu_lists_split_performance_from_efficient() {
        let designs = CoreDesign::from_intel_cpulists("0-1", "2-3");
        assert_eq!(designs[&1], CoreDesign::IntelPerformanceCore);
        assert_eq!(designs[&2], CoreDesign::IntelEfficientCore);
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
