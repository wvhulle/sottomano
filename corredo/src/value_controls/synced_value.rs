use std::mem;

#[must_use = "perform the write and report the outcome via `absolute_write_finished`, \
              `relative_write_resolved` or `relative_write_failed`, or the value never advances"]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValueWrite {
    Absolute(f32),
    Relative(f32),
}

#[derive(Debug, Clone, Copy)]
pub struct Nudge {
    pub step_delta: f32,
    pub resulting_value: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceTracking {
    FollowingSource,
    Diverging,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DragWrites {
    #[default]
    WhileDragging,
    OnRelease,
}

#[derive(Debug, Default)]
pub struct SyncedValue {
    drag_writes: DragWrites,
    source_feedback: SourceFeedback,
    user_request: UserRequest,
    outgoing_write: OutgoingWrite,
}

#[derive(Debug, Default)]
struct SourceFeedback {
    reported_value: f32,
    accepted_value: f32,
}

#[derive(Debug, Default)]
enum UserRequest {
    #[default]
    FollowingSource,
    DraggingTo {
        requested_value: f32,
    },
    AwaitingArrival {
        requested_value: f32,
    },
}

#[derive(Debug, Default)]
enum OutgoingWrite {
    #[default]
    NothingOutstanding,
    AwaitingConfirmation,
    AwaitingConfirmationWithQueuedNudge {
        queued_step_delta: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceArrival {
    Reached,
    Lagging,
}

impl SourceFeedback {
    fn arrival_at(&self, value: f32, step: f32) -> SourceArrival {
        if (self.reported_value - value).abs() <= step / 2.0 {
            SourceArrival::Reached
        } else {
            SourceArrival::Lagging
        }
    }
}

impl UserRequest {
    fn requested_value(&self) -> Option<f32> {
        match self {
            Self::FollowingSource => None,
            Self::DraggingTo { requested_value } | Self::AwaitingArrival { requested_value } => {
                Some(*requested_value)
            }
        }
    }

    fn requesting(&self, requested_value: f32) -> Self {
        match self {
            Self::DraggingTo { .. } => Self::DraggingTo { requested_value },
            Self::FollowingSource | Self::AwaitingArrival { .. } => {
                Self::AwaitingArrival { requested_value }
            }
        }
    }
}

impl SyncedValue {
    #[must_use]
    pub fn writing_on_release() -> Self {
        Self {
            drag_writes: DragWrites::OnRelease,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn shown_value(&self) -> f32 {
        self.user_request
            .requested_value()
            .unwrap_or(self.source_feedback.reported_value)
    }

    #[must_use]
    pub fn requested_value(&self) -> Option<f32> {
        self.user_request.requested_value()
    }

    #[must_use]
    pub fn reported_value(&self) -> f32 {
        self.source_feedback.reported_value
    }

    #[must_use]
    pub fn lagging_reported_value(&self, step: f32) -> Option<f32> {
        let requested_value = self.user_request.requested_value()?;
        (self.source_feedback.arrival_at(requested_value, step) == SourceArrival::Lagging)
            .then_some(self.source_feedback.reported_value)
    }

    #[must_use]
    pub fn source_tracking(&self) -> SourceTracking {
        match (&self.user_request, &self.outgoing_write) {
            (UserRequest::FollowingSource, OutgoingWrite::NothingOutstanding) => {
                SourceTracking::FollowingSource
            }
            _ => SourceTracking::Diverging,
        }
    }

    pub fn drag_moved_to(&mut self, requested_value: f32) -> Option<ValueWrite> {
        self.user_request = UserRequest::DraggingTo { requested_value };
        match self.drag_writes {
            DragWrites::WhileDragging => self.write_requested_value(),
            DragWrites::OnRelease => None,
        }
    }

    pub fn drag_ended(&mut self) -> Option<ValueWrite> {
        if let UserRequest::DraggingTo { requested_value } = self.user_request {
            self.user_request = UserRequest::AwaitingArrival { requested_value };
        }
        self.write_requested_value()
    }

    pub fn scrolled_to(&mut self, requested_value: f32) -> Option<ValueWrite> {
        self.user_request = self.user_request.requesting(requested_value);
        self.write_requested_value()
    }

    pub fn nudged_by(&mut self, nudge: Nudge) -> Option<ValueWrite> {
        let Nudge {
            step_delta,
            resulting_value,
        } = nudge;
        self.user_request = self.user_request.requesting(resulting_value);
        match &mut self.outgoing_write {
            OutgoingWrite::NothingOutstanding => {
                self.outgoing_write = OutgoingWrite::AwaitingConfirmation;
                Some(ValueWrite::Relative(step_delta))
            }
            OutgoingWrite::AwaitingConfirmation => {
                self.outgoing_write = OutgoingWrite::AwaitingConfirmationWithQueuedNudge {
                    queued_step_delta: step_delta,
                };
                None
            }
            OutgoingWrite::AwaitingConfirmationWithQueuedNudge { queued_step_delta } => {
                *queued_step_delta += step_delta;
                None
            }
        }
    }

    pub fn absolute_write_finished<WriteFailure>(
        &mut self,
        written_value: f32,
        write_outcome: &Result<(), WriteFailure>,
    ) -> Option<ValueWrite> {
        if let Ok(()) = write_outcome {
            self.source_feedback.accepted_value = written_value;
        }
        match self.take_queued_nudge() {
            Some(queued_step_delta) => Some(self.send_nudge(queued_step_delta)),
            None => self.write_requested_value(),
        }
    }

    pub fn relative_write_failed(&mut self) {
        self.take_queued_nudge();
        self.user_request = UserRequest::FollowingSource;
    }

    pub fn relative_write_resolved(&mut self, value: f32) -> Option<ValueWrite> {
        let queued_nudge = self.take_queued_nudge();
        self.source_feedback = SourceFeedback {
            reported_value: value,
            accepted_value: value,
        };
        if let Some(queued_step_delta) = queued_nudge {
            self.user_request = self.user_request.requesting(value + queued_step_delta);
            Some(self.send_nudge(queued_step_delta))
        } else {
            self.user_request = UserRequest::FollowingSource;
            None
        }
    }

    pub fn source_reported(&mut self, reported_value: f32, step: f32) {
        self.source_feedback.reported_value = reported_value;
        if matches!(self.user_request, UserRequest::DraggingTo { .. })
            || !matches!(self.outgoing_write, OutgoingWrite::NothingOutstanding)
        {
            return;
        }
        if self
            .source_feedback
            .arrival_at(self.source_feedback.accepted_value, step)
            == SourceArrival::Reached
        {
            self.user_request = UserRequest::FollowingSource;
        } else if self.user_request.requested_value().is_none() {
            self.source_feedback.accepted_value = reported_value;
        }
    }

    pub fn follow_source(&mut self) {
        self.take_queued_nudge();
        self.user_request = UserRequest::FollowingSource;
    }

    fn take_queued_nudge(&mut self) -> Option<f32> {
        match mem::take(&mut self.outgoing_write) {
            OutgoingWrite::AwaitingConfirmationWithQueuedNudge { queued_step_delta } => {
                Some(queued_step_delta)
            }
            OutgoingWrite::NothingOutstanding | OutgoingWrite::AwaitingConfirmation => None,
        }
    }

    fn send_nudge(&mut self, queued_step_delta: f32) -> ValueWrite {
        self.outgoing_write = OutgoingWrite::AwaitingConfirmation;
        ValueWrite::Relative(queued_step_delta)
    }

    #[expect(
        clippy::float_cmp,
        reason = "a request equal to the accepted value was already written"
    )]
    fn write_requested_value(&mut self) -> Option<ValueWrite> {
        if !matches!(self.outgoing_write, OutgoingWrite::NothingOutstanding) {
            return None;
        }
        match self.user_request.requested_value() {
            Some(requested_value) if requested_value != self.source_feedback.accepted_value => {
                self.outgoing_write = OutgoingWrite::AwaitingConfirmation;
                Some(ValueWrite::Absolute(requested_value))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Nudge, SourceTracking, SyncedValue, ValueWrite};

    const STEP: f32 = 1.0;
    const ACCEPTED: Result<(), ()> = Ok(());

    fn assert_shown(synced: &SyncedValue, expected: f32) {
        assert!(
            (synced.shown_value() - expected).abs() < f32::EPSILON,
            "shown {} instead of {expected}",
            synced.shown_value()
        );
    }

    #[test]
    fn dragging_writes_once_and_holds_the_request_until_the_source_arrives() {
        let mut synced = SyncedValue::default();
        assert_eq!(synced.drag_moved_to(40.0), Some(ValueWrite::Absolute(40.0)));
        assert_eq!(synced.drag_moved_to(45.0), None);
        assert_eq!(
            synced.absolute_write_finished(40.0, &ACCEPTED),
            Some(ValueWrite::Absolute(45.0))
        );
        assert_eq!(synced.drag_ended(), None);
        assert_eq!(synced.absolute_write_finished(45.0, &ACCEPTED), None);
        synced.source_reported(20.0, STEP);
        assert_shown(&synced, 45.0);
        assert_eq!(synced.lagging_reported_value(STEP), Some(20.0));
        synced.source_reported(45.0, STEP);
        assert_eq!(synced.source_tracking(), SourceTracking::FollowingSource);
    }

    #[test]
    fn writing_on_release_waits_for_the_drag_to_end() {
        let mut synced = SyncedValue::writing_on_release();
        assert_eq!(synced.drag_moved_to(30.0), None);
        assert_shown(&synced, 30.0);
        assert_eq!(synced.drag_ended(), Some(ValueWrite::Absolute(30.0)));
    }

    #[test]
    fn nudges_queue_behind_an_unconfirmed_one() {
        let mut synced = SyncedValue::default();
        let nudge = |step_delta, resulting_value| Nudge {
            step_delta,
            resulting_value,
        };
        assert_eq!(
            synced.nudged_by(nudge(5.0, 55.0)),
            Some(ValueWrite::Relative(5.0))
        );
        assert_eq!(synced.nudged_by(nudge(5.0, 60.0)), None);
        assert_eq!(synced.nudged_by(nudge(5.0, 65.0)), None);
        assert_eq!(
            synced.relative_write_resolved(55.0),
            Some(ValueWrite::Relative(10.0))
        );
        assert_shown(&synced, 65.0);
        assert_eq!(synced.relative_write_resolved(65.0), None);
        assert_eq!(synced.source_tracking(), SourceTracking::FollowingSource);
    }

    #[test]
    fn following_the_source_drops_the_request() {
        let mut synced = SyncedValue::writing_on_release();
        synced.source_reported(10.0, STEP);
        assert_eq!(synced.drag_moved_to(90.0), None);
        synced.follow_source();
        assert_shown(&synced, 10.0);
        assert_eq!(synced.source_tracking(), SourceTracking::FollowingSource);
    }
}
