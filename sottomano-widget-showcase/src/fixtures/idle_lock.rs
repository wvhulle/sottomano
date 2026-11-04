use std::time::Duration;

use sottomano_sources::{
    SnapshotDelivery,
    login_session_presence::kernel_sleep_obstacles::{
        KernelHibernation, LogindCapability, SleepObstacles, SleepState, SleepStateSupport,
    },
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::idle_pause_groups::{IdlePauseGroups, IdlePauseMessage, Timeouts},
};

use crate::showcased_group::ShowcasedGroup;

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = IdlePauseGroups::default();
    let _ = group.update(
        IdlePauseMessage::Loaded(Timeouts {
            lock_user: Some(Duration::from_mins(15)),
            screen_off: Some(Duration::from_mins(20)),
            sleep: Some(Duration::from_hours(1)),
            hibernate: None,
            power_down: None,
        }),
        clock,
    );
    let _ = group.update(
        IdlePauseMessage::SleepObstaclesFetched(Box::new(SnapshotDelivery::live_push(Ok(
            hibernation_held_by_secret_memory(),
        )))),
        clock,
    );
    ShowcasedGroup::IdleTimeouts(Box::new(group))
}

fn hibernation_held_by_secret_memory() -> SleepObstacles {
    SleepObstacles {
        sleep_states: SleepState::ALL
            .map(|state| SleepStateSupport {
                state,
                capability: if state == SleepState::Suspend {
                    LogindCapability::Allowed
                } else {
                    LogindCapability::Unsupported
                },
            })
            .to_vec(),
        sleep_order: vec![
            SleepState::SuspendThenHibernate,
            SleepState::Suspend,
            SleepState::Hibernate,
        ],
        kernel_hibernation: KernelHibernation::DisabledBySecretMemory(vec!["Bitwarden".to_owned()]),
        resume_device_configured: true,
        blocking_inhibitors: Vec::new(),
        logind_idle_action_enabled: false,
        scheduled_shutdown: None,
    }
}
