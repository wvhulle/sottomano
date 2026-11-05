use std::{borrow::Cow, iter::once};

use chrono::Locale;
use iced::alignment::Horizontal;
use sottomano_config::severity::Severity;
use sottomano_desktop_sources::login_session_presence::{
    idle_session_actions::IdleAction,
    kernel_sleep_obstacles::{
        InhibitedOperation, KernelHibernation, LogindCapability, ScheduledShutdown, SleepObstacles,
        SleepState,
    },
    screensaver_bus_idle_inhibitors::IdleInhibitor,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        idle::ActionTrigger,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        ongoing_toast::moment_text,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BlockerCause {
    KernelSecretMemory,
    KernelBootFlag,
    KernelLockdown,
    KernelUnidentified,
    KernelUninspected,
    LogindRefusal,
    SleepLock,
    ShutdownLock,
    LogindIdleLock,
    ScreenSaverHold,
}

impl BlockerCause {
    fn section_heading(self) -> (&'static str, &'static str) {
        match self {
            BlockerCause::KernelSecretMemory => (
                "Hibernate",
                "The kernel disables hibernation while these apps hold secret memory.",
            ),
            BlockerCause::KernelBootFlag => (
                "Hibernate",
                "The kernel disables hibernation because of how it was booted.",
            ),
            BlockerCause::KernelLockdown => (
                "Hibernate",
                "The kernel disables hibernation while lockdown is active.",
            ),
            BlockerCause::KernelUnidentified => (
                "Hibernate",
                "Disabled by the kernel. No boot flag, lockdown or secret memory was found.",
            ),
            BlockerCause::KernelUninspected => (
                "Hibernate",
                "Disabled by the kernel. Enable the NixOS module to see which app caused it.",
            ),
            BlockerCause::LogindRefusal => ("Sleep state", "As answered by logind."),
            BlockerCause::SleepLock => {
                ("Sleep", "These apps hold a logind lock that blocks sleep.")
            }
            BlockerCause::ShutdownLock => (
                "Power-down",
                "These apps hold a logind lock that blocks shutdown.",
            ),
            BlockerCause::LogindIdleLock => (
                "logind idle action",
                "These apps hold a logind lock that stops logind's own idle action.",
            ),
            BlockerCause::ScreenSaverHold => (
                "Idle timers",
                "These apps asked the compositor to hold idle timers, as seen since sottomano \
                 started. Apps using Wayland's idle-inhibit protocol can't be listed.",
            ),
        }
    }

    fn holds_only_idle_timers(self) -> bool {
        matches!(
            self,
            BlockerCause::LogindIdleLock | BlockerCause::ScreenSaverHold
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PauseBlocker {
    pub pause: IdleAction,
    pub holder: String,
    pub why: String,
    pub cause: BlockerCause,
    pub rises_to_panel: bool,
}

impl PauseBlocker {
    fn blocked_phrase(&self) -> Cow<'static, str> {
        match (self.cause, self.pause) {
            (BlockerCause::ScreenSaverHold, pause) => format!("{} held", pause_title(pause)).into(),
            (BlockerCause::LogindIdleLock, _) => "Idle action held".into(),
            (_, IdleAction::Hibernate) => "No hibernate".into(),
            (_, IdleAction::PowerOff) => "No power-down".into(),
            (_, _) => "No sleep".into(),
        }
    }
}

#[must_use]
pub(super) fn pause_title(pause: IdleAction) -> &'static str {
    match pause {
        IdleAction::Lock => "Lock",
        IdleAction::ScreenOff => "Screen off",
        IdleAction::Sleep => "Standby",
        IdleAction::Hibernate => "Hibernate",
        IdleAction::PowerOff => "Power-down",
    }
}

impl GroupItemReading for PauseBlocker {
    fn name(&self) -> &'static str {
        "Pause blocker"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.holder.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Warning(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.blocked_phrase().into_owned())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.cause.holds_only_idle_timers() {
            ReadingDisplayState::Idle
        } else {
            ReadingDisplayState::Warning
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections(blocker_sections(once(self)))
    }
}

impl IntoTableRecord<1> for &PauseBlocker {
    fn into_table_record(self) -> TableRecord<1> {
        TableRecord::new(self.holder.clone(), [self.why.clone().into()])
    }
}

pub(super) struct NextSleepStateRow {
    pub state: SleepState,
}

impl GroupItemReading for NextSleepStateRow {
    fn name(&self) -> &'static str {
        "Sleep type that runs"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.state.title().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Bed(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some("will run".to_owned())
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
    }
}

pub(super) struct ScheduledShutdownRow<'a> {
    pub shutdown: &'a ScheduledShutdown,
}

impl GroupItemReading for ScheduledShutdownRow<'_> {
    fn name(&self) -> &'static str {
        "Scheduled shutdown"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        match self.shutdown.kind.as_str() {
            "poweroff" | "dry-poweroff" => "Power off scheduled",
            "reboot" | "dry-reboot" => "Reboot scheduled",
            "halt" | "dry-halt" => "Halt scheduled",
            _ => "Shutdown scheduled",
        }
        .into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Clock(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        self.shutdown
            .at
            .map(|at| moment_text(at, clock.utc(), locale))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Warning
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
    }
}

