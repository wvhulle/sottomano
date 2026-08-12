use std::ops::RangeInclusive;

use iced::{
    Alignment, Animation, Element, Padding, Task,
    alignment::Horizontal,
    animation::Easing,
    mouse::ScrollDelta,
    time::{Duration, Instant},
    widget::{Row, button, container, mouse_area},
};

use crate::{
    config::Config,
    data::source::{BarWidget, Glyph, Level, TooltipBody, Unit, Value, font_awesome::FontAwesome},
    message_subscriptions::blocking,
    wayland_layers::tooltip::{HoverTarget, TooltipMessage, hoverable},
    widget_groups::scroll::scroll_step,
    widgets::{
        flat_button,
        icon_gauge::source_reading,
        inline_slider::inline_slider,
        reveal::reveal_x,
        text::bar_text,
        tracked::{Nudge, Tracked, Write},
    },
};

const EXPAND: Duration = Duration::from_millis(90);

#[allow(clippy::struct_excessive_bools)]
pub struct SliderControl {
    open: Animation<bool>,
    accum: f32,
    value: Tracked,
    disabled: bool,
    caption: Option<String>,
    range: RangeInclusive<f32>,
    scroll_step: f32,
    slider_step: f32,
    readout_width: f32,
    readout: fn(f32) -> String,
    gauge: bool,
    label: Option<fn(f32) -> String>,
    icon: FontAwesome,
    glyph: Option<fn(f32, bool) -> FontAwesome>,
    thresholds: SeverityThresholds,
    target: HoverTarget,
    right_click: bool,
    relative_scroll: bool,
}

#[derive(Default)]
struct SeverityThresholds {
    warn: Option<f32>,
    urgent: Option<f32>,
    higher_worse: bool,
}

impl SeverityThresholds {
    fn level(&self, value: f32) -> Level {
        let hit = |t: Option<f32>| {
            t.is_some_and(|t| {
                if self.higher_worse {
                    value >= t
                } else {
                    value <= t
                }
            })
        };
        if hit(self.urgent) {
            Level::Critical
        } else if hit(self.warn) {
            Level::Warning
        } else {
            Level::Ok
        }
    }
}

#[derive(Debug, Clone)]
pub enum ControlMessage {
    Press,
    Scroll(ScrollDelta),
    Set(f32),
    Release,
    Confirmed { value: f32, ok: bool },
    Adjusted(Option<f32>),
    Reading(f32),
    Disabled(bool),
    Caption(Option<String>),
    Right,
    Tooltip(TooltipMessage),
}

impl ControlMessage {
    pub fn commit(
        label: &'static str,
        value: f32,
        write: impl FnOnce() -> bool + Send + 'static,
    ) -> Task<ControlMessage> {
        blocking(label, write).map(move |ok| ControlMessage::Confirmed { value, ok })
    }
}

pub enum Action {
    None,
    Opened,
    Commit(f32),
    Adjust(f32),
    Right,
}

impl SliderControl {
    pub fn new(range: RangeInclusive<f32>, target: HoverTarget, icon: FontAwesome) -> Self {
        Self {
            open: Animation::new(false)
                .easing(Easing::EaseOutCubic)
                .duration(EXPAND),
            accum: 0.0,
            value: Tracked::default(),
            disabled: false,
            caption: None,
            range,
            scroll_step: 1.0,
            slider_step: 1.0,
            readout_width: 40.0,
            readout: |value| format!("{value:.0}"),
            gauge: true,
            label: None,
            icon,
            glyph: None,
            thresholds: SeverityThresholds::default(),
            target,
            right_click: false,
            relative_scroll: false,
        }
    }

    #[must_use]
    pub fn readout(mut self, format: fn(f32) -> String) -> Self {
        self.readout = format;
        self
    }

    #[must_use]
    pub fn scroll_step(mut self, step: f32) -> Self {
        self.scroll_step = step;
        self
    }

    #[must_use]
    pub fn slider_step(mut self, step: f32) -> Self {
        self.slider_step = step;
        self
    }

    #[must_use]
    pub fn readout_width(mut self, width: f32) -> Self {
        self.readout_width = width;
        self
    }

    #[must_use]
    pub fn label(mut self, format: fn(f32) -> String) -> Self {
        self.label = Some(format);
        self.gauge = false;
        self
    }

    #[must_use]
    pub fn icon_only(mut self) -> Self {
        self.gauge = false;
        self
    }

    #[must_use]
    pub fn glyph(mut self, glyph: fn(f32, bool) -> FontAwesome) -> Self {
        self.glyph = Some(glyph);
        self
    }

    #[must_use]
    pub fn thresholds(mut self, warn: f32, urgent: f32, higher_worse: bool) -> Self {
        self.thresholds = SeverityThresholds {
            warn: Some(warn),
            urgent: Some(urgent),
            higher_worse,
        };
        self
    }

    #[must_use]
    pub fn right_click(mut self, on: bool) -> Self {
        self.right_click = on;
        self
    }

    #[must_use]
    pub fn relative_scroll(mut self) -> Self {
        self.relative_scroll = true;
        self
    }

    pub fn close(&mut self) {
        self.open.go_mut(false, Instant::now());
    }

