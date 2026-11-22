use super::{
    IdleActionSetting, InhibitedOperation, KernelHibernation, KernelSleepVerdict, LogindCapability,
    ResumeDevice, ShutdownSchedule, SleepObstacles, SleepState, SleepStateSupport,
};

fn obstacles_with(capabilities: [LogindCapability; 4]) -> SleepObstacles {
    SleepObstacles {
        sleep_states: SleepState::ALL
            .into_iter()
            .zip(capabilities)
            .map(|(state, capability)| SleepStateSupport { state, capability })
            .collect(),
        sleep_order: vec![
            SleepState::SuspendThenHibernate,
            SleepState::Suspend,
            SleepState::Hibernate,
        ],
        kernel_hibernation: KernelHibernation::Allowed,
        resume_device: ResumeDevice::Configured,
        blocking_inhibitors: Vec::new(),
        idle_action_setting: IdleActionSetting::Ignored,
        shutdown_schedule: ShutdownSchedule::NotScheduled,
    }
}

#[test]
fn a_zero_resume_device_means_hibernation_was_never_set_up() {
    assert_eq!(ResumeDevice::from("259:2\n"), ResumeDevice::Configured);
    assert_eq!(ResumeDevice::from("0:0\n"), ResumeDevice::Unset);
    assert_eq!(ResumeDevice::from(""), ResumeDevice::Unset);
}

#[test]
fn the_first_available_state_in_logind_order_is_what_runs() {
    let obstacles = obstacles_with([
        LogindCapability::Allowed,
        LogindCapability::Unsupported,
        LogindCapability::Unsupported,
        LogindCapability::Unsupported,
    ]);
    assert_eq!(obstacles.sleep_state_that_runs(), Some(SleepState::Suspend));
}

#[test]
fn a_state_needing_authorisation_still_counts_as_available() {
    let obstacles = obstacles_with([
        LogindCapability::Unsupported,
        LogindCapability::Unsupported,
        LogindCapability::Unsupported,
        LogindCapability::AfterAuthorisation,
    ]);
    assert_eq!(
        obstacles.sleep_state_that_runs(),
        Some(SleepState::SuspendThenHibernate)
    );
}

#[test]
fn nothing_runs_when_no_state_is_available() {
    let obstacles = obstacles_with([LogindCapability::NotPermitted; 4]);
    assert_eq!(obstacles.sleep_state_that_runs(), None);
    assert_eq!(
        obstacles
            .inhibitors_blocking(InhibitedOperation::Sleep)
            .count(),
        0
    );
}

#[test]
fn the_kernel_only_blocks_states_that_write_a_hibernation_image() {
    let obstacles = SleepObstacles {
        kernel_hibernation: KernelHibernation::DisabledWithoutInspection,
        ..obstacles_with([LogindCapability::Allowed; 4])
    };
    assert_eq!(
        obstacles.kernel_verdict(SleepState::Suspend),
        KernelSleepVerdict::Permitted
    );
    for hibernating_state in [
        SleepState::Hibernate,
        SleepState::HybridSleep,
        SleepState::SuspendThenHibernate,
    ] {
        assert_eq!(
            obstacles.kernel_verdict(hibernating_state),
            KernelSleepVerdict::Blocked
        );
    }
}