pub(super) fn blocker_sections<'a>(
    blockers: impl IntoIterator<Item = &'a PauseBlocker>,
) -> Vec<Section> {
    let blockers: Vec<&PauseBlocker> = blockers.into_iter().collect();
    let mut causes: Vec<BlockerCause> = Vec::new();
    for blocker in &blockers {
        if !causes.contains(&blocker.cause) {
            causes.push(blocker.cause);
        }
    }
    causes
        .into_iter()
        .map(|cause| {
            let rows = blockers
                .iter()
                .copied()
                .filter(|blocker| blocker.cause == cause);
            Section::column_table(
                [ColumnHeading::titled("Why").align_x(Horizontal::Left)],
                rows,
            )
            .titled(cause.section_heading())
        })
        .collect()
}

pub(super) fn sleep_state_sections(obstacles: &SleepObstacles) -> Vec<Section> {
    let state_that_runs = obstacles.sleep_state_that_runs();
    let rows = SleepState::ALL.into_iter().map(|state| {
        let answer = sleep_state_answer(obstacles, state, state_that_runs);
        let row = LabeledValue::new(state.title(), answer);
        if obstacles.is_available(state) {
            row
        } else {
            row.level(Severity::Warning)
        }
    });
    vec![Section::dictionary(rows).titled((
        "What standby does now",
        "logind runs the first available state in its configured order.",
    ))]
}

fn sleep_state_answer(
    obstacles: &SleepObstacles,
    state: SleepState,
    state_that_runs: Option<SleepState>,
) -> String {
    if state_that_runs == Some(state) {
        return "will run".to_owned();
    }
    let sleep_lock_holders: Vec<&str> = obstacles
        .inhibitors_of(InhibitedOperation::Sleep)
        .map(|inhibitor| inhibitor.holder.as_str())
        .collect();
    match obstacles.capability_of(state) {
        LogindCapability::Allowed => "available".to_owned(),
        LogindCapability::AfterAuthorisation => "available after authorisation".to_owned(),
        _ if obstacles.blocked_by_kernel(state) => {
            kernel_refusal_text(&obstacles.kernel_hibernation)
        }
        _ if !sleep_lock_holders.is_empty() => {
            format!(
                "blocked by a sleep lock from {}",
                sleep_lock_holders.join(", ")
            )
        }
        LogindCapability::NotPermitted => "not permitted for this user".to_owned(),
        LogindCapability::Unsupported => unsupported_by_logind_text().to_owned(),
    }
}

