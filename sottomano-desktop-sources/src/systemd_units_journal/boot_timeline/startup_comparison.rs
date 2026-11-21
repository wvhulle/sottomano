use std::time::Duration;

use sottomano_toml_config::severity::Severity;
use uom::si::{
    f64::{Ratio, Time},
    ratio::{percent, ratio},
};

use super::boot_stage_phases::{BootStage, OsStartupPhases};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StartupSlowdownThresholds {
    pub warning_percent: u32,
    pub critical_percent: u32,
}

const SHORTEST_NOTICEABLE_SLOWDOWN: Duration = Duration::from_secs(1);

impl StartupSlowdownThresholds {
    fn severity(self, slowdown: Duration, change: Ratio) -> Severity {
        if slowdown < SHORTEST_NOTICEABLE_SLOWDOWN {
            Severity::Normal
        } else if change >= Ratio::new::<percent>(f64::from(self.critical_percent)) {
            Severity::Critical
        } else if change >= Ratio::new::<percent>(f64::from(self.warning_percent)) {
            Severity::Warning
        } else {
            Severity::Normal
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DurationComparison {
    pub previous: Duration,
    pub change: Ratio,
    pub severity: Severity,
}

impl DurationComparison {
    fn compared_durations(
        current: Duration,
        previous: Duration,
        thresholds: StartupSlowdownThresholds,
    ) -> Option<Self> {
        let previous_time = Time::try_from(previous)
            .inspect_err(|conversion_failure| {
                tracing::warn!(
                    "startup slowdown: previous duration {previous:?} not convertible: \
                     {conversion_failure:?}"
                );
            })
            .ok()?;
        let current_time = Time::try_from(current)
            .inspect_err(|conversion_failure| {
                tracing::warn!(
                    "startup slowdown: current duration {current:?} not convertible: \
                     {conversion_failure:?}"
                );
            })
            .ok()?;
        let change: Ratio = current_time / previous_time;
        let change = Some(change - Ratio::new::<ratio>(1.0)).filter(|change| change.is_finite())?;
        Some(Self {
            previous,
            change,
            severity: current
                .checked_sub(previous)
                .map_or(Severity::Normal, |slowdown| {
                    thresholds.severity(slowdown, change)
                }),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StageComparison {
    pub stage: BootStage,
    pub comparison: DurationComparison,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StartupComparison {
    pub os_startup: DurationComparison,
    pub stages: Vec<StageComparison>,
}

impl StartupComparison {
    pub(super) fn compared_startups(
        current: OsStartupPhases,
        previous: OsStartupPhases,
        thresholds: StartupSlowdownThresholds,
    ) -> Option<Self> {
        let stages = [BootStage::Kernel, BootStage::Initrd, BootStage::Userspace]
            .into_iter()
            .filter_map(|stage| {
                Some(StageComparison {
                    stage,
                    comparison: DurationComparison::compared_durations(
                        current.stage_duration(stage)?,
                        previous.stage_duration(stage)?,
                        thresholds,
                    )?,
                })
            })
            .collect();
        Some(Self {
            os_startup: DurationComparison::compared_durations(
                current.total(),
                previous.total(),
                thresholds,
            )?,
            stages,
        })
    }

    #[must_use]
    pub fn stage(&self, stage: BootStage) -> Option<&DurationComparison> {
        self.stages
            .iter()
            .find(|stage_comparison| stage_comparison.stage == stage)
            .map(|stage_comparison| &stage_comparison.comparison)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systemd_units_journal::boot_timeline::boot_stage_phases::InitrdPhase;

    fn seconds(seconds: f64) -> Duration {
        Duration::from_secs_f64(seconds)
    }

    const THRESHOLDS: StartupSlowdownThresholds = StartupSlowdownThresholds {
        warning_percent: 25,
        critical_percent: 60,
    };

    fn assert_whole_percent(change: Ratio, expected_percent: f64) {
        assert!(
            (change.get::<percent>() - expected_percent).abs() < 0.5,
            "{} % is not about {expected_percent} %",
            change.get::<percent>()
        );
    }

    fn os_phases(kernel: f64, initrd: InitrdPhase, userspace: f64) -> OsStartupPhases {
        OsStartupPhases {
            kernel: seconds(kernel),
            initrd,
            userspace: seconds(userspace),
        }
    }

    #[test]
    fn startup_compares_only_stages_both_boots_had() {
        let comparison = StartupComparison::compared_startups(
            os_phases(0.5, InitrdPhase::Ran(seconds(6.0)), 12.0),
            os_phases(0.6, InitrdPhase::Skipped, 10.0),
            THRESHOLDS,
        )
        .expect("a comparison");
        assert_eq!(comparison.os_startup.previous.as_millis(), 10_600);
        assert_whole_percent(comparison.os_startup.change, 74.5);
        assert_eq!(comparison.os_startup.severity, Severity::Critical);
        assert_eq!(comparison.stage(BootStage::Initrd), None);
        assert_eq!(comparison.stage(BootStage::Firmware), None);
        let userspace = comparison
            .stage(BootStage::Userspace)
            .expect("userspace compared");
        assert_whole_percent(userspace.change, 20.0);
        assert_eq!(userspace.severity, Severity::Normal);
    }

    #[test]
    fn comparison_grades_by_thresholds_and_ignores_sub_second_slowdowns() {
        let graded = |current: f64| {
            DurationComparison::compared_durations(seconds(current), seconds(10.0), THRESHOLDS)
                .expect("a comparison")
        };
        assert_whole_percent(graded(9.0).change, -10.0);
        assert_eq!(graded(12.4).severity, Severity::Normal);
        assert_eq!(graded(12.5).severity, Severity::Warning);
        assert_eq!(graded(15.9).severity, Severity::Warning);
        assert_eq!(graded(16.0).severity, Severity::Critical);
        let tiny = DurationComparison::compared_durations(seconds(0.9), seconds(0.5), THRESHOLDS)
            .expect("a comparison");
        assert_whole_percent(tiny.change, 80.0);
        assert_eq!(tiny.severity, Severity::Normal);
        assert_eq!(
            DurationComparison::compared_durations(seconds(1.0), Duration::ZERO, THRESHOLDS),
            None
        );
    }
}
