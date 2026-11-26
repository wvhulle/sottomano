use chrono::{DateTime, TimeDelta, Utc};
use iced::{Task, task};
use tokio::time::sleep;

use super::{DarkModeGroup, DarkModeMessage, SWITCH_TIMER_MARGIN};
use crate::{
    clock::Clock, panel_menus::display_menu::dark_mode_group::auto_switch_indicator::ScheduleStatus,
};

pub(super) enum SwitchTimer {
    Unarmed,
    Armed(ArmedSwitch),
}

pub(super) struct ArmedSwitch {
    at: DateTime<Utc>,
    _timer: task::Handle,
}

impl DarkModeGroup {
    fn upcoming_switch(&self, clock: Clock) -> Option<DateTime<Utc>> {
        match self.auto_switch_toggle.status_at(clock.utc()) {
            ScheduleStatus::Following { upcoming, .. } => Some(upcoming.switch_time),
            ScheduleStatus::Disarmed
            | ScheduleStatus::AwaitingLocationFix
            | ScheduleStatus::NoSunEvents => None,
        }
    }

    pub(super) fn rearm_switch(&mut self, clock: Clock) -> Task<DarkModeMessage> {
        let Some(at) = self.upcoming_switch(clock) else {
            self.switch_timer = SwitchTimer::Unarmed;
            return Task::none();
        };
        if let SwitchTimer::Armed(armed) = &self.switch_timer
            && armed.at == at
        {
            return Task::none();
        }
        let remaining = (at - clock.utc())
            .max(TimeDelta::zero())
            .to_std()
            .expect("a non-negative time delta fits a std duration");
        let (timer, handle) = Task::future(sleep(remaining + SWITCH_TIMER_MARGIN))
            .map(|()| DarkModeMessage::SwitchDue)
            .abortable();
        self.switch_timer = SwitchTimer::Armed(ArmedSwitch {
            at,
            _timer: handle.abort_on_drop(),
        });
        timer
    }
}
