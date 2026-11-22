use std::{error::Error, sync::Arc};

use corredo::value_controls::{
    SliderEffect, SliderGesture, SliderMessage, SourceTracking, ValueWrite, WriteOutcome,
};
use iced::Task;

use super::{Action, ControlMessage, ControlWriteFailure, SliderControl, SliderLevelAnnouncing};
use crate::widgets::announced_level::LevelChangeOrigin;

impl ControlMessage {
    pub fn commit<WriteError: Error + Send + Sync + 'static>(
        label: &'static str,
        value: f32,
        write: impl Future<Output = Result<(), WriteError>> + Send + 'static,
    ) -> Task<ControlMessage> {
        let device_write = async move {
            write
                .await
                .map_err(|write_error| ControlWriteFailure::Rejected(Arc::new(write_error)))
        };
        Task::perform(device_write, move |write_outcome| {
            if let Err(write_failure) = &write_outcome {
                tracing::warn!("{label}: {write_failure}");
            }
            ControlMessage::DeviceAccepted {
                value,
                write_outcome,
            }
        })
    }

    pub fn persisted<StoreFailure: Error + Send + Sync + 'static>(
        value: f32,
        store_outcome: Result<(), StoreFailure>,
    ) -> ControlMessage {
        ControlMessage::Persisted {
            value,
            write_outcome: store_outcome
                .map_err(|store_failure| ControlWriteFailure::Rejected(Arc::new(store_failure))),
        }
    }
}

impl SliderControl {
    #[allow(clippy::needless_pass_by_value)]
    pub fn update(&mut self, msg: ControlMessage) -> Action {
        match msg {
            ControlMessage::Press => self.gestured(SliderGesture::Toggle),
            ControlMessage::Scroll(delta) => self.gestured(SliderGesture::Scroll(delta)),
            ControlMessage::Set(position) => self.gestured(SliderGesture::Drag(position)),
            ControlMessage::Release => self.gestured(SliderGesture::Release),
            ControlMessage::DeviceAccepted {
                value,
                write_outcome,
            } => into_action(
                self.expanding_slider
                    .update(SliderMessage::AbsoluteWritten {
                        value,
                        outcome: slider_write_outcome(&write_outcome),
                    }),
            ),
            ControlMessage::Persisted {
                value,
                write_outcome,
            } => {
                let follow_up = self
                    .expanding_slider
                    .update(SliderMessage::AbsoluteWritten {
                        value,
                        outcome: slider_write_outcome(&write_outcome),
                    });
                match write_outcome {
                    Ok(()) => {
                        self.expanding_slider
                            .update(SliderMessage::SourceReported(value));
                    }
                    Err(store_failure) => tracing::warn!("slider value not saved: {store_failure}"),
                }
                into_action(follow_up)
            }
            ControlMessage::Adjusted(value) => into_action(
                self.expanding_slider
                    .update(SliderMessage::RelativeWritten(value)),
            ),
            ControlMessage::AdjustFailed => {
                self.expanding_slider.update(SliderMessage::RelativeFailed);
                Action::None
            }
            ControlMessage::Sample(sample) => {
                let tracking_before = self.expanding_slider.synced_value.source_tracking();
                let shown_before = self.expanding_slider.synced_value.shown_value();
                self.expanding_slider
                    .update(SliderMessage::SourceReported(sample));
                let moved = (self.expanding_slider.synced_value.shown_value() - shown_before).abs()
                    >= self.expanding_slider.slider_step / 2.0;
                let change_origin = match tracking_before {
                    SourceTracking::FollowingSource if moved => LevelChangeOrigin::OutsidePanel,
                    SourceTracking::FollowingSource | SourceTracking::Diverging => {
                        LevelChangeOrigin::FromPanel
                    }
                };
                if let SliderLevelAnnouncing::Announcing(announcement) = &mut self.level_announcing
                {
                    announcement.device_reported(change_origin);
                }
                Action::None
            }
            ControlMessage::Level(level) => {
                self.live = level;
                Action::None
            }
            ControlMessage::Activity(slider_activity) => {
                let change_origin = if slider_activity == self.slider_activity {
                    LevelChangeOrigin::FromPanel
                } else {
                    LevelChangeOrigin::OutsidePanel
                };
                self.slider_activity = slider_activity;
                if let SliderLevelAnnouncing::Announcing(announcement) = &mut self.level_announcing
                {
                    announcement.device_reported(change_origin);
                }
                Action::None
            }
            ControlMessage::Right => Action::Right,
        }
    }

    fn gestured(&mut self, gesture: SliderGesture) -> Action {
        into_action(
            self.expanding_slider
                .update(SliderMessage::Gesture(gesture)),
        )
    }
}

impl From<SliderGesture> for ControlMessage {
    fn from(gesture: SliderGesture) -> Self {
        match gesture {
            SliderGesture::Toggle => Self::Press,
            SliderGesture::Scroll(delta) => Self::Scroll(delta),
            SliderGesture::Drag(position) => Self::Set(position),
            SliderGesture::Release => Self::Release,
            SliderGesture::SecondaryPress => Self::Right,
        }
    }
}

fn slider_write_outcome(write_outcome: &Result<(), ControlWriteFailure>) -> WriteOutcome {
    match write_outcome {
        Ok(()) => WriteOutcome::Accepted,
        Err(_) => WriteOutcome::Rejected,
    }
}

fn into_action(slider_effect: SliderEffect) -> Action {
    match slider_effect {
        SliderEffect::Write(ValueWrite::Absolute(value)) => Action::Commit(value),
        SliderEffect::Write(ValueWrite::Relative(delta)) => Action::Adjust(delta),
        SliderEffect::Opened => Action::Opened,
        SliderEffect::SecondaryPressed => Action::Right,
        SliderEffect::Closed | SliderEffect::Unchanged => Action::None,
    }
}
