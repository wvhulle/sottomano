use std::{ops::RangeInclusive, rc::Rc};

use iced::{
    Alignment, Element, Padding,
    widget::{Row, button, container, mouse_area},
};

use super::{ExpandingSlider, ExpandingSliderParts, SecondaryPress, SliderGesture, SliderLook};
use crate::{
    orientation::Axis,
    press_actions::flat_button,
    theming::text_color,
    transitions::{ExpansionState, expand_transition, reveal},
    value_controls::{inline_slider, synced_value::SourceLag},
};

#[derive(Debug, Clone, Copy)]
struct SliderPosition {
    shown: f32,
    source_lag: SourceLag,
    step: f32,
}

impl ExpandingSlider {
    fn position(&self) -> SliderPosition {
        SliderPosition {
            shown: self.synced_value.shown_value(),
            source_lag: self.synced_value.source_lag(self.slider_step),
            step: self.slider_step,
        }
    }

    pub fn slider<'view, Message: Clone + 'view>(
        &self,
        look: SliderLook<'view>,
        on_message: impl Fn(SliderGesture) -> Message + 'view,
    ) -> Element<'view, Message> {
        slider_element(
            self.range.clone(),
            self.position(),
            look,
            Rc::new(on_message),
        )
    }

    pub fn open_row<'view, Message: Clone + 'view>(
        &self,
        look: SliderLook<'view>,
        readout: Element<'view, Message>,
        gap_px: f32,
        on_message: impl Fn(SliderGesture) -> Message + 'view,
    ) -> Element<'view, Message> {
        let on_message: Rc<dyn Fn(SliderGesture) -> Message + 'view> = Rc::new(on_message);
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

    pub fn view<'view, Message: Clone + 'view>(
        &self,
        parts: ExpandingSliderParts<'view, Message>,
        on_message: impl Fn(SliderGesture) -> Message + 'view,
    ) -> Element<'view, Message> {
        let on_message: Rc<dyn Fn(SliderGesture) -> Message + 'view> = Rc::new(on_message);
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
                let closing_button = |content| -> Element<'view, Message> {
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

fn slider_element<'view, Message: Clone + 'view>(
    range: RangeInclusive<f32>,
    position: SliderPosition,
    look: SliderLook<'view>,
    on_message: Rc<dyn Fn(SliderGesture) -> Message + 'view>,
) -> Element<'view, Message> {
    let released = on_message(SliderGesture::Release);
    let dragged = Rc::clone(&on_message);
    inline_slider(range, position.shown, move |value| {
        dragged(SliderGesture::Drag(value))
    })
    .icon_size(look.icon_size_px)
    .lagging_behind(position.source_lag)
    .step(position.step)
    .accents(look.accents)
    .on_release(released)
    .on_scroll(move |delta| on_message(SliderGesture::Scroll(delta)))
    .into()
}
