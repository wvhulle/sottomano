pub(super) mod sleep_obstacle_blockers;
pub(super) mod sleep_state_answers;

use std::{borrow::Cow, iter::once};

use chrono::Locale;
use iced::alignment::Horizontal;
use sottomano_desktop_sources::login_session_presence::{
    idle_session_actions::IdleAction,
    kernel_sleep_obstacles::{ScheduledShutdown, ShutdownMoment, WindowOwnership},
};

use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        ongoing_toast::moment_text,
    },
};

#[cfg(test)]
mod pause_blocker_tests;

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
                "Kernel",
                "The kernel disables hibernation while these apps hold secret memory.",
            ),
            BlockerCause::KernelBootFlag => (
                "Kernel",
                "The kernel disables hibernation because of how it was booted.",
            ),
            BlockerCause::KernelLockdown => (
                "Kernel",
                "The kernel disables hibernation while lockdown is active.",
            ),
            BlockerCause::KernelUnidentified => (
                "Kernel",
                "Disabled by the kernel. No boot flag, lockdown or secret memory was found.",
            ),
            BlockerCause::KernelUninspected => (
                "Kernel",
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IdleActionBlocking {
    StopsArmedAction,
    LeavesActionRunning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PauseBlocker {
    pub pause: IdleAction,
    pub holder: String,
    pub why: String,
    pub cause: BlockerCause,
    pub action_blocking: IdleActionBlocking,
    pub holder_process: HolderProcessIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HolderProcessIdentity {
    Identified(HolderProcess),
    NotIdentified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HolderProcess {
    pub pid: u32,
    pub window_ownership: WindowOwnership,
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

impl PauseBlocker {
    pub(super) fn display_state(&self) -> IndicatorState {
        match (self.action_blocking, self.cause) {
            (IdleActionBlocking::StopsArmedAction, _) => IndicatorState::Critical,
            (
                IdleActionBlocking::LeavesActionRunning,
                BlockerCause::LogindIdleLock | BlockerCause::ScreenSaverHold,
            ) => IndicatorState::Idle,
            (IdleActionBlocking::LeavesActionRunning, _) => IndicatorState::Warning,
        }
    }

    pub(super) fn indicator(&self) -> GroupIndicator<'_> {
        GroupIndicator::glyph_only(
            self.holder.clone(),
            Phosphor::Warning(Weight::Regular).into(),
            self.display_state(),
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(self.blocked_phrase().into_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        blocker_sections(once(self)).into_iter().collect::<Vec<_>>()
    }
}

impl From<&PauseBlocker> for TableRecord<1> {
    fn from(pause_blocker: &PauseBlocker) -> Self {
        TableRecord::with_cells(
            pause_blocker.holder.clone(),
            [pause_blocker.why.clone().into()],
        )
    }
}

impl From<&PauseBlocker> for TableRecord<2> {
    fn from(blocker: &PauseBlocker) -> Self {
        let (pid_text, window_text) = match blocker.holder_process {
            HolderProcessIdentity::Identified(process) => (
                process.pid.to_string(),
                match process.window_ownership {
                    WindowOwnership::OwnsWindow => "open",
                    WindowOwnership::Windowless => "none",
                }
                .to_owned(),
            ),
            HolderProcessIdentity::NotIdentified => (String::new(), String::new()),
        };
        TableRecord::with_cells(
            blocker.holder.clone(),
            [pid_text.into(), window_text.into()],
        )
    }
}

pub(super) struct ScheduledShutdownIndicator<'view> {
    pub shutdown: &'view ScheduledShutdown,
}

impl ScheduledShutdownIndicator<'_> {
    pub(super) fn indicator<'indicator>(
        self,
        clock: Clock,
        locale: Locale,
    ) -> GroupIndicator<'indicator> {
        let name = match self.shutdown.kind.as_str() {
            "poweroff" | "dry-poweroff" => "Power off scheduled",
            "reboot" | "dry-reboot" => "Reboot scheduled",
            "halt" | "dry-halt" => "Halt scheduled",
            _ => "Shutdown scheduled",
        };
        let label = match self.shutdown.moment {
            ShutdownMoment::At(at) => ReadoutLabel::Labeled(moment_text(at, clock.utc(), locale)),
            ShutdownMoment::Unreported => ReadoutLabel::Unlabeled,
        };
        GroupIndicator::glyph_only(
            name,
            Phosphor::Clock(Weight::Regular).into(),
            IndicatorState::Warning,
        )
        .with_readout(IndicatorReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(|_| Vec::new()))
    }
}

pub(super) fn blocker_sections<'view>(
    blockers: impl IntoIterator<Item = &'view PauseBlocker>,
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
            let blocker_table = if cause == BlockerCause::KernelSecretMemory {
                headed_table(
                    [
                        ColumnHeading::titled("PID"),
                        ColumnHeading::titled("Window").align_x(Horizontal::Left),
                    ],
                    rows,
                )
            } else {
                headed_table(
                    [ColumnHeading::titled("Why").align_x(Horizontal::Left)],
                    rows,
                )
            };
            let (heading, caption) = cause.section_heading();
            Section::new(SectionTitle::captioned(heading, caption), blocker_table)
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TimerArming {
    Armed,
    Unarmed,
}
