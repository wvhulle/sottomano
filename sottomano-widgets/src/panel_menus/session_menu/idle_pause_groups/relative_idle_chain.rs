use std::time::Duration;

use sottomano_desktop_sources::login_session_presence::{
    idle_session_actions::IdleAction, session_power_actions::SessionAction,
};

use crate::widgets::idle::{ActionTrigger, ArmedIdleChain};

#[cfg(test)]
mod relative_idle_chain_tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageSetting {
    Off,
    Unavailable,
    After(Duration),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuspendThenHibernateSupport {
    Permitted,
    Refused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainBase {
    LastInput,
    Stage(IdleAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageSchedule {
    Off,
    Unavailable,
    AfterIdle {
        idle_for: Duration,
        performs: SessionAction,
    },
    AsleepAfterStandby {
        asleep_for: Duration,
    },
    UnreachableWhileAsleep,
    ZeroWithoutEarlierStage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledAfterIdle {
    Never,
    Disabled,
    After(Duration),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HibernateDelayDemand {
    Wanted(Duration),
    Indifferent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedStage {
    pub counted_from: ChainBase,
    pub schedule: StageSchedule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedIdleChain {
    stages: [ResolvedStage; 5],
    standby_performs: SessionAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MachineWakefulness {
    Awake,
    Asleep,
}

fn stage_index(stage: IdleAction) -> usize {
    match stage {
        IdleAction::Lock => 0,
        IdleAction::ScreenOff => 1,
        IdleAction::Sleep => 2,
        IdleAction::Hibernate => 3,
        IdleAction::PowerOff => 4,
    }
}

fn standby_verb(
    hibernate_setting: StageSetting,
    suspend_then_hibernate: SuspendThenHibernateSupport,
) -> SessionAction {
    match (hibernate_setting, suspend_then_hibernate) {
        (StageSetting::After(Duration::ZERO), _) => SessionAction::Hibernate,
        (StageSetting::After(_), SuspendThenHibernateSupport::Permitted) => {
            SessionAction::SuspendThenHibernate
        }
        (StageSetting::After(_), SuspendThenHibernateSupport::Refused)
        | (StageSetting::Off | StageSetting::Unavailable, _) => SessionAction::Suspend,
    }
}

impl ResolvedIdleChain {
    #[must_use]
    pub fn resolve(
        settings: [StageSetting; 5],
        suspend_then_hibernate: SuspendThenHibernateSupport,
    ) -> Self {
        let hibernate_setting = settings[stage_index(IdleAction::Hibernate)];
        let standby_performs = standby_verb(hibernate_setting, suspend_then_hibernate);
        let mut idle_so_far = Duration::ZERO;
        let mut counted_from = ChainBase::LastInput;
        let mut wakefulness = MachineWakefulness::Awake;
        let stages = IdleAction::ALL.map(|stage| {
            let stage_base = counted_from;
            let schedule = match (settings[stage_index(stage)], wakefulness, stage) {
                (StageSetting::Off, _, _) => StageSchedule::Off,
                (StageSetting::Unavailable, _, _) => StageSchedule::Unavailable,
                (
                    StageSetting::After(asleep_for),
                    MachineWakefulness::Asleep,
                    IdleAction::Hibernate,
                ) if counted_from == ChainBase::Stage(IdleAction::Sleep) => {
                    match standby_performs {
                        SessionAction::Suspend => StageSchedule::Unavailable,
                        _ => StageSchedule::AsleepAfterStandby { asleep_for },
                    }
                }
                (StageSetting::After(_), MachineWakefulness::Asleep, _) => {
                    StageSchedule::UnreachableWhileAsleep
                }
                (StageSetting::After(Duration::ZERO), MachineWakefulness::Awake, _)
                    if counted_from == ChainBase::LastInput =>
                {
                    StageSchedule::ZeroWithoutEarlierStage
                }
                (StageSetting::After(offset), MachineWakefulness::Awake, _) => {
                    idle_so_far += offset;
                    counted_from = ChainBase::Stage(stage);
                    if matches!(stage, IdleAction::Sleep | IdleAction::Hibernate) {
                        wakefulness = MachineWakefulness::Asleep;
                    }
                    StageSchedule::AfterIdle {
                        idle_for: idle_so_far,
                        performs: match stage {
                            IdleAction::Sleep => standby_performs,
                            IdleAction::Lock
                            | IdleAction::ScreenOff
                            | IdleAction::Hibernate
                            | IdleAction::PowerOff => stage.session_action(),
                        },
                    }
                }
            };
            ResolvedStage {
                counted_from: stage_base,
                schedule,
            }
        });
        Self {
            stages,
            standby_performs,
        }
    }

    #[must_use]
    pub fn stage(&self, stage: IdleAction) -> ResolvedStage {
        self.stages[stage_index(stage)]
    }

    #[must_use]
    pub fn armed(&self) -> ArmedIdleChain {
        ArmedIdleChain {
            triggers: IdleAction::ALL
                .into_iter()
                .filter_map(|stage| match self.stage(stage).schedule {
                    StageSchedule::AfterIdle { idle_for, .. } => Some(ActionTrigger {
                        after: idle_for,
                        tag: stage,
                    }),
                    StageSchedule::Off
                    | StageSchedule::Unavailable
                    | StageSchedule::AsleepAfterStandby { .. }
                    | StageSchedule::UnreachableWhileAsleep
                    | StageSchedule::ZeroWithoutEarlierStage => None,
                })
                .collect(),
            standby_performs: self.standby_performs,
        }
    }

    #[must_use]
    pub fn hibernate_delay_demand(&self) -> HibernateDelayDemand {
        match self.stage(IdleAction::Hibernate).schedule {
            StageSchedule::AsleepAfterStandby { asleep_for } if !asleep_for.is_zero() => {
                HibernateDelayDemand::Wanted(asleep_for)
            }
            _ => HibernateDelayDemand::Indifferent,
        }
    }

    #[must_use]
    pub fn scheduled(&self, action: SessionAction) -> ScheduledAfterIdle {
        let performing_stage =
            IdleAction::ALL
                .into_iter()
                .find(|stage| match self.stage(*stage).schedule {
                    StageSchedule::AfterIdle { performs, .. } => performs == action,
                    _ => *stage != IdleAction::Sleep && stage.session_action() == action,
                });
        let Some(stage) = performing_stage else {
            return ScheduledAfterIdle::Never;
        };
        match self.stage(stage).schedule {
            StageSchedule::AfterIdle { idle_for, .. } => ScheduledAfterIdle::After(idle_for),
            StageSchedule::Off
            | StageSchedule::Unavailable
            | StageSchedule::AsleepAfterStandby { .. }
            | StageSchedule::UnreachableWhileAsleep
            | StageSchedule::ZeroWithoutEarlierStage => ScheduledAfterIdle::Disabled,
        }
    }
}
