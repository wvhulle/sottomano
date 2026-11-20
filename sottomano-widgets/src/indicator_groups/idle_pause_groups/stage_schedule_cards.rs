use std::time::Duration;

use sottomano_desktop_sources::login_session_presence::{
    idle_session_actions::IdleAction, system_hibernate_delay::HibernateDelay,
};

use super::{
    pause_blocker_indicators::pause_title,
    relative_idle_chain::{ChainBase, ResolvedStage, StageSchedule},
};
use crate::{
    clock::Clock,
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_time::LocalizedTimeFormat,
        value_cells::{LabeledValue, ValueCell},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScreenOffOverride {
    QuietHours(Duration),
    FollowsSlider,
}

pub(super) fn stage_caption(stage: IdleAction, resolved: ResolvedStage) -> &'static str {
    match (resolved.schedule, resolved.counted_from) {
        (StageSchedule::UnreachableWhileAsleep, _) => "never reached",
        (StageSchedule::Unavailable, _) => "unavailable",
        (StageSchedule::ZeroWithoutEarlierStage, _) => "not armed",
        (StageSchedule::AsleepAfterStandby { .. }, _) => "asleep for",
        (_, ChainBase::LastInput) => "after input",
        (_, ChainBase::Stage(IdleAction::Lock)) => "after lock",
        (_, ChainBase::Stage(IdleAction::ScreenOff)) => "after screen off",
        (_, ChainBase::Stage(IdleAction::Sleep)) => match stage {
            IdleAction::Hibernate => "asleep for",
            IdleAction::Lock | IdleAction::ScreenOff | IdleAction::Sleep | IdleAction::PowerOff => {
                "after standby"
            }
        },
        (_, ChainBase::Stage(IdleAction::Hibernate)) => "after hibernate",
        (_, ChainBase::Stage(IdleAction::PowerOff)) => "after power-down",
    }
}

fn counted_from_text(counted_from: ChainBase) -> String {
    match counted_from {
        ChainBase::LastInput => "last input".to_owned(),
        ChainBase::Stage(stage) => pause_title(stage).to_owned(),
    }
}

fn hibernate_delay_cell(system_delay: HibernateDelay) -> ValueCell {
    match system_delay {
        HibernateDelay::Configured(delay) => ValueCell::duration(delay),
        HibernateDelay::BatteryEstimated => "estimated from battery drain".into(),
    }
}

pub(super) fn schedule_section(
    section_title: &'static str,
    resolved: ResolvedStage,
    screen_off_override: ScreenOffOverride,
    system_delay: Option<HibernateDelay>,
    clock: Clock,
) -> Section {
    let counted_from = LabeledValue::new("Counted from", counted_from_text(resolved.counted_from));
    let schedule_rows = match resolved.schedule {
        StageSchedule::Off => vec![LabeledValue::new("State", "disabled")],
        StageSchedule::Unavailable => vec![LabeledValue::new("State", "unavailable")],
        StageSchedule::ZeroWithoutEarlierStage => vec![LabeledValue::new(
            "State",
            "not armed: +0 needs an earlier enabled step",
        )],
        StageSchedule::UnreachableWhileAsleep => vec![
            LabeledValue::new("State", "never reached: the machine is asleep by then"),
            counted_from,
        ],
        StageSchedule::AfterIdle { idle_for, performs } => {
            let label = format!("{} at", performs.verb());
            let fires_at = match chrono::Duration::from_std(idle_for) {
                Ok(delay) => LabeledValue::new(
                    label,
                    ValueCell::Time {
                        at: clock.utc() + delay,
                        format: LocalizedTimeFormat::TimeOfDay,
                    },
                ),
                Err(_) => LabeledValue::new(label, "beyond the calendar range"),
            };
            vec![
                fires_at,
                LabeledValue::new("Idle for", ValueCell::duration(idle_for)),
                counted_from,
                LabeledValue::new("Via", performs.mechanism()),
            ]
        }
        StageSchedule::AsleepAfterStandby { asleep_for } => {
            let system_row = system_delay
                .map(|delay| LabeledValue::new("System delay", hibernate_delay_cell(delay)));
            [
                LabeledValue::new("Asleep for", ValueCell::duration(asleep_for)),
                LabeledValue::new("Via", "systemd HibernateDelaySec"),
                LabeledValue::new("Applies to", "every suspend-then-hibernate, lid close too"),
            ]
            .into_iter()
            .chain(system_row)
            .collect()
        }
    };
    let quiet_hours_row = match screen_off_override {
        ScreenOffOverride::QuietHours(offset) => Some(LabeledValue::new(
            "Quiet hours",
            ValueCell::duration(offset),
        )),
        ScreenOffOverride::FollowsSlider => None,
    };
    Section::new(
        section_title,
        dictionary_table(schedule_rows.into_iter().chain(quiet_hours_row)),
    )
}
