use std::mem;

#[must_use = "perform the write and report the outcome via `absolute_write_finished` or \
              `relative_write_finished`, or the setting never advances"]
pub enum Write {
    Absolute(f32),
    Relative(f32),
}

#[derive(Debug, Clone, Copy)]
pub struct Nudge {
    pub step_delta: f32,
    pub resulting_value: f32,
}

#[derive(Default)]
pub struct DeviceSetting {
    device_feedback: DeviceFeedback,
    user_request: UserRequest,
    outgoing_write: OutgoingWrite,
}

#[derive(Default)]
struct DeviceFeedback {
    reported_value: f32,
    accepted_value: f32,
}

#[derive(Default)]
enum UserRequest {
    #[default]
    FollowingDevice,
    DraggingTo {
        requested_value: f32,
    },
    AwaitingArrival {
        requested_value: f32,
    },
}

#[derive(Default)]
enum OutgoingWrite {
    #[default]
    NothingOutstanding,
    AwaitingConfirmation,
    AwaitingConfirmationWithQueuedNudge {
        queued_step_delta: f32,
    },
}

impl DeviceFeedback {
    fn has_reached(&self, value: f32, step: f32) -> bool {
        (self.reported_value - value).abs() <= step / 2.0
    }
}

impl UserRequest {
    fn requested_value(&self) -> Option<f32> {
        match self {
            Self::FollowingDevice => None,
            Self::DraggingTo { requested_value } | Self::AwaitingArrival { requested_value } => {
                Some(*requested_value)
            }
        }
    }

    fn requesting(&self, requested_value: f32) -> Self {
        match self {
            Self::DraggingTo { .. } => Self::DraggingTo { requested_value },
            Self::FollowingDevice | Self::AwaitingArrival { .. } => {
                Self::AwaitingArrival { requested_value }
            }
        }
    }

    fn is_dragging(&self) -> bool {
        matches!(self, Self::DraggingTo { .. })
    }
}

impl OutgoingWrite {
    fn is_outstanding(&self) -> bool {
        !matches!(self, Self::NothingOutstanding)
    }
}

impl DeviceSetting {
    pub fn wanted_value(&self) -> f32 {
        self.user_request
            .requested_value()
            .unwrap_or(self.device_feedback.reported_value)
    }

    pub fn lagging_device_value(&self, step: f32) -> Option<f32> {
        let requested_value = self.user_request.requested_value()?;
        (!self.device_feedback.has_reached(requested_value, step))
            .then_some(self.device_feedback.reported_value)
    }

    pub fn drag_moved_to(&mut self, requested_value: f32) -> Option<Write> {
        self.user_request = UserRequest::DraggingTo { requested_value };
        self.write_requested_value()
    }

    pub fn drag_ended(&mut self) -> Option<Write> {
        if let UserRequest::DraggingTo { requested_value } = self.user_request {
            self.user_request = UserRequest::AwaitingArrival { requested_value };
        }
        self.write_requested_value()
    }

    pub fn scrolled_to(&mut self, requested_value: f32) -> Option<Write> {
        self.user_request = self.user_request.requesting(requested_value);
        self.write_requested_value()
    }

    pub fn nudged_by(&mut self, nudge: Nudge) -> Option<Write> {
        let Nudge {
            step_delta,
            resulting_value,
        } = nudge;
        self.user_request = self.user_request.requesting(resulting_value);
        match &mut self.outgoing_write {
            OutgoingWrite::NothingOutstanding => {
                self.outgoing_write = OutgoingWrite::AwaitingConfirmation;
                Some(Write::Relative(step_delta))
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

    pub fn absolute_write_finished(
        &mut self,
        written_value: f32,
        succeeded: bool,
    ) -> Option<Write> {
        if succeeded {
            self.device_feedback.accepted_value = written_value;
        }
        match self.take_queued_nudge() {
            Some(queued_step_delta) => Some(self.send_nudge(queued_step_delta)),
            None => self.write_requested_value(),
        }
    }

    pub fn relative_write_finished(&mut self, resolved_value: Option<f32>) -> Option<Write> {
        let queued_nudge = self.take_queued_nudge();
        let Some(value) = resolved_value else {
            self.user_request = UserRequest::FollowingDevice;
            return None;
        };
        self.device_feedback = DeviceFeedback {
            reported_value: value,
            accepted_value: value,
        };
        if let Some(queued_step_delta) = queued_nudge {
            self.user_request = self.user_request.requesting(value + queued_step_delta);
            Some(self.send_nudge(queued_step_delta))
        } else {
            self.user_request = UserRequest::FollowingDevice;
            None
        }
    }

    pub fn device_reported(&mut self, reported_value: f32, step: f32) {
        self.device_feedback.reported_value = reported_value;
        if self.user_request.is_dragging() || self.outgoing_write.is_outstanding() {
            return;
        }
        if self
            .device_feedback
            .has_reached(self.device_feedback.accepted_value, step)
        {
            self.user_request = UserRequest::FollowingDevice;
        } else if self.user_request.requested_value().is_none() {
            self.device_feedback.accepted_value = reported_value;
        }
    }

    fn take_queued_nudge(&mut self) -> Option<f32> {
        match mem::take(&mut self.outgoing_write) {
            OutgoingWrite::AwaitingConfirmationWithQueuedNudge { queued_step_delta } => {
                Some(queued_step_delta)
            }
            OutgoingWrite::NothingOutstanding | OutgoingWrite::AwaitingConfirmation => None,
        }
    }

    fn send_nudge(&mut self, queued_step_delta: f32) -> Write {
        self.outgoing_write = OutgoingWrite::AwaitingConfirmation;
        Write::Relative(queued_step_delta)
    }

    #[allow(clippy::float_cmp)]
    fn write_requested_value(&mut self) -> Option<Write> {
        if self.outgoing_write.is_outstanding() {
            return None;
        }
        match self.user_request.requested_value() {
            Some(requested_value) if requested_value != self.device_feedback.accepted_value => {
                self.outgoing_write = OutgoingWrite::AwaitingConfirmation;
                Some(Write::Absolute(requested_value))
            }
            _ => None,
        }
    }
}
