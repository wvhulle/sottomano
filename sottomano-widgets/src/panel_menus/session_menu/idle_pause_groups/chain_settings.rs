use iced::Task;
use sottomano_desktop_sinks::login_session_presence::hibernate_delay_writes::apply_hibernate_delay;
use sottomano_desktop_sources::login_session_presence::{
    idle_session_actions::IdleAction,
    kernel_sleep_obstacles::{LogindCapability, SleepState},
    system_hibernate_delay::HibernateDelay,
};

use super::{
    HibernateDelayDemand, IdlePauseGroups, IdlePauseMessage, ResolvedIdleChain, StageSetting,
    SuspendThenHibernateSupport, chain_stage_rows::HibernateDelayApply,
    stage_schedule_cards::ScreenOffOverride,
};
use crate::widgets::idle::timer::{ActionAvailability, IdleTimer};

pub(super) fn capability_availability(capability: Option<LogindCapability>) -> ActionAvailability {
    match capability {
        Some(LogindCapability::NotPermitted | LogindCapability::Unsupported) => {
            ActionAvailability::Unavailable
        }
        Some(LogindCapability::Allowed | LogindCapability::AfterAuthorisation) | None => {
            ActionAvailability::Available
        }
    }
}

impl IdlePauseGroups {
    pub(super) fn apply_wanted_hibernate_delay(&mut self) -> Task<IdlePauseMessage> {
        let HibernateDelayDemand::Wanted(wanted_delay) = self.idle_chain().hibernate_delay_demand()
        else {
            return Task::none();
        };
        if self.hibernate_delay.get() == Some(&HibernateDelay::Configured(wanted_delay)) {
            self.hibernate_delay_apply = HibernateDelayApply::Settled;
            return Task::none();
        }
        self.hibernate_delay_apply = HibernateDelayApply::Applying;
        Task::perform(
            apply_hibernate_delay(wanted_delay),
            IdlePauseMessage::HibernateDelayApplied,
        )
    }

    fn suspend_then_hibernate_support(&self) -> SuspendThenHibernateSupport {
        let capability = self
            .sleep_obstacles
            .get()
            .map(|obstacles| obstacles.sleep_state_capability(SleepState::SuspendThenHibernate));
        match capability_availability(capability) {
            ActionAvailability::Available => SuspendThenHibernateSupport::Permitted,
            ActionAvailability::Unavailable => SuspendThenHibernateSupport::Refused,
        }
    }

    pub(super) fn resolved_chain_with(
        &self,
        setting_of: impl Fn(&IdleTimer) -> StageSetting,
    ) -> ResolvedIdleChain {
        let settings = IdleAction::ALL.map(|stage| {
            let stage_setting = setting_of(&self.pause(stage).timer);
            let ScreenOffOverride::QuietHours(quiet_offset) = self.screen_off_override else {
                return stage_setting;
            };
            match (stage, stage_setting) {
                (IdleAction::ScreenOff, StageSetting::After(_) | StageSetting::Off) => {
                    StageSetting::After(quiet_offset)
                }
                _ => stage_setting,
            }
        });
        ResolvedIdleChain::resolve(settings, self.suspend_then_hibernate_support())
    }

    #[must_use]
    pub fn idle_chain(&self) -> ResolvedIdleChain {
        self.resolved_chain_with(IdleTimer::setting)
    }
}