    pub fn is_animating(&self) -> bool {
        self.open.is_animating(Instant::now())
    }

    pub fn current(&self) -> f32 {
        self.value.value()
    }

    fn severity(&self) -> Level {
        self.thresholds.level(self.current())
    }

    fn effective_icon(&self) -> FontAwesome {
        self.glyph
            .map_or(self.icon, |glyph| glyph(self.current(), self.disabled))
    }

    pub fn update(&mut self, msg: ControlMessage) -> Action {
        match msg {
            ControlMessage::Press => {
                self.open.go_mut(!self.open.value(), Instant::now());
                if self.open.value() {
                    Action::Opened
                } else {
                    Action::None
                }
            }
            ControlMessage::Scroll(delta) => {
                let Some(step) = scroll_step(&mut self.accum, delta) else {
                    return Action::None;
                };
                #[allow(clippy::cast_precision_loss)]
                let target = (self.current() + step as f32 * self.scroll_step)
                    .clamp(*self.range.start(), *self.range.end());
                if self.relative_scroll {
                    let nudge = Nudge {
                        delta: target - self.current(),
                        optimistic: target,
                    };
                    into_action(self.value.adjust(nudge))
                } else {
                    into_action(self.value.scroll(target))
                }
            }
            ControlMessage::Set(pos) => into_action(self.value.drag(pos)),
            ControlMessage::Release => into_action(self.value.end_drag()),
            ControlMessage::Confirmed { value, ok } => into_action(self.value.confirmed(value, ok)),
            ControlMessage::Adjusted(resolved) => into_action(self.value.adjusted(resolved)),
            ControlMessage::Reading(reading) => {
                self.value.sync(reading);
                Action::None
            }
            ControlMessage::Disabled(disabled) => {
                self.disabled = disabled;
                Action::None
            }
            ControlMessage::Caption(caption) => {
                self.caption = caption;
                Action::None
            }
            ControlMessage::Right => Action::Right,
            ControlMessage::Tooltip(_) => Action::None,
        }
    }

    pub fn view<'a>(&self, config: &Config, reveal: f32) -> Element<'a, ControlMessage> {
        let expand = self.open.interpolate(0.0, 1.0, Instant::now());
        if expand > 0.0 {
            let glyph = button(
                self.effective_icon()
                    .glyph()
                    .size(config.font.icon_size_px)
                    .color(config.palette.bar_foreground),
            )
            .padding(0)
            .style(flat_button(config.palette.bar_foreground))
            .on_press(ControlMessage::Press);
            let slider = inline_slider(
                config,
                self.range.clone(),
                self.current(),
                self.value.ghost(),
                self.slider_step,
                self.severity(),
                ControlMessage::Set,
                ControlMessage::Release,
            );
            let readout = button(
                bar_text(&config.font, (self.readout)(self.current()))
                    .width(self.readout_width)
                    .align_x(Horizontal::Center)
                    .color(config.palette.bar_foreground),
            )
            .padding(0)
            .style(flat_button(config.palette.bar_foreground))
            .on_press(ControlMessage::Press);
            let gap = config.spacing.widget_spacing_px;
            let revealed = Row::with_children([slider, readout.into()])
                .spacing(gap)
                .align_y(Alignment::Center);
            return Row::with_children([
                glyph.into(),
                reveal_x(container(revealed).padding(Padding::ZERO.left(gap)), expand),
            ])
            .align_y(Alignment::Center)
            .into();
        }

        let reading = source_reading(config, Some(self as &dyn BarWidget), reveal)
            .unwrap_or_else(|| Row::new().into());
        let hover = hoverable(reading, self.target.clone(), ControlMessage::Tooltip);
        let mut area = mouse_area(hover)
            .on_press(ControlMessage::Press)
            .on_scroll(ControlMessage::Scroll);
        if self.right_click {
            area = area.on_right_press(ControlMessage::Right);
        }
        area.into()
    }
}

impl BarWidget for SliderControl {
    fn icon(&self) -> Glyph {
        self.effective_icon().into()
    }

    fn label(&self) -> Option<String> {
        self.caption
            .clone()
            .or_else(|| self.label.map(|format| format(self.current())))
    }

    fn level(&self) -> Level {
        self.severity()
    }

    fn value(&self) -> Option<Value> {
        self.gauge.then(|| Value::Gauge {
            #[allow(clippy::cast_possible_truncation)]
            percent: self.current().round() as i64,
            unit: Unit::Percent,
        })
    }

    fn disabled(&self) -> bool {
        self.disabled
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(Vec::new())
    }
}

#[allow(clippy::needless_pass_by_value)]
fn into_action(write: Option<Write>) -> Action {
    match write {
        Some(Write::Absolute(value)) => Action::Commit(value),
        Some(Write::Relative(delta)) => Action::Adjust(delta),
        None => Action::None,
    }
}

#[allow(clippy::cast_precision_loss)]
pub(crate) fn percent_f(percent: i64) -> f32 {
    percent as f32
}

#[allow(clippy::cast_possible_truncation)]
pub(crate) fn as_percent(value: f32) -> i64 {
    value.round() as i64
}