fn kernel_refusal_text(kernel_hibernation: &KernelHibernation) -> String {
    match kernel_hibernation {
        KernelHibernation::Allowed => unsupported_by_logind_text().to_owned(),
        KernelHibernation::DisabledByBootFlag(flag) => {
            format!("disabled by the kernel: booted with {flag}")
        }
        KernelHibernation::DisabledByLockdown(level) => {
            format!("disabled by the kernel: lockdown is {level}")
        }
        KernelHibernation::DisabledBySecretMemory(holders) => format!(
            "disabled by the kernel while {} holds secret memory",
            holders.join(", ")
        ),
        KernelHibernation::DisabledForUnidentifiedCause => {
            "disabled by the kernel, with no boot flag, lockdown or secret memory found".to_owned()
        }
        KernelHibernation::DisabledWithoutInspection => {
            "disabled by the kernel; enable the NixOS module to see which app".to_owned()
        }
    }
}

fn unsupported_by_logind_text() -> &'static str {
    "logind reports this isn't supported on this machine"
}

pub(super) struct PauseFacts<'a> {
    pub obstacles: Option<&'a SleepObstacles>,
    pub idle_inhibitors: &'a [IdleInhibitor],
    pub armed_triggers: &'a [ActionTrigger],
    pub hibernate_timer_set: bool,
}

pub(super) fn pause_blockers(facts: &PauseFacts<'_>) -> Vec<PauseBlocker> {
    let logind_and_kernel = facts.obstacles.into_iter().flat_map(|obstacles| {
        hibernate_blockers(obstacles, facts.hibernate_timer_set)
            .into_iter()
            .chain(lock_blockers(obstacles))
            .chain(no_sleep_state_blocker(obstacles))
    });
    logind_and_kernel
        .chain(screensaver_blockers(
            facts.idle_inhibitors,
            facts.armed_triggers,
        ))
        .collect()
}

