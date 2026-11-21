use uom::si::{frequency::megahertz, information::mebibyte};

use super::{sysfs_shared_caches::parse_cache_size, *};

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
            last_level_cache: LastLevelCache::Shared(last_level_cache.clone()),
            peak_clock: PeakClock::Reported(Frequency::new::<megahertz>(peak_megahertz)),
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
        .core_groups
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
    let topology = CpuTopology::from(
        [
            cpus(0..=3, &CoreDesign::IntelPerformanceCore, &l3, 5200.0),
            cpus(4..=7, &CoreDesign::IntelEfficientCore, &l3, 3900.0),
        ]
        .concat()
        .as_slice(),
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
    let topology = CpuTopology::from(
        [
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
        .concat()
        .as_slice(),
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
    let topology = CpuTopology::from(
        cpus(
            0..=15,
            &CoreDesign::NotExposed,
            &cache(3, 32.0, 0..=15),
            5400.0,
        )
        .as_slice(),
    );
    assert_eq!(roles(&topology), [(CoreRole::AllCores, (0..=15).collect())]);
}

#[test]
fn ryzen_with_equal_chiplets_splits_into_numbered_complexes() {
    let first_chiplet = cache(3, 32.0, 0..=3);
    let second_chiplet = cache(3, 32.0, 4..=7);
    let topology = CpuTopology::from(
        [
            cpus(0..=3, &CoreDesign::NotExposed, &first_chiplet, 5700.0),
            cpus(4..=7, &CoreDesign::NotExposed, &second_chiplet, 5600.0),
        ]
        .concat()
        .as_slice(),
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
    let topology = CpuTopology::from(
        [
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
        .concat()
        .as_slice(),
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
    let topology = CpuTopology::from(
        [
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
        .concat()
        .as_slice(),
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
    let topology = CpuTopology::from(
        [
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
        .concat()
        .as_slice(),
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
    let topology = CpuTopology::from(
        [
            cpus(0..=0, &arm_part("0xd81", 1005), &l3, 3000.0),
            cpus(1..=1, &arm_part("0xd81", 1024), &l3, 3100.0),
            cpus(2..=3, &arm_part("0xd80", 676), &l3, 2000.0),
        ]
        .concat()
        .as_slice(),
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
    let topology = CpuTopology::from(
        [
            cpus(0..=0, &arm_part("0xd46", 400), &l3, 1800.0),
            cpus(1..=1, &arm_part("0xd47", 800), &l3, 2400.0),
            cpus(2..=2, &arm_part("0xd48", 1024), &l3, 3000.0),
        ]
        .concat()
        .as_slice(),
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
    let topology = CpuTopology::from(cpus(0..=1, &arm_part("0xd05", 1024), &l3, 1800.0).as_slice());
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
