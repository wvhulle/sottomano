use sottomano_desktop_sources::login_session_presence::{
    idle_session_actions::IdleAction,
    inhibit_reason_text::InhibitReason,
    kernel_sleep_obstacles::{
        IdleActionSetting, InhibitedOperation, KernelHibernation, LogindCapability, ResumeDevice,
        SleepObstacles, SleepState,
    },
    screensaver_bus_idle_inhibitors::IdleInhibitor,
};

use super::{
    BlockerCause, HolderProcess, HolderProcessIdentity, IdleActionBlocking, PauseBlocker,
    TimerArming,
    sleep_state_answers::{kernel_refusal_text, unsupported_by_logind_text},
};
use crate::widgets::idle::ActionTrigger;

pub(in crate::indicator_groups::idle_pause_groups) struct ScreensaverFacts<'group> {
    pub idle_inhibitors: &'group [IdleInhibitor],
    pub armed_triggers: &'group [ActionTrigger],
}

pub(in crate::indicator_groups::idle_pause_groups) fn screensaver_pause_blockers(
    facts: &ScreensaverFacts<'_>,
) -> Vec<PauseBlocker> {
    screensaver_blockers(facts.idle_inhibitors, facts.armed_triggers)
        .fold(Vec::new(), merge_blocker_of_same_holder)
}

pub(in crate::indicator_groups::idle_pause_groups) fn pause_blockers_with_sleep_obstacles(
    obstacles: &SleepObstacles,
    hibernate_timer: TimerArming,
    facts: &ScreensaverFacts<'_>,
) -> Vec<PauseBlocker> {
    hibernate_blockers(obstacles, hibernate_timer)
        .into_iter()
        .chain(lock_blockers(obstacles))
        .chain(no_sleep_state_blocker(obstacles))
        .chain(screensaver_blockers(
            facts.idle_inhibitors,
            facts.armed_triggers,
        ))
        .fold(Vec::new(), merge_blocker_of_same_holder)
}

fn merge_blocker_of_same_holder(
    mut merged_blockers: Vec<PauseBlocker>,
    blocker: PauseBlocker,
) -> Vec<PauseBlocker> {
    let Some(same_holder) = merged_blockers
        .iter_mut()
        .find(|merged| merged.pause == blocker.pause && merged.holder == blocker.holder)
    else {
        merged_blockers.push(blocker);
        return merged_blockers;
    };
    if !same_holder
        .why
        .split("; ")
        .any(|reason| reason == blocker.why)
    {
        same_holder.why = format!("{}; {}", same_holder.why, blocker.why);
    }
    if blocker.action_blocking == IdleActionBlocking::StopsArmedAction {
        same_holder.action_blocking = IdleActionBlocking::StopsArmedAction;
    }
    if same_holder.holder_process == HolderProcessIdentity::NotIdentified {
        same_holder.holder_process = blocker.holder_process;
    }
    merged_blockers
}

fn hibernate_blockers(
    obstacles: &SleepObstacles,
    hibernate_timer: TimerArming,
) -> Vec<PauseBlocker> {
    let explained_by_sleep_lock = obstacles.kernel_hibernation == KernelHibernation::Allowed
        && obstacles
            .inhibitors_blocking(InhibitedOperation::Sleep)
            .count()
            > 0;
    let hibernate_available = matches!(
        obstacles.sleep_state_capability(SleepState::Hibernate),
        LogindCapability::Allowed | LogindCapability::AfterAuthorisation
    );
    if hibernate_available || explained_by_sleep_lock {
        return Vec::new();
    }
    let action_blocking = match (obstacles.resume_device, hibernate_timer) {
        (ResumeDevice::Unset, TimerArming::Unarmed) => IdleActionBlocking::LeavesActionRunning,
        (ResumeDevice::Configured, _) | (_, TimerArming::Armed) => {
            IdleActionBlocking::StopsArmedAction
        }
    };
    let blocker = |holder: String, why: String, cause: BlockerCause| PauseBlocker {
        pause: IdleAction::Hibernate,
        holder,
        why,
        cause,
        action_blocking,
        holder_process: HolderProcessIdentity::NotIdentified,
    };
    let refusal = kernel_refusal_text(&obstacles.kernel_hibernation);
    let kernel = "Kernel".to_owned();
    match &obstacles.kernel_hibernation {
        KernelHibernation::Allowed => {
            let why = match obstacles.sleep_state_capability(SleepState::Hibernate) {
                LogindCapability::NotPermitted => "not permitted for this user".to_owned(),
                LogindCapability::Allowed
                | LogindCapability::AfterAuthorisation
                | LogindCapability::Unsupported => unsupported_by_logind_text().to_owned(),
            };
            vec![blocker(
                "logind".to_owned(),
                why,
                BlockerCause::LogindRefusal,
            )]
        }
        KernelHibernation::DisabledBySecretMemory(holders) => holders
            .iter()
            .map(|holder| PauseBlocker {
                holder_process: HolderProcessIdentity::Identified(HolderProcess {
                    pid: holder.pid,
                    window_ownership: holder.window_ownership,
                }),
                ..blocker(
                    holder.name.clone(),
                    "holds secret memory".to_owned(),
                    BlockerCause::KernelSecretMemory,
                )
            })
            .collect(),
        KernelHibernation::DisabledByBootFlag(_) => {
            vec![blocker(kernel, refusal, BlockerCause::KernelBootFlag)]
        }
        KernelHibernation::DisabledByLockdown(_) => {
            vec![blocker(kernel, refusal, BlockerCause::KernelLockdown)]
        }
        KernelHibernation::DisabledForUnidentifiedCause => {
            vec![blocker(kernel, refusal, BlockerCause::KernelUnidentified)]
        }
        KernelHibernation::DisabledWithoutInspection => {
            vec![blocker(kernel, refusal, BlockerCause::KernelUninspected)]
        }
    }
}