fn hibernate_blockers(obstacles: &SleepObstacles, timer_set: bool) -> Vec<PauseBlocker> {
    let explained_by_sleep_lock = obstacles.kernel_hibernation.is_allowed()
        && obstacles.inhibitors_of(InhibitedOperation::Sleep).count() > 0;
    if obstacles.is_available(SleepState::Hibernate) || explained_by_sleep_lock {
        return Vec::new();
    }
    let rises_to_panel = obstacles.resume_device_configured || timer_set;
    let blocker = |holder: String, why: String, cause: BlockerCause| PauseBlocker {
        pause: IdleAction::Hibernate,
        holder,
        why,
        cause,
        rises_to_panel,
    };
    let refusal = kernel_refusal_text(&obstacles.kernel_hibernation);
    let kernel = "Kernel".to_owned();
    match &obstacles.kernel_hibernation {
        KernelHibernation::Allowed => {
            let why = match obstacles.capability_of(SleepState::Hibernate) {
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
            .map(|holder| {
                blocker(
                    holder.clone(),
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
    let stated_reason = |reason: &Option<String>, fallback: &str| {
        reason.clone().unwrap_or_else(|| fallback.to_owned())
    };
    let sleep_locks =
        obstacles
            .inhibitors_of(InhibitedOperation::Sleep)
            .flat_map(move |inhibitor| {
                let why = stated_reason(&inhibitor.reason, "holds a sleep lock");
                [(IdleAction::Sleep, true), (IdleAction::Hibernate, false)].map(
                    |(pause, rises_to_panel)| PauseBlocker {
                        pause,
                        holder: inhibitor.holder.clone(),
                        why: why.clone(),
                        cause: BlockerCause::SleepLock,
                        rises_to_panel,
                    },
                )
            });
    let shutdown_locks =
        obstacles
            .inhibitors_of(InhibitedOperation::Shutdown)
            .map(move |inhibitor| PauseBlocker {
                pause: IdleAction::PowerOff,
                holder: inhibitor.holder.clone(),
                why: stated_reason(&inhibitor.reason, "holds a shutdown lock"),
                cause: BlockerCause::ShutdownLock,
                rises_to_panel: true,
            });
    let idle_locks = obstacles
        .inhibitors_of(InhibitedOperation::Idle)
        .map(move |inhibitor| PauseBlocker {
            pause: IdleAction::Sleep,
            holder: inhibitor.holder.clone(),
            why: stated_reason(&inhibitor.reason, "holds an idle lock"),
            cause: BlockerCause::LogindIdleLock,
            rises_to_panel: obstacles.logind_idle_action_enabled,
        });
    sleep_locks.chain(shutdown_locks).chain(idle_locks)
}

fn no_sleep_state_blocker(obstacles: &SleepObstacles) -> Option<PauseBlocker> {
    let no_lock_explains_it = obstacles.inhibitors_of(InhibitedOperation::Sleep).count() == 0;
    (no_lock_explains_it
        && !obstacles.sleep_order.is_empty()
        && obstacles.sleep_state_that_runs().is_none())
    .then(|| PauseBlocker {
        pause: IdleAction::Sleep,
        holder: "logind".to_owned(),
        why: "no configured sleep state is available".to_owned(),
        cause: BlockerCause::LogindRefusal,
        rises_to_panel: true,
    })
}

fn screensaver_blockers<'a>(
    idle_inhibitors: &'a [IdleInhibitor],
    armed_triggers: &'a [ActionTrigger],
) -> impl Iterator<Item = PauseBlocker> + 'a {
    let first_to_fire = armed_triggers
        .iter()
        .min_by_key(|trigger| trigger.after)
        .map(|trigger| trigger.tag);
    idle_inhibitors.iter().flat_map(move |inhibitor| {
        armed_triggers.iter().map(move |trigger| PauseBlocker {
            pause: trigger.tag,
            holder: inhibitor.holder.clone(),
            why: inhibitor
                .reason
                .clone()
                .unwrap_or_else(|| "asked the compositor to hold idle timers".to_owned()),
            cause: BlockerCause::ScreenSaverHold,
            rises_to_panel: Some(trigger.tag) == first_to_fire,
        })
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sottomano_desktop_sources::login_session_presence::{
        idle_session_actions::IdleAction,
        kernel_sleep_obstacles::{
            BlockingInhibitor, InhibitedOperation, KernelHibernation, LogindCapability,
            SleepObstacles, SleepState, SleepStateSupport,
        },
        screensaver_bus_idle_inhibitors::IdleInhibitor,
    };

    use super::{BlockerCause, PauseBlocker, PauseFacts, pause_blockers, sleep_state_answer};
    use crate::widgets::idle::ActionTrigger;

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
            resume_device_configured: true,
            blocking_inhibitors: Vec::new(),
            logind_idle_action_enabled: false,
            scheduled_shutdown: None,
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
            kernel_hibernation: KernelHibernation::DisabledBySecretMemory(vec![
                "Bitwarden".to_owned(),
            ]),
            ..clear_obstacles()
        }
    }

    fn blockers_of(obstacles: &SleepObstacles, hibernate_timer_set: bool) -> Vec<PauseBlocker> {
        pause_blockers(&PauseFacts {
            obstacles: Some(obstacles),
            idle_inhibitors: &[],
            armed_triggers: &[],
            hibernate_timer_set,
        })
    }

    fn lock_of(operation: InhibitedOperation) -> BlockingInhibitor {
        BlockingInhibitor {
            holder: "Backup".to_owned(),
            operation,
            reason: Some("Backup running".to_owned()),
        }
    }

    #[test]
    fn nothing_blocks_a_clear_machine() {
        assert!(blockers_of(&clear_obstacles(), false).is_empty());
    }

    #[test]
    fn secret_memory_names_the_exact_app_under_hibernate_only() {
        let blockers = blockers_of(&secret_memory_obstacles(), false);
        assert_eq!(blockers.len(), 1);
        assert_eq!(blockers[0].pause, IdleAction::Hibernate);
        assert_eq!(blockers[0].holder, "Bitwarden");
        assert_eq!(blockers[0].cause, BlockerCause::KernelSecretMemory);
        assert!(blockers[0].rises_to_panel);
    }

    #[test]
    fn without_the_helper_no_app_is_named() {
        let obstacles = SleepObstacles {
            kernel_hibernation: KernelHibernation::DisabledWithoutInspection,
            ..secret_memory_obstacles()
        };
        let blockers = blockers_of(&obstacles, false);
        assert_eq!(blockers[0].holder, "Kernel");
        assert_eq!(blockers[0].cause, BlockerCause::KernelUninspected);
    }

    #[test]
    fn a_machine_never_set_up_for_hibernation_stays_off_the_panel() {
        let obstacles = SleepObstacles {
            resume_device_configured: false,
            ..secret_memory_obstacles()
        };
        assert!(!blockers_of(&obstacles, false)[0].rises_to_panel);
        assert!(blockers_of(&obstacles, true)[0].rises_to_panel);
    }

    #[test]
    fn a_sleep_lock_raises_standby_but_only_lists_under_hibernate() {
        let obstacles = SleepObstacles {
            blocking_inhibitors: vec![lock_of(InhibitedOperation::Sleep)],
            ..clear_obstacles()
        };
        let blockers = blockers_of(&obstacles, false);
        let rising: Vec<IdleAction> = blockers
            .iter()
            .filter(|blocker| blocker.rises_to_panel)
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
            blocking_inhibitors: vec![lock_of(InhibitedOperation::Shutdown)],
            ..clear_obstacles()
        };
        let blockers = blockers_of(&obstacles, false);
        assert_eq!(blockers[0].pause, IdleAction::PowerOff);
        assert!(blockers[0].rises_to_panel);
    }

    #[test]
    fn a_logind_idle_lock_only_rises_when_logind_has_an_idle_action() {
        let without_action = SleepObstacles {
            blocking_inhibitors: vec![lock_of(InhibitedOperation::Idle)],
            ..clear_obstacles()
        };
        let with_action = SleepObstacles {
            logind_idle_action_enabled: true,
            ..without_action.clone()
        };
        assert!(!blockers_of(&without_action, false)[0].rises_to_panel);
        assert!(blockers_of(&with_action, false)[0].rises_to_panel);
    }

    #[test]
    fn a_screensaver_hold_lists_every_armed_timer_and_raises_the_first_to_fire() {
        let idle_inhibitors = [IdleInhibitor {
            holder: "Firefox".to_owned(),
            reason: Some("Playing video".to_owned()),
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
        let blockers = pause_blockers(&PauseFacts {
            obstacles: None,
            idle_inhibitors: &idle_inhibitors,
            armed_triggers: &armed_triggers,
            hibernate_timer_set: false,
        });
        assert_eq!(blockers.len(), 2);
        let rising: Vec<IdleAction> = blockers
            .iter()
            .filter(|blocker| blocker.rises_to_panel)
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
            blocking_inhibitors: vec![lock_of(InhibitedOperation::Sleep)],
            ..clear_obstacles()
        };
        assert_eq!(
            sleep_state_answer(&obstacles, SleepState::Suspend, None),
            "blocked by a sleep lock from Backup"
        );
        let blockers = blockers_of(&obstacles, false);
        assert!(blockers.iter().all(|blocker| blocker.holder == "Backup"));
    }

    #[test]
    fn a_kernel_disabled_state_keeps_the_kernel_reason_during_a_sleep_lock() {
        let obstacles = SleepObstacles {
            blocking_inhibitors: vec![lock_of(InhibitedOperation::Sleep)],
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
        assert_eq!(blockers_of(&obstacles, false)[0].holder, "logind");
    }
}
