use std::time::Duration;

use sottomano_desktop_sources::login_session_presence::{
    idle_session_actions::IdleAction,
    inhibit_reason_text::InhibitReason,
    kernel_sleep_obstacles::{
        BlockingInhibitor, IdleActionSetting, InhibitedOperation, KernelHibernation,
        LogindCapability, ResumeDevice, SecretMemoryHolder, ShutdownSchedule, SleepObstacles,
        SleepState, SleepStateSupport, WindowOwnership,
    },
    screensaver_bus_idle_inhibitors::IdleInhibitor,
};

use super::{
    BlockerCause, HolderProcess, HolderProcessIdentity, IdleActionBlocking, PauseBlocker,
    TimerArming,
    sleep_obstacle_blockers::{
        ScreensaverFacts, pause_blockers_with_sleep_obstacles, screensaver_pause_blockers,
    },
    sleep_state_answers::sleep_state_answer,
};
use crate::{group_indicators::indicator_state::IndicatorState, widgets::idle::ActionTrigger};

fn clear_obstacles() -> SleepObstacles {
    SleepObstacles {
        sleep_states: SleepState::ALL
            .map(|state| SleepStateSupport {
                state,
                capability: LogindCapability::Allowed,
            })
            .to_vec(),
        sleep_order: vec![SleepState::Suspend],
        kernel_hibernation: KernelHibernation::Allowed,
        resume_device: ResumeDevice::Configured,
        blocking_inhibitors: Vec::new(),
        idle_action_setting: IdleActionSetting::Ignored,
        shutdown_schedule: ShutdownSchedule::NotScheduled,
    }
}

fn secret_memory_obstacles() -> SleepObstacles {
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
        kernel_hibernation: KernelHibernation::DisabledBySecretMemory(vec![SecretMemoryHolder {
            name: "Bitwarden".to_owned(),
            pid: 6627,
            window_ownership: WindowOwnership::Windowless,
        }]),
        ..clear_obstacles()
    }
}

fn obstacle_blockers(
    obstacles: &SleepObstacles,
    hibernate_timer: TimerArming,
) -> Vec<PauseBlocker> {
    pause_blockers_with_sleep_obstacles(
        obstacles,
        hibernate_timer,
        &ScreensaverFacts {
            idle_inhibitors: &[],
            armed_triggers: &[],
        },
    )
}

fn backup_inhibitor(operation: InhibitedOperation) -> BlockingInhibitor {
    BlockingInhibitor {
        holder: "Backup".to_owned(),
        operation,
        reason: InhibitReason::Stated("Backup running".to_owned()),
    }
}

#[test]
fn nothing_blocks_a_clear_machine() {
    assert!(obstacle_blockers(&clear_obstacles(), TimerArming::Unarmed).is_empty());
}

#[test]
fn secret_memory_names_the_exact_app_under_hibernate_only() {
    let blockers = obstacle_blockers(&secret_memory_obstacles(), TimerArming::Unarmed);
    assert_eq!(blockers.len(), 1);
    assert_eq!(blockers[0].pause, IdleAction::Hibernate);
    assert_eq!(blockers[0].holder, "Bitwarden");
    assert_eq!(blockers[0].cause, BlockerCause::KernelSecretMemory);
    assert_eq!(
        blockers[0].holder_process,
        HolderProcessIdentity::Identified(HolderProcess {
            pid: 6627,
            window_ownership: WindowOwnership::Windowless,
        })
    );
    assert_eq!(
        blockers[0].action_blocking,
        IdleActionBlocking::StopsArmedAction
    );
}

#[test]
fn without_the_helper_no_app_is_named() {
    let obstacles = SleepObstacles {
        kernel_hibernation: KernelHibernation::DisabledWithoutInspection,
        ..secret_memory_obstacles()
    };
    let blockers = obstacle_blockers(&obstacles, TimerArming::Unarmed);
    assert_eq!(blockers[0].holder, "Kernel");
    assert_eq!(blockers[0].cause, BlockerCause::KernelUninspected);
}

#[test]
fn a_machine_never_set_up_for_hibernation_stays_off_the_panel() {
    let obstacles = SleepObstacles {
        resume_device: ResumeDevice::Unset,
        ..secret_memory_obstacles()
    };
    assert_eq!(
        obstacle_blockers(&obstacles, TimerArming::Unarmed)[0].action_blocking,
        IdleActionBlocking::LeavesActionRunning
    );
    assert_eq!(
        obstacle_blockers(&obstacles, TimerArming::Armed)[0].action_blocking,
        IdleActionBlocking::StopsArmedAction
    );
}

#[test]
fn a_sleep_lock_raises_standby_but_only_lists_under_hibernate() {
    let obstacles = SleepObstacles {
        blocking_inhibitors: vec![backup_inhibitor(InhibitedOperation::Sleep)],
        ..clear_obstacles()
    };
    let blockers = obstacle_blockers(&obstacles, TimerArming::Unarmed);
    let rising: Vec<IdleAction> = blockers
        .iter()
        .filter(|blocker| blocker.display_state() == IndicatorState::Critical)
        .map(|blocker| blocker.pause)
        .collect();
    assert_eq!(rising, vec![IdleAction::Sleep]);
    assert!(
        blockers
            .iter()
            .any(|blocker| blocker.pause == IdleAction::Hibernate)
    );
    assert_eq!(blockers[0].why, "Backup running");
}

