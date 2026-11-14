use std::{ops::RangeInclusive, rc::Rc, time::Duration};

use iced::{
    Alignment, Element, Padding,
    mouse::ScrollDelta,
    widget::{Row, button, container, mouse_area},
};

use crate::{
    orientation::Axis,
    press_actions::flat_button,
    selection_scroll::wheel_step_direction,
    theming::text_color,
    transitions::{ExpansionState, expand_transition, reveal},
    value_controls::{
        SliderAccents, inline_slider,
        synced_value::{Nudge, SourceTracking, SyncedValue, ValueWrite},
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

pub struct SliderLook<'a> {
    pub icon_size_px: f32,
    pub accents: SliderAccents<'a>,
}

type ElementPart<'a, Message> = Box<dyn Fn() -> Element<'a, Message> + 'a>;

pub struct ExpandingSliderParts<'a, Message> {
    pub collapsed: ElementPart<'a, Message>,
    pub face: ElementPart<'a, Message>,
    pub readout: ElementPart<'a, Message>,
    pub look: Box<dyn Fn() -> SliderLook<'a> + 'a>,
    pub decorate: Box<dyn Fn(Element<'a, Message>, ExpansionState) -> Element<'a, Message> + 'a>,
    pub gap_px: f32,
    pub expansion_duration: Duration,
}

#[derive(Debug)]
pub struct ExpandingSlider {
    expansion_state: ExpansionState,
    scroll_accumulated: f32,
    synced_value: SyncedValue,
    range: RangeInclusive<f32>,
    scroll_step: f32,
    slider_step: f32,
    scroll_writes: ScrollWrites,
    secondary_press: SecondaryPress,
}

#[derive(Debug, Clone, Copy)]
struct SliderPosition {
    shown: f32,
    lagging: Option<f32>,
    step: f32,
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
    pub fn shown_value(&self) -> f32 {
        self.synced_value.shown_value()
    }

    #[must_use]
    pub fn reported_value(&self) -> f32 {
        self.synced_value.reported_value()
    }

    #[must_use]
    pub fn source_tracking(&self) -> SourceTracking {
        self.synced_value.source_tracking()
    }

    #[must_use]
    pub fn range(&self) -> RangeInclusive<f32> {
        self.range.clone()
    }

    #[must_use]
    pub fn slider_step_size(&self) -> f32 {
        self.slider_step
    }

    #[must_use]
    pub fn fraction_of_range(&self) -> f32 {
        let span = self.range.end() - self.range.start();
        ((self.shown_value() - self.range.start()) / span).clamp(0.0, 1.0)
    }

    #[must_use]
    pub fn expansion_state(&self) -> ExpansionState {
        self.expansion_state
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
        let shown = self.shown_value();
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

    fn position(&self) -> SliderPosition {
        SliderPosition {
            shown: self.shown_value(),
            lagging: self.synced_value.lagging_reported_value(self.slider_step),
            step: self.slider_step,
        }
    }

    pub fn slider<'a, Message: Clone + 'a>(
        &self,
        look: SliderLook<'a>,
        on_message: impl Fn(SliderGesture) -> Message + 'a,
    ) -> Element<'a, Message> {
        slider_element(
            self.range.clone(),
            self.position(),
            look,
            Rc::new(on_message),
        )
    }

    pub fn open_row<'a, Message: Clone + 'a>(
        &self,
        look: SliderLook<'a>,
        readout: Element<'a, Message>,
        gap_px: f32,
        on_message: impl Fn(SliderGesture) -> Message + 'a,
    ) -> Element<'a, Message> {
        let on_message: Rc<dyn Fn(SliderGesture) -> Message + 'a> = Rc::new(on_message);
        let scrolled = Rc::clone(&on_message);
        let row = Row::with_children([
            slider_element(
                self.range.clone(),
                self.position(),
                look,
                Rc::clone(&on_message),
            ),
            readout,
        ])
        .spacing(gap_px)
        .align_y(Alignment::Center);
        mouse_area(row)
            .on_scroll(move |delta| scrolled(SliderGesture::Scroll(delta)))
            .into()
    }

    pub fn view<'a, Message: Clone + 'a>(
        &self,
        parts: ExpandingSliderParts<'a, Message>,
        on_message: impl Fn(SliderGesture) -> Message + 'a,
    ) -> Element<'a, Message> {
        let on_message: Rc<dyn Fn(SliderGesture) -> Message + 'a> = Rc::new(on_message);
        let range = self.range.clone();
        let position = self.position();
        let secondary_press = self.secondary_press;
        let ExpandingSliderParts {
            collapsed,
            face,
            readout,
            look,
            decorate,
            gap_px,
            expansion_duration,
        } = parts;
        expand_transition(self.expansion_state, expansion_duration, move |expand| {
            let toggle = on_message(SliderGesture::Toggle);
            let (content, stage) = if expand > 0.0 {
                let closing_button = |content: Element<'a, Message>| -> Element<'a, Message> {
                    button(content)
                        .padding(0)
                        .style(flat_button(text_color))
                        .on_press(toggle.clone())
                        .into()
                };
                let revealed = Row::with_children([
                    slider_element(range.clone(), position, look(), Rc::clone(&on_message)),
                    closing_button(readout()),
                ])
                .spacing(gap_px)
                .align_y(Alignment::Center);
                let opened = Row::with_children([
                    closing_button(face()),
                    reveal(
                        container(revealed).padding(Padding::ZERO.left(gap_px)),
                        expand,
                        Axis::Horizontal,
                    )
                    .into(),
                ])
                .align_y(Alignment::Center);
                (opened.into(), ExpansionState::Expanded)
            } else {
                (collapsed(), ExpansionState::Collapsed)
            };
            let scrolled = Rc::clone(&on_message);
            let area = mouse_area(decorate(content, stage))
                .on_scroll(move |delta| scrolled(SliderGesture::Scroll(delta)));
            let area = match stage {
                ExpansionState::Collapsed => area.on_press(toggle),
                ExpansionState::Expanded => area,
            };
            match secondary_press {
                SecondaryPress::Reported => area
                    .on_right_press(on_message(SliderGesture::SecondaryPress))
                    .into(),
                SecondaryPress::Ignored => area.into(),
            }
        })
    }
}

fn slider_element<'a, Message: Clone + 'a>(
    range: RangeInclusive<f32>,
    position: SliderPosition,
    look: SliderLook<'a>,
    on_message: Rc<dyn Fn(SliderGesture) -> Message + 'a>,
) -> Element<'a, Message> {
    let released = on_message(SliderGesture::Release);
    let dragged = Rc::clone(&on_message);
    inline_slider(range, position.shown, move |value| {
        dragged(SliderGesture::Drag(value))
    })
    .icon_size(look.icon_size_px)
    .lagging_behind(position.lagging)
    .step(position.step)
    .accents(look.accents)
    .on_release(released)
    .on_scroll(move |delta| on_message(SliderGesture::Scroll(delta)))
    .into()
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
