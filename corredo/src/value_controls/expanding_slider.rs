mod expanding_slider_view;

use std::{ops::RangeInclusive, time::Duration};

use iced::{Element, mouse::ScrollDelta};

use crate::{
    selection_scroll::wheel_step_direction,
    transitions::ExpansionState,
    value_controls::{
        SliderAccents,
        synced_value::{Nudge, SyncedValue, ValueWrite},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollWrites {
    Absolute,
    Relative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondaryPress {
    Ignored,
    Reported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteOutcome {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SliderGesture {
    Toggle,
    Scroll(ScrollDelta),
    Drag(f32),
    Release,
    SecondaryPress,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SliderMessage {
    Gesture(SliderGesture),
    AbsoluteWritten { value: f32, outcome: WriteOutcome },
    RelativeWritten(f32),
    RelativeFailed,
    SourceReported(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SliderEffect {
    Unchanged,
    Opened,
    Closed,
    SecondaryPressed,
    Write(ValueWrite),
}

pub struct SliderLook<'view> {
    pub icon_size_px: f32,
    pub accents: SliderAccents<'view>,
}

type ElementPart<'view, Message> = Box<dyn Fn() -> Element<'view, Message> + 'view>;

pub struct ExpandingSliderParts<'view, Message> {
    pub collapsed: ElementPart<'view, Message>,
    pub face: ElementPart<'view, Message>,
    pub readout: ElementPart<'view, Message>,
    pub look: Box<dyn Fn() -> SliderLook<'view> + 'view>,
    pub decorate:
        Box<dyn Fn(Element<'view, Message>, ExpansionState) -> Element<'view, Message> + 'view>,
    pub gap_px: f32,
    pub expansion_duration: Duration,
}

#[derive(Debug)]
pub struct ExpandingSlider {
    pub expansion_state: ExpansionState,
    scroll_accumulated: f32,
    pub synced_value: SyncedValue,
    pub range: RangeInclusive<f32>,
    scroll_step: f32,
    pub slider_step: f32,
    scroll_writes: ScrollWrites,
    secondary_press: SecondaryPress,
}

impl ExpandingSlider {
    #[must_use]
    pub fn new(range: RangeInclusive<f32>) -> Self {
        Self {
            expansion_state: ExpansionState::Collapsed,
            scroll_accumulated: 0.0,
            synced_value: SyncedValue::default(),
            range,
            scroll_step: 1.0,
            slider_step: 1.0,
            scroll_writes: ScrollWrites::Absolute,
            secondary_press: SecondaryPress::Ignored,
        }
    }

    #[must_use]
    pub fn scroll_step(self, scroll_step: f32) -> Self {
        Self {
            scroll_step,
            ..self
        }
    }

    #[must_use]
    pub fn slider_step(self, slider_step: f32) -> Self {
        Self {
            slider_step,
            ..self
        }
    }

    #[must_use]
    pub fn scroll_writes(self, scroll_writes: ScrollWrites) -> Self {
        Self {
            scroll_writes,
            ..self
        }
    }

    #[must_use]
    pub fn secondary_press(self, secondary_press: SecondaryPress) -> Self {
        Self {
            secondary_press,
            ..self
        }
    }

    #[must_use]
    pub fn fraction_of_range(&self) -> f32 {
        let span = self.range.end() - self.range.start();
        ((self.synced_value.shown_value() - self.range.start()) / span).clamp(0.0, 1.0)
    }

    pub fn close(&mut self) {
        self.expansion_state = ExpansionState::Collapsed;
    }

    pub fn update(&mut self, message: SliderMessage) -> SliderEffect {
        match message {
            SliderMessage::Gesture(gesture) => self.gestured(gesture),
            SliderMessage::AbsoluteWritten { value, outcome } => {
                let outcome = match outcome {
                    WriteOutcome::Accepted => Ok(()),
                    WriteOutcome::Rejected => Err(()),
                };
                written(self.synced_value.absolute_write_finished(value, &outcome))
            }
            SliderMessage::RelativeWritten(value) => {
                written(self.synced_value.relative_write_resolved(value))
            }
            SliderMessage::RelativeFailed => {
                self.synced_value.relative_write_failed();
                SliderEffect::Unchanged
            }
            SliderMessage::SourceReported(value) => {
                self.synced_value.source_reported(value, self.slider_step);
                SliderEffect::Unchanged
            }
        }
    }

    fn gestured(&mut self, gesture: SliderGesture) -> SliderEffect {
        match gesture {
            SliderGesture::Toggle => match self.expansion_state {
                ExpansionState::Collapsed => {
                    self.expansion_state = ExpansionState::Expanded;
                    SliderEffect::Opened
                }
                ExpansionState::Expanded => {
                    self.expansion_state = ExpansionState::Collapsed;
                    SliderEffect::Closed
                }
            },
            SliderGesture::Scroll(delta) => self.scrolled(delta),
            SliderGesture::Drag(value) => written(self.synced_value.drag_moved_to(value)),
            SliderGesture::Release => written(self.synced_value.drag_ended()),
            SliderGesture::SecondaryPress => match self.secondary_press {
                SecondaryPress::Reported => SliderEffect::SecondaryPressed,
                SecondaryPress::Ignored => SliderEffect::Unchanged,
            },
        }
    }

    fn scrolled(&mut self, delta: ScrollDelta) -> SliderEffect {
        let Some(steps) = wheel_step_direction(&mut self.scroll_accumulated, delta) else {
            return SliderEffect::Unchanged;
        };
        let shown = self.synced_value.shown_value();
        #[expect(
            clippy::cast_precision_loss,
            reason = "a scroll gesture moves a handful of steps"
        )]
        let target =
            (shown + steps as f32 * self.scroll_step).clamp(*self.range.start(), *self.range.end());
        written(match self.scroll_writes {
            ScrollWrites::Absolute => self.synced_value.scrolled_to(target),
            ScrollWrites::Relative => self.synced_value.nudged_by(Nudge {
                step_delta: target - shown,
                resulting_value: target,
            }),
        })
    }
}

fn written(value_write: Option<ValueWrite>) -> SliderEffect {
    match value_write {
        Some(value_write) => SliderEffect::Write(value_write),
        None => SliderEffect::Unchanged,
    }
}

#[cfg(test)]
mod tests {
    use iced::mouse::ScrollDelta;

    use super::{
        ExpandingSlider, ScrollWrites, SliderEffect, SliderGesture, SliderMessage, WriteOutcome,
    };
    use crate::value_controls::synced_value::ValueWrite;

    const LINE_UP: ScrollDelta = ScrollDelta::Lines { x: 0.0, y: 1.0 };

    #[test]
    fn toggling_opens_then_closes() {
        let mut slider = ExpandingSlider::new(0.0..=100.0);
        assert_eq!(
            slider.update(SliderMessage::Gesture(SliderGesture::Toggle)),
            SliderEffect::Opened
        );
        assert_eq!(
            slider.update(SliderMessage::Gesture(SliderGesture::Toggle)),
            SliderEffect::Closed
        );
    }

    #[test]
    fn scrolling_writes_one_step_within_the_range() {
        let mut slider = ExpandingSlider::new(0.0..=10.0).scroll_step(4.0);
        assert_eq!(
            slider.update(SliderMessage::Gesture(SliderGesture::Scroll(LINE_UP))),
            SliderEffect::Write(ValueWrite::Absolute(4.0))
        );
        slider.update(SliderMessage::AbsoluteWritten {
            value: 4.0,
            outcome: WriteOutcome::Accepted,
        });
        slider.update(SliderMessage::Gesture(SliderGesture::Scroll(LINE_UP)));
        slider.update(SliderMessage::AbsoluteWritten {
            value: 8.0,
            outcome: WriteOutcome::Accepted,
        });
        assert_eq!(
            slider.update(SliderMessage::Gesture(SliderGesture::Scroll(LINE_UP))),
            SliderEffect::Write(ValueWrite::Absolute(10.0))
        );
    }

    #[test]
    fn relative_scrolling_sends_the_step() {
        let mut slider = ExpandingSlider::new(0.0..=100.0)
            .scroll_step(5.0)
            .scroll_writes(ScrollWrites::Relative);
        assert_eq!(
            slider.update(SliderMessage::Gesture(SliderGesture::Scroll(LINE_UP))),
            SliderEffect::Write(ValueWrite::Relative(5.0))
        );
    }
}
