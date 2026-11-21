use std::time::Duration;

use super::{ManagerTimestamps, MonotonicStamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootStage {
    Firmware,
    Loader,
    Kernel,
    Initrd,
    Userspace,
}

impl BootStage {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            BootStage::Firmware => "firmware",
            BootStage::Loader => "loader",
            BootStage::Kernel => "kernel",
            BootStage::Initrd => "initrd",
            BootStage::Userspace => "userspace",
        }
    }

    #[must_use]
    pub fn meaning(self) -> &'static str {
        match self {
            BootStage::Firmware => "UEFI, before the boot loader",
            BootStage::Loader => "boot loader picking and loading the kernel",
            BootStage::Kernel => "kernel until it hands over to initrd or userspace",
            BootStage::Initrd => "early userspace that mounts the root file system",
            BootStage::Userspace => "systemd starting units until the default target",
        }
    }
}

fn phase_between(phase_start: Duration, phase_end: Duration, stage: BootStage) -> Option<Duration> {
    let phase_length = phase_end.checked_sub(phase_start);
    if phase_length.is_none() {
        tracing::debug!(
            "boot timeline: {stage:?} ends at {phase_end:?}, before it starts at {phase_start:?}"
        );
    }
    phase_length
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootPhase {
    pub stage: BootStage,
    pub duration: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitrdPhase {
    Skipped,
    Ran(Duration),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootPhases {
    pub pre_kernel: Vec<BootPhase>,
    pub os_startup: OsStartupPhases,
}

impl BootPhases {
    pub(super) fn phases_until_finish(
        timestamps: &ManagerTimestamps,
        finish: Duration,
    ) -> Option<Self> {
        let MonotonicStamp::SinceBoot(userspace_start) = timestamps.userspace else {
            return None;
        };
        let firmware_duration = match timestamps.firmware {
            MonotonicStamp::Unset => None,
            MonotonicStamp::SinceBoot(firmware_start) => match timestamps.loader {
                MonotonicStamp::Unset => Some(firmware_start),
                MonotonicStamp::SinceBoot(loader_start) => {
                    phase_between(loader_start, firmware_start, BootStage::Firmware)
                }
            },
        };
        let firmware_phase = firmware_duration.map(|duration| BootPhase {
            stage: BootStage::Firmware,
            duration,
        });
        let loader_phase = match timestamps.loader {
            MonotonicStamp::Unset => None,
            MonotonicStamp::SinceBoot(duration) => Some(BootPhase {
                stage: BootStage::Loader,
                duration,
            }),
        };
        let (kernel, initrd) = match timestamps.initrd {
            MonotonicStamp::Unset => (userspace_start, InitrdPhase::Skipped),
            MonotonicStamp::SinceBoot(initrd_start) => (
                initrd_start,
                phase_between(initrd_start, userspace_start, BootStage::Initrd)
                    .map_or(InitrdPhase::Skipped, InitrdPhase::Ran),
            ),
        };
        Some(Self {
            pre_kernel: firmware_phase.into_iter().chain(loader_phase).collect(),
            os_startup: OsStartupPhases {
                kernel,
                initrd,
                userspace: phase_between(userspace_start, finish, BootStage::Userspace)?,
            },
        })
    }

    #[must_use]
    pub fn present(&self) -> Vec<BootPhase> {
        self.pre_kernel
            .iter()
            .copied()
            .chain(self.os_startup.present())
            .collect()
    }

    #[must_use]
    pub fn total(&self) -> Duration {
        self.present().iter().map(|phase| phase.duration).sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OsStartupPhases {
    pub kernel: Duration,
    pub initrd: InitrdPhase,
    pub userspace: Duration,
}

impl OsStartupPhases {
    #[must_use]
    pub fn total(&self) -> Duration {
        self.present().map(|phase| phase.duration).sum()
    }

    fn present(self) -> impl Iterator<Item = BootPhase> {
        let initrd_phase = match self.initrd {
            InitrdPhase::Skipped => None,
            InitrdPhase::Ran(duration) => Some(BootPhase {
                stage: BootStage::Initrd,
                duration,
            }),
        };
        [BootPhase {
            stage: BootStage::Kernel,
            duration: self.kernel,
        }]
        .into_iter()
        .chain(initrd_phase)
        .chain([BootPhase {
            stage: BootStage::Userspace,
            duration: self.userspace,
        }])
    }

    pub(super) fn stage_duration(&self, stage: BootStage) -> Option<Duration> {
        self.present()
            .find(|phase| phase.stage == stage)
            .map(|phase| phase.duration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seconds(seconds: f64) -> Duration {
        Duration::from_secs_f64(seconds)
    }

    #[test]
    fn phases_follow_systemd_analyze_and_skip_a_missing_loader() {
        let timestamps = ManagerTimestamps {
            firmware: MonotonicStamp::SinceBoot(seconds(26.0)),
            loader: MonotonicStamp::Unset,
            initrd: MonotonicStamp::SinceBoot(seconds(0.5)),
            userspace: MonotonicStamp::SinceBoot(seconds(4.0)),
            finish: MonotonicStamp::SinceBoot(seconds(37.0)),
        };
        let phases =
            BootPhases::phases_until_finish(&timestamps, Duration::from_secs(37)).expect("phases");
        assert_eq!(
            phases.present(),
            [
                BootPhase {
                    stage: BootStage::Firmware,
                    duration: Duration::from_secs(26),
                },
                BootPhase {
                    stage: BootStage::Kernel,
                    duration: Duration::from_millis(500),
                },
                BootPhase {
                    stage: BootStage::Initrd,
                    duration: Duration::from_millis(3500),
                },
                BootPhase {
                    stage: BootStage::Userspace,
                    duration: Duration::from_secs(33),
                },
            ]
        );
    }
}
