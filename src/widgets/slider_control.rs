use std::ops::RangeInclusive;

use iced::{
    Alignment, Element, Padding, Task,
    alignment::Horizontal,
    mouse::ScrollDelta,
    time::Duration,
    widget::{Row, button, container, mouse_area},
};
use uom::si::{f64::Ratio, ratio};

use crate::{
    config::Config,
    message_subscriptions::blocking,
    wayland_layers::tooltip::{HoverTarget, TooltipMessage, hoverable},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        device_setting::{DeviceSetting, Nudge, Write},
        expand_animation::expand_animation,
        flat_button,
        inline_slider::inline_slider,
        phosphor::Phosphor,
        reveal::reveal_x,
        source_reading::source_reading,
        text::bar_text,
        tooltip_body::TooltipBody,
        widget_state::{Level, WidgetState},
    },
};

const EXPAND: Duration = Duration::from_millis(90);
const PIXELS_PER_SCROLL_STEP: f32 = 40.0;

fn scroll_direction(accum: &mut f32, delta: ScrollDelta) -> Option<i64> {
    match delta {
        ScrollDelta::Lines { y, .. } => {
            if y > 0.0 {
                Some(1)
            } else if y < 0.0 {
                Some(-1)
            } else {
                None
            }
        }
        ScrollDelta::Pixels { y, .. } => {
            *accum += y;
            if accum.abs() >= PIXELS_PER_SCROLL_STEP {
                let step = if *accum > 0.0 { 1 } else { -1 };
                *accum = 0.0;
                Some(step)
            } else {
                None
            }
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
pub struct SliderControl {
    open: bool,
    accum: f32,
    device_setting: DeviceSetting,
    disabled: bool,
    live: f32,
    range: RangeInclusive<f32>,
    scroll_step: f32,
    slider_step: f32,
    readout_width: f32,
    readout: fn(f32) -> String,
    gauge: bool,
    label: Option<fn(f32) -> String>,
    icon: Glyph,
    glyph: Option<fn(f32, bool) -> Phosphor>,
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
    Level(f32),
    Disabled(bool),
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
    pub fn new(range: RangeInclusive<f32>, target: HoverTarget, icon: impl Into<Glyph>) -> Self {
        Self {
            open: false,
            accum: 0.0,
            device_setting: DeviceSetting::default(),
            disabled: false,
            live: 1.0,
            range,
            scroll_step: 1.0,
            slider_step: 1.0,
            readout_width: 40.0,
            readout: |value| format!("{value:.0}"),
            gauge: true,
            label: None,
            icon: icon.into(),
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
    pub fn glyph(mut self, glyph: fn(f32, bool) -> Phosphor) -> Self {
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
        self.open = false;
    }

    pub fn current(&self) -> f32 {
        self.device_setting.wanted_value()
    }

    pub fn readout_text(&self) -> String {
        (self.readout)(self.current())
    }

    fn severity(&self) -> Level {
        self.thresholds.level(self.current())
    }

    fn effective_icon(&self) -> Glyph {
        self.glyph.map_or(self.icon, |glyph| {
            glyph(self.current(), self.disabled).into()
        })
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn update(&mut self, msg: ControlMessage) -> Action {
        match msg {
            ControlMessage::Press => {
                self.open = !self.open;
                if self.open {
                    Action::Opened
                } else {
                    Action::None
                }
            }
            ControlMessage::Scroll(delta) => {
                let Some(step) = scroll_direction(&mut self.accum, delta) else {
                    return Action::None;
                };
                #[allow(clippy::cast_precision_loss)]
                let target = (self.current() + step as f32 * self.scroll_step)
                    .clamp(*self.range.start(), *self.range.end());
                if self.relative_scroll {
                    let nudge = Nudge {
                        step_delta: target - self.current(),
                        resulting_value: target,
                    };
                    into_action(self.device_setting.nudged_by(nudge))
                } else {
                    into_action(self.device_setting.scrolled_to(target))
                }
            }
            ControlMessage::Set(pos) => into_action(self.device_setting.drag_moved_to(pos)),
            ControlMessage::Release => into_action(self.device_setting.drag_ended()),
            ControlMessage::Confirmed { value, ok } => {
                into_action(self.device_setting.absolute_write_finished(value, ok))
            }
            ControlMessage::Adjusted(resolved) => {
                into_action(self.device_setting.relative_write_finished(resolved))
            }
            ControlMessage::Reading(reading) => {
                self.device_setting
                    .device_reported(reading, self.slider_step);
                Action::None
            }
            ControlMessage::Level(level) => {
                self.live = level;
                Action::None
            }
            ControlMessage::Disabled(disabled) => {
                self.disabled = disabled;
                Action::None
            }
            ControlMessage::Right => Action::Right,
            ControlMessage::Tooltip(_) => Action::None,
        }
    }

    pub fn view<'a>(&'a self, config: &'a Config, reveal: f32) -> Element<'a, ControlMessage> {
        expand_animation(self.open, EXPAND, move |expand| {
            if expand > 0.0 {
                self.expanded_view(config, expand)
            } else {
                self.collapsed_view(config, reveal)
            }
        })
    }

    fn expanded_view<'a>(&self, config: &Config, expand: f32) -> Element<'a, ControlMessage> {
        let state = self.state();
        let glyph = button(
            self.effective_icon()
                .with_weight(state.weight())
                .view(config.font.icon_size_px, state.color(&config.palette)),
        )
        .padding(0)
        .style(flat_button(config.palette.bar_foreground))
        .on_press(ControlMessage::Press);
        let slider = inline_slider(
            config,
            self.range.clone(),
            self.current(),
            self.device_setting.lagging_device_value(self.slider_step),
            self.slider_step,
            self.severity(),
            ControlMessage::Set,
            ControlMessage::Release,
        );
        let readout = button(
            bar_text(&config.font, self.readout_text())
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
        let opened = Row::with_children([
            glyph.into(),
            reveal_x(container(revealed).padding(Padding::ZERO.left(gap)), expand),
        ])
        .align_y(Alignment::Center);
        let mut area = mouse_area(opened).on_scroll(ControlMessage::Scroll);
        if self.right_click {
            area = area.on_right_press(ControlMessage::Right);
        }
        area.into()
    }

    fn collapsed_view<'a>(&self, config: &Config, reveal: f32) -> Element<'a, ControlMessage> {
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
        self.effective_icon()
    }

    fn label(&self) -> Option<String> {
        self.label.map(|format| format(self.current()))
    }

    fn state(&self) -> WidgetState {
        if self.disabled {
            WidgetState::Off
        } else {
            self.severity().into()
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        self.gauge
            .then(|| Ratio::new::<ratio::percent>(f64::from(self.current())))
    }

    fn live_gauge(&self) -> Option<Ratio> {
        self.gauge()
            .map(|fill| fill * Ratio::new::<ratio::ratio>(f64::from(self.live)))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(Vec::new())
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
