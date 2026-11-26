use std::time::Duration;

use chrono::{DateTime, Local, TimeDelta};
use iced::{Task, task};
use tokio::time::sleep;

use super::{
    QuietScheduleGroup, QuietScheduleMessage,
    quiet_window::{QuietPhase, QuietScheduleAdherence},
};

const EDGE_TIMER_MARGIN: Duration = Duration::from_secs(1);

pub(super) struct ManualOverride {
    phase: QuietPhase,
    until: ManualOverrideEnd,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ManualOverrideEnd {
    ScheduleEdge(DateTime<Local>),
    NextToggle,
}

pub(super) enum QuietControl {
    Scheduled,
    Manual(ManualOverride),
}

pub(super) struct ArmedEdge {
    at: DateTime<Local>,
    _timer: task::Handle,
}

pub(super) enum EdgeTimer {
    Disarmed,
    Armed(ArmedEdge),
}

impl QuietScheduleGroup {
    pub(super) fn manual_at(&self, now: DateTime<Local>) -> Option<QuietPhase> {
        let QuietControl::Manual(manual) = &self.control else {
            return None;
        };
        match manual.until {
            ManualOverrideEnd::ScheduleEdge(until) if now >= until => None,
            ManualOverrideEnd::ScheduleEdge(_) | ManualOverrideEnd::NextToggle => {
                Some(manual.phase)
            }
        }
    }

    pub(super) fn phase_at(&self, now: DateTime<Local>) -> QuietPhase {
        if let Some(manual_phase) = self.manual_at(now) {
            manual_phase
        } else {
            match self.schedule_adherence {
                QuietScheduleAdherence::Followed => self.window().phase_at(now),
                QuietScheduleAdherence::Ignored => QuietPhase::Inactive,
            }
        }
    }

    pub(super) fn next_change(&self, now: DateTime<Local>) -> Option<DateTime<Local>> {
        let manual_expiry = match &self.control {
            QuietControl::Manual(ManualOverride {
                until: ManualOverrideEnd::ScheduleEdge(until),
                ..
            }) => Some(*until),
            QuietControl::Manual(_) | QuietControl::Scheduled => None,
        };
        let scheduled = match self.schedule_adherence {
            QuietScheduleAdherence::Followed => Some(self.window().next_edge(now)),
            QuietScheduleAdherence::Ignored => None,
        };
        [manual_expiry, scheduled].into_iter().flatten().min()
    }

    pub(super) fn toggle_quiet_now(&mut self, now: DateTime<Local>) {
        let phase = self.phase_at(now).opposite();
        let until = match self.schedule_adherence {
            QuietScheduleAdherence::Followed => ManualOverrideEnd::ScheduleEdge(
                self.window().next_transition_into(phase.opposite(), now),
            ),
            QuietScheduleAdherence::Ignored => ManualOverrideEnd::NextToggle,
        };
        self.control = QuietControl::Manual(ManualOverride { phase, until });
    }

    pub(super) fn rearm_edge(&mut self, now: DateTime<Local>) -> Task<QuietScheduleMessage> {
        let upcoming = self.next_change(now);
        let Some(at) = upcoming else {
            self.edge_timer = EdgeTimer::Disarmed;
            return Task::none();
        };
        if let EdgeTimer::Armed(armed) = &self.edge_timer
            && armed.at == at
        {
            return Task::none();
        }
        let remaining = (at - now)
            .max(TimeDelta::zero())
            .to_std()
            .expect("a non-negative time delta fits a std duration");
        let (timer, handle) = Task::future(sleep(remaining + EDGE_TIMER_MARGIN))
            .map(|()| QuietScheduleMessage::EdgeDue)
            .abortable();
        self.edge_timer = EdgeTimer::Armed(ArmedEdge {
            at,
            _timer: handle.abort_on_drop(),
        });
        timer
    }
}