fn lock_blockers(obstacles: &SleepObstacles) -> impl Iterator<Item = PauseBlocker> + '_ {
    let stated_reason = |reason: &InhibitReason, fallback: &str| match reason {
        InhibitReason::Stated(reason_text) => reason_text.clone(),
        InhibitReason::Unstated => fallback.to_owned(),
    };
    let sleep_locks = obstacles
        .inhibitors_blocking(InhibitedOperation::Sleep)
        .flat_map(move |inhibitor| {
            let why = stated_reason(&inhibitor.reason, "holds a sleep lock");
            [
                (IdleAction::Sleep, IdleActionBlocking::StopsArmedAction),
                (
                    IdleAction::Hibernate,
                    IdleActionBlocking::LeavesActionRunning,
                ),
            ]
            .map(|(pause, action_blocking)| PauseBlocker {
                pause,
                holder: inhibitor.holder.clone(),
                why: why.clone(),
                cause: BlockerCause::SleepLock,
                action_blocking,
                holder_process: HolderProcessIdentity::NotIdentified,
            })
        });
    let shutdown_locks = obstacles
        .inhibitors_blocking(InhibitedOperation::Shutdown)
        .map(move |inhibitor| PauseBlocker {
            pause: IdleAction::PowerOff,
            holder: inhibitor.holder.clone(),
            why: stated_reason(&inhibitor.reason, "holds a shutdown lock"),
            cause: BlockerCause::ShutdownLock,
            action_blocking: IdleActionBlocking::StopsArmedAction,
            holder_process: HolderProcessIdentity::NotIdentified,
        });
    let idle_locks =
        obstacles
            .inhibitors_blocking(InhibitedOperation::Idle)
            .map(move |inhibitor| PauseBlocker {
                pause: IdleAction::Sleep,
                holder: inhibitor.holder.clone(),
                why: stated_reason(&inhibitor.reason, "holds an idle lock"),
                cause: BlockerCause::LogindIdleLock,
                action_blocking: match obstacles.idle_action_setting {
                    IdleActionSetting::Acting => IdleActionBlocking::StopsArmedAction,
                    IdleActionSetting::Ignored => IdleActionBlocking::LeavesActionRunning,
                },
                holder_process: HolderProcessIdentity::NotIdentified,
            });
    sleep_locks.chain(shutdown_locks).chain(idle_locks)
}

fn no_sleep_state_blocker(obstacles: &SleepObstacles) -> Option<PauseBlocker> {
    let no_lock_explains_it = obstacles
        .inhibitors_blocking(InhibitedOperation::Sleep)
        .count()
        == 0;
    (no_lock_explains_it
        && !obstacles.sleep_order.is_empty()
        && obstacles.sleep_state_that_runs().is_none())
    .then(|| PauseBlocker {
        pause: IdleAction::Sleep,
        holder: "logind".to_owned(),
        why: "no configured sleep state is available".to_owned(),
        cause: BlockerCause::LogindRefusal,
        action_blocking: IdleActionBlocking::StopsArmedAction,
        holder_process: HolderProcessIdentity::NotIdentified,
    })
}

fn screensaver_blockers<'group>(
    idle_inhibitors: &'group [IdleInhibitor],
    armed_triggers: &'group [ActionTrigger],
) -> impl Iterator<Item = PauseBlocker> + 'group {
    let first_to_fire = armed_triggers
        .iter()
        .min_by_key(|trigger| trigger.after)
        .map(|trigger| trigger.tag);
    idle_inhibitors.iter().flat_map(move |inhibitor| {
        armed_triggers.iter().map(move |trigger| PauseBlocker {
            pause: trigger.tag,
            holder: inhibitor.holder.clone(),
            why: match &inhibitor.reason {
                InhibitReason::Stated(reason_text) => reason_text.clone(),
                InhibitReason::Unstated => "asked the compositor to hold idle timers".to_owned(),
            },
            cause: BlockerCause::ScreenSaverHold,
            action_blocking: if Some(trigger.tag) == first_to_fire {
                IdleActionBlocking::StopsArmedAction
            } else {
                IdleActionBlocking::LeavesActionRunning
            },
            holder_process: HolderProcessIdentity::NotIdentified,
        })
    })
}