#[test]
fn a_shutdown_lock_raises_power_down() {
    let obstacles = SleepObstacles {
        blocking_inhibitors: vec![backup_inhibitor(InhibitedOperation::Shutdown)],
        ..clear_obstacles()
    };
    let blockers = obstacle_blockers(&obstacles, TimerArming::Unarmed);
    assert_eq!(blockers[0].pause, IdleAction::PowerOff);
    assert_eq!(
        blockers[0].action_blocking,
        IdleActionBlocking::StopsArmedAction
    );
}

#[test]
fn a_logind_idle_lock_only_rises_when_logind_has_an_idle_action() {
    let without_action = SleepObstacles {
        blocking_inhibitors: vec![backup_inhibitor(InhibitedOperation::Idle)],
        ..clear_obstacles()
    };
    let with_action = SleepObstacles {
        idle_action_setting: IdleActionSetting::Acting,
        ..without_action.clone()
    };
    assert_eq!(
        obstacle_blockers(&without_action, TimerArming::Unarmed)[0].action_blocking,
        IdleActionBlocking::LeavesActionRunning
    );
    assert_eq!(
        obstacle_blockers(&with_action, TimerArming::Unarmed)[0].action_blocking,
        IdleActionBlocking::StopsArmedAction
    );
}

#[test]
fn a_screensaver_hold_lists_every_armed_timer_and_raises_the_first_to_fire() {
    let idle_inhibitors = [IdleInhibitor {
        holder: "Firefox".to_owned(),
        reason: InhibitReason::Stated("Playing video".to_owned()),
    }];
    let armed_triggers = [
        ActionTrigger {
            after: Duration::from_mins(30),
            tag: IdleAction::Sleep,
        },
        ActionTrigger {
            after: Duration::from_mins(5),
            tag: IdleAction::Lock,
        },
    ];
    let blockers = screensaver_pause_blockers(&ScreensaverFacts {
        idle_inhibitors: &idle_inhibitors,
        armed_triggers: &armed_triggers,
    });
    assert_eq!(blockers.len(), 2);
    let rising: Vec<IdleAction> = blockers
        .iter()
        .filter(|blocker| blocker.action_blocking == IdleActionBlocking::StopsArmedAction)
        .map(|blocker| blocker.pause)
        .collect();
    assert_eq!(rising, vec![IdleAction::Lock]);
}

#[test]
fn every_sleep_state_gets_its_own_exact_answer() {
    let obstacles = secret_memory_obstacles();
    let runs = obstacles.sleep_state_that_runs();
    assert_eq!(
        sleep_state_answer(&obstacles, SleepState::Suspend, runs),
        "will run"
    );
    assert_eq!(
        sleep_state_answer(&obstacles, SleepState::Hibernate, runs),
        "disabled by the kernel while Bitwarden holds secret memory"
    );
}

#[test]
fn a_sleep_lock_is_named_as_the_reason_rather_than_lack_of_support() {
    let obstacles = SleepObstacles {
        sleep_states: SleepState::ALL
            .map(|state| SleepStateSupport {
                state,
                capability: LogindCapability::Unsupported,
            })
            .to_vec(),
        blocking_inhibitors: vec![backup_inhibitor(InhibitedOperation::Sleep)],
        ..clear_obstacles()
    };
    assert_eq!(
        sleep_state_answer(&obstacles, SleepState::Suspend, None),
        "blocked by a sleep lock from Backup"
    );
    let blockers = obstacle_blockers(&obstacles, TimerArming::Unarmed);
    assert!(blockers.iter().all(|blocker| blocker.holder == "Backup"));
}

#[test]
fn a_kernel_disabled_state_keeps_the_kernel_reason_during_a_sleep_lock() {
    let obstacles = SleepObstacles {
        blocking_inhibitors: vec![backup_inhibitor(InhibitedOperation::Sleep)],
        ..secret_memory_obstacles()
    };
    assert_eq!(
        sleep_state_answer(&obstacles, SleepState::Hibernate, None),
        "disabled by the kernel while Bitwarden holds secret memory"
    );
}

#[test]
fn logind_refusal_is_reported_as_answered_rather_than_explained() {
    let obstacles = SleepObstacles {
        sleep_states: SleepState::ALL
            .map(|state| SleepStateSupport {
                state,
                capability: if state == SleepState::Hibernate {
                    LogindCapability::Unsupported
                } else {
                    LogindCapability::Allowed
                },
            })
            .to_vec(),
        ..clear_obstacles()
    };
    assert_eq!(
        sleep_state_answer(&obstacles, SleepState::Hibernate, Some(SleepState::Suspend)),
        "logind reports this isn't supported on this machine"
    );
    assert_eq!(
        obstacle_blockers(&obstacles, TimerArming::Unarmed)[0].holder,
        "logind"
    );
}
