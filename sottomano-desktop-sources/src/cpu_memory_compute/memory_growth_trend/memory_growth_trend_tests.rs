use super::*;

fn process_incarnation(pid: i32, started: u64) -> ProcessIncarnation {
    ProcessIncarnation { pid, started }
}

fn resident_sample(pid: i32, started: u64, mebibytes: u64) -> ProcessResident {
    ProcessResident {
        incarnation: process_incarnation(pid, started),
        command_name: format!("process-{pid}"),
        resident: ByteSize::mib(mebibytes),
    }
}

fn projection_after(used_mebibytes: &[u64], headroom_mebibytes: u64) -> Option<Duration> {
    let start = Instant::now();
    let mut window = MemoryTrendWindow::spanning(MemoryGrowthTrendConfig::default().trend_window);
    used_mebibytes
        .iter()
        .zip(0u64..)
        .map(|(used, tick)| {
            window
                .record_used(
                    start
                        + GROWTH_POLL_PERIOD * u32::try_from(tick).expect("test series are short"),
                    ByteSize::mib(*used),
                )
                .filter(|bytes_per_second| *bytes_per_second > 0.0)
                .and_then(|bytes_per_second| {
                    match projected_exhaustion(ByteSize::mib(headroom_mebibytes), bytes_per_second)
                    {
                        ExhaustionProjection::FullWithin(remaining) => Some(remaining),
                        ExhaustionProjection::BeyondProjection => None,
                    }
                })
        })
        .last()
        .flatten()
}

fn steadily_rising_mebibytes(start: u64, step: u64, sample_count: u64) -> Vec<u64> {
    (0..sample_count).map(|tick| start + step * tick).collect()
}

#[test]
fn flat_or_falling_usage_projects_no_exhaustion() {
    assert_eq!(projection_after(&[5_000; 40], 4_000), None);
    let falling: Vec<u64> = (0..40).map(|tick| 9_000 - 100 * tick).collect();
    assert_eq!(projection_after(&falling, 4_000), None);
}

#[test]
fn too_short_a_history_projects_no_exhaustion() {
    assert_eq!(
        projection_after(&steadily_rising_mebibytes(5_000, 1_000, 10), 4_000),
        None
    );
}

#[test]
fn rising_usage_projects_when_memory_runs_out() {
    let projection = projection_after(&steadily_rising_mebibytes(5_000, 100, 31), 3_000)
        .expect("rising usage projects exhaustion");
    assert_eq!(projection.as_secs(), 60);
}

#[test]
fn a_short_compile_burst_does_not_project_near_exhaustion() {
    let flat_then_burst: Vec<u64> =
        [vec![5_000; 50], steadily_rising_mebibytes(5_200, 200, 10)].concat();
    let projection = projection_after(&flat_then_burst, 8_000);
    assert!(
        projection.is_none_or(|remaining| remaining > Duration::from_secs(300)),
        "{projection:?}"
    );
}

#[test]
fn severity_follows_the_configured_thresholds() {
    let trend_config = MemoryGrowthTrendConfig::default();
    let rising_towards = |remaining| MemoryUsageTrend::Rising {
        used_per_minute: ByteSize::gib(1),
        exhaustion: ExhaustionProjection::FullWithin(remaining),
    };
    assert_eq!(
        MemoryUsageTrend::Steady.filling_severity(&trend_config),
        Severity::Normal
    );
    assert_eq!(
        rising_towards(Duration::from_mins(10)).filling_severity(&trend_config),
        Severity::Normal
    );
    assert_eq!(
        rising_towards(Duration::from_mins(3)).filling_severity(&trend_config),
        Severity::Warning
    );
    assert_eq!(
        rising_towards(Duration::from_secs(30)).filling_severity(&trend_config),
        Severity::Critical
    );
}

#[test]
fn free_swap_counts_as_headroom() {
    let rising = steadily_rising_mebibytes(5_000, 100, 31);
    let ram_only = projection_after(&rising, 8_000).expect("rising usage projects");
    let with_swap = projection_after(&rising, 24_000).expect("rising usage projects");
    assert!(ram_only < Duration::from_secs(300), "{ram_only:?}");
    assert!(with_swap > Duration::from_secs(300), "{with_swap:?}");
}

#[test]
fn samples_older_than_the_window_stop_counting() {
    let start = Instant::now();
    let mut window = MemoryTrendWindow::spanning(MemoryGrowthTrendConfig::default().trend_window);
    window.record_used(start, ByteSize::mib(1_000));
    window.record_used(
        start + MemoryGrowthTrendConfig::default().trend_window + Duration::from_secs(1),
        ByteSize::mib(2_000),
    );
    assert_eq!(window.used_samples.len(), 1);
}

#[test]
fn growth_is_ranked_and_measured_per_minute() {
    let start = Instant::now();
    let mut window = MemoryTrendWindow::spanning(MemoryGrowthTrendConfig::default().trend_window);
    let first = window.record_residents(
        start,
        vec![
            resident_sample(1, 10, 1_000),
            resident_sample(2, 20, 500),
            resident_sample(3, 30, 800),
        ],
    );
    assert!(first.is_empty());
    let growing = window.record_residents(
        start + Duration::from_secs(30),
        vec![
            resident_sample(1, 10, 1_100),
            resident_sample(2, 20, 900),
            resident_sample(3, 30, 700),
        ],
    );
    let ranked: Vec<(i32, ByteSize)> = growing
        .iter()
        .map(|process| (process.pid, process.growth_per_minute))
        .collect();
    assert_eq!(
        ranked,
        vec![(2, ByteSize::mib(800)), (1, ByteSize::mib(200))]
    );
}

#[test]
fn a_reused_pid_is_not_growth() {
    let start = Instant::now();
    let mut window = MemoryTrendWindow::spanning(MemoryGrowthTrendConfig::default().trend_window);
    window.record_residents(start, vec![resident_sample(7, 100, 100)]);
    let growing = window.record_residents(
        start + Duration::from_secs(10),
        vec![resident_sample(7, 200, 4_000)],
    );
    assert!(growing.is_empty());
}
