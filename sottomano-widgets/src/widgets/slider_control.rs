use std::ops::RangeInclusive;

use chrono::Locale;
use iced::{
    Alignment, Element, Length, Padding, Task,
    alignment::Horizontal,
    mouse::ScrollDelta,
    widget::{Row, button, container, mouse_area, text, text::Wrapping},
};
use sottomano_config::{Config, severity::Severity};
use sottomano_sources::blocking_background_work::blocking;
use uom::si::{f64::Ratio, ratio};

use crate::{
    clock::Clock,
    hover_tooltips::{HoverTarget, PressHints, TooltipMessage, hoverable},
    widgets::{
        announced_level::{AnnouncedLevel, LevelAnnouncement},
        bar_widget::{BarWidget, Glyph},
        configured_theme::text_color,
        device_setting::{DeviceSetting, Nudge, Write},
        expand_animation::expand_animation,
        flat_button,
        inline_slider::inline_slider,
        phosphor::Phosphor,
        reveal::reveal_x,
        source_reading::{labeled_reading, source_reading},
        text::bar_text,
        tooltip_body::{
            TooltipBody,
            tooltip_source_delivery::{DeliveryTiming, Tooltip},
        },
        value_cells::{LabeledValue, SliderReadout, ValueCell},
        widest_readout_text::{ReadoutLabels, widest_readout_text},
        widget_state::WidgetState,
    },
};

const PIXELS_PER_SCROLL_STEP: f32 = 40.0;

pub fn scroll_direction(accum: &mut f32, delta: ScrollDelta) -> Option<i64> {
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
    name: &'static str,
    open: bool,
    accum: f32,
    device_setting: DeviceSetting,
    disabled: bool,
    live: f32,
    range: RangeInclusive<f32>,
    scroll_step: f32,
    slider_step: f32,
    readout: fn(f32, Locale) -> String,
    gauge: bool,
    label: Option<fn(f32, Locale) -> Option<String>>,
    icon: Glyph,
    glyph: Option<fn(f32, bool) -> Phosphor>,
    thresholds: Option<SeverityThresholds>,
    target: HoverTarget,
    right_click: Option<&'static str>,
    relative_scroll: bool,
    level_announcement: Option<LevelAnnouncement>,
}

struct SeverityThresholds {
    warn: f32,
    urgent: f32,
    higher_worse: bool,
}

impl SeverityThresholds {
    fn level(&self, value: f32) -> Severity {
        let hit = |t: f32| {
            if self.higher_worse {
                value >= t
            } else {
                value <= t
            }
        };
        if hit(self.urgent) {
            Severity::Critical
        } else if hit(self.warn) {
            Severity::Warning
        } else {
            Severity::Normal
        }
    }
}

#[derive(Debug, Clone)]
pub enum ControlMessage {
    Press,
    Scroll(ScrollDelta),
    Set(f32),
    Release,
    DeviceAccepted { value: f32, ok: bool },
    Persisted { value: f32, ok: bool },
    Adjusted(f32),
    AdjustFailed,
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
        Task::perform(blocking(label, write), move |ok| {
            ControlMessage::DeviceAccepted { value, ok }
        })
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
    pub fn new(
        name: &'static str,
        range: RangeInclusive<f32>,
        target: impl Into<HoverTarget>,
        icon: impl Into<Glyph>,
    ) -> Self {
        Self {
            name,
            open: false,
            accum: 0.0,
            device_setting: DeviceSetting::default(),
            disabled: false,
            live: 1.0,
            range,
            scroll_step: 1.0,
            slider_step: 1.0,
            readout: |value, _| format!("{value:.0}"),
            gauge: true,
            label: None,
            icon: icon.into(),
            glyph: None,
            thresholds: None,
            target: target.into(),
            right_click: None,
            relative_scroll: false,
            level_announcement: None,
        }
    }

    #[must_use]
    pub fn readout(mut self, format: fn(f32, Locale) -> String) -> Self {
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
    pub fn label(mut self, format: fn(f32, Locale) -> Option<String>) -> Self {
        self.label = Some(format);
        self.gauge = false;
        self
    }

    #[must_use]
    pub fn label_with_gauge(mut self, format: fn(f32, Locale) -> Option<String>) -> Self {
        self.label = Some(format);
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
        self.thresholds = Some(SeverityThresholds {
            warn,
            urgent,
            higher_worse,
        });
        self
    }

    #[must_use]
    pub fn right_click(mut self, does: &'static str) -> Self {
        self.right_click = Some(does);
        self
    }

    #[must_use]
    pub fn relative_scroll(mut self) -> Self {
        self.relative_scroll = true;
        self
    }

    #[must_use]
    pub fn announces_level(mut self, summary_text: &'static str) -> Self {
        self.level_announcement = Some(LevelAnnouncement::new(summary_text));
        self
    }

    fn fraction_of_range(&self) -> f32 {
        let span = self.range.end() - self.range.start();
        ((self.current() - self.range.start()) / span).clamp(0.0, 1.0)
    }

    fn range_fill(&self) -> Ratio {
        Ratio::new::<ratio::ratio>(f64::from(self.fraction_of_range()))
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    #[must_use]
    pub fn target(&self) -> &HoverTarget {
        &self.target
    }

    pub fn set_icon(&mut self, icon: impl Into<Glyph>) {
        self.icon = icon.into();
    }

    #[must_use]
    pub fn current(&self) -> f32 {
        self.device_setting.wanted_value()
    }

    #[must_use]
    pub fn settled_value(&self) -> f32 {
        self.device_setting.reported_value()
    }

    #[must_use]
    pub fn readout_text(&self, locale: Locale) -> String {
        (self.readout)(self.current(), locale)
    }

    #[must_use]
    pub fn readout_cell(&self) -> ValueCell {
        ValueCell::Readout(SliderReadout::new(self.current(), self.readout))
    }

    pub fn readout_tooltip(&self, label: impl Into<ValueCell>) -> Tooltip {
        Tooltip::sourced(
            TooltipBody::dictionary([LabeledValue::new(label, self.readout_cell())]),
            DeliveryTiming::LivePush,
        )
    }

    fn severity(&self) -> Severity {
        self.thresholds
            .as_ref()
            .map_or(Severity::Normal, |thresholds| {
                thresholds.level(self.current())
            })
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
            ControlMessage::DeviceAccepted { value, ok } => {
                into_action(self.device_setting.absolute_write_finished(value, ok))
            }
            ControlMessage::Persisted { value, ok } => {
                let follow_up = self.device_setting.absolute_write_finished(value, ok);
                if ok {
                    self.device_setting.device_reported(value, self.slider_step);
                }
                into_action(follow_up)
            }
            ControlMessage::Adjusted(value) => {
                into_action(self.device_setting.relative_write_resolved(value))
            }
            ControlMessage::AdjustFailed => {
                self.device_setting.relative_write_failed();
                Action::None
            }
            ControlMessage::Reading(reading) => {
                let followed_device = self.device_setting.follows_device();
                let shown_before = self.current();
                self.device_setting
                    .device_reported(reading, self.slider_step);
                let moved = (self.current() - shown_before).abs() >= self.slider_step / 2.0;
                if let Some(announcement) = &mut self.level_announcement {
                    announcement.device_reported(followed_device && moved);
                }
                Action::None
            }
            ControlMessage::Level(level) => {
                self.live = level;
                Action::None
            }
            ControlMessage::Disabled(disabled) => {
                let toggled = disabled != self.disabled;
                self.disabled = disabled;
                if let Some(announcement) = &mut self.level_announcement {
                    announcement.device_reported(toggled);
                }
                Action::None
            }
            ControlMessage::Right => Action::Right,
            ControlMessage::Tooltip(_) => Action::None,
        }
    }

    #[must_use]
    pub fn view<'a>(
        &'a self,
        config: &'a Config,
        reveal: f32,
        clock: Clock,
    ) -> Element<'a, ControlMessage> {
        let expansion = config.panel_bar.slider_expansion;
        expand_animation(self.open, expansion, move |expand| {
            if expand > 0.0 {
                self.expanded_view(config, expand, clock)
            } else {
                self.collapsed_view(config, reveal, clock)
            }
        })
    }

    #[must_use]
    pub fn always_open_view<'a>(
        &self,
        config: &Config,
        clock: Clock,
        gap: f32,
    ) -> Element<'a, ControlMessage> {
        let opened = Row::with_children([
            self.state_glyph(config, clock),
            self.inline_slider_view(config),
            self.readout_view(config),
        ])
        .spacing(gap)
        .align_y(Alignment::Center);
        let mut area = mouse_area(opened).on_scroll(ControlMessage::Scroll);
        if self.right_click.is_some() {
            area = area.on_right_press(ControlMessage::Right);
        }
        area.into()
    }

    #[must_use]
    pub fn menu_view<'a>(&self, config: &Config, gap: f32) -> Element<'a, ControlMessage> {
        let opened =
            Row::with_children([self.inline_slider_view(config), self.readout_view(config)])
                .spacing(gap)
                .align_y(Alignment::Center);
        mouse_area(opened).on_scroll(ControlMessage::Scroll).into()
    }

    fn expanded_view<'a>(
        &self,
        config: &Config,
        expand: f32,
        clock: Clock,
    ) -> Element<'a, ControlMessage> {
        let closing_button = |content: Element<'a, ControlMessage>| -> Element<'a, ControlMessage> {
            button(content)
                .padding(0)
                .style(flat_button(text_color))
                .on_press(ControlMessage::Press)
                .into()
        };
        let gap = config.layout.panel.widget_spacing;
        let revealed = Row::with_children([
            self.inline_slider_view(config),
            closing_button(self.readout_view(config)),
        ])
        .spacing(gap)
        .align_y(Alignment::Center);
        let opened = Row::with_children([
            closing_button(self.state_glyph(config, clock)),
            reveal_x(container(revealed).padding(Padding::ZERO.left(gap)), expand),
        ])
        .align_y(Alignment::Center);
        let press_hints = PressHints {
            left: Some("close the slider"),
            right: self.right_click,
            scroll: Some("adjust"),
            ..PressHints::default()
        };
        self.scrollable_hover_area(opened.into(), press_hints)
    }

    fn state_glyph<'a>(&self, config: &Config, clock: Clock) -> Element<'a, ControlMessage> {
        let state = self.state(clock);
        self.effective_icon()
            .with_weight(state.weight())
            .view(config.theme.font.icon_size_px, move |theme| {
                state.color(theme)
            })
    }

    fn inline_slider_view<'a>(&self, config: &Config) -> Element<'a, ControlMessage> {
        inline_slider(
            config,
            self.range.clone(),
            self.current(),
            self.device_setting.lagging_device_value(self.slider_step),
            self.slider_step,
            self.severity(),
            ControlMessage::Set,
            ControlMessage::Release,
            ControlMessage::Scroll,
        )
    }

    fn readout_view<'a>(&self, config: &Config) -> Element<'a, ControlMessage> {
        widest_readout_text(
            ReadoutLabels {
                owner: self.target.clone(),
                range: self.range.clone(),
                step: self.slider_step,
                format: self.readout,
                locale: config.calendar.locale,
            },
            &config.theme.font,
            bar_text(
                &config.theme.font,
                self.readout_text(config.calendar.locale),
            )
            .wrapping(Wrapping::None)
            .width(Length::Fill)
            .align_x(Horizontal::Center)
            .style(text::base),
        )
        .into()
    }

    fn scrollable_hover_area<'a>(
        &self,
        content: Element<'a, ControlMessage>,
        press_hints: PressHints,
    ) -> Element<'a, ControlMessage> {
        let hover = hoverable(
            content,
            self.target.clone(),
            press_hints,
            ControlMessage::Tooltip,
        );
        let mut area = mouse_area(hover).on_scroll(ControlMessage::Scroll);
        if self.right_click.is_some() {
            area = area.on_right_press(ControlMessage::Right);
        }
        area.into()
    }

    fn collapsed_view<'a>(
        &self,
        config: &Config,
        reveal: f32,
        clock: Clock,
    ) -> Element<'a, ControlMessage> {
        let reading = match self.label {
            Some(_) => labeled_reading(config, self, reveal, clock),
            None => source_reading(config, self, reveal, clock),
        };
        let press_hints = PressHints {
            left: Some("open the slider"),
            right: self.right_click,
            scroll: Some("adjust"),
            ..PressHints::default()
        };
        let hover = hoverable(
            reading,
            self.target.clone(),
            press_hints,
            ControlMessage::Tooltip,
        );
        let mut area = mouse_area(hover)
            .on_press(ControlMessage::Press)
            .on_scroll(ControlMessage::Scroll);
        if self.right_click.is_some() {
            area = area.on_right_press(ControlMessage::Right);
        }
        area.into()
    }
}

impl BarWidget for SliderControl {
    fn name(&self) -> &'static str {
        self.name
    }

    fn icon(&self) -> Glyph {
        self.effective_icon()
    }

    fn label(&self, _clock: Clock, locale: Locale) -> Option<String> {
        self.label.and_then(|format| format(self.current(), locale))
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        if self.disabled {
            WidgetState::Off
        } else {
            self.severity().into()
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        self.gauge.then(|| self.range_fill())
    }

    fn live_gauge(&self) -> Option<Ratio> {
        self.gauge()
            .map(|fill| fill * Ratio::new::<ratio::ratio>(f64::from(self.live)))
    }

    fn announced_level(&self, locale: Locale) -> Option<AnnouncedLevel> {
        let announcement = self.level_announcement.as_ref()?;
        let readout = self.readout_text(locale);
        let body_text = if self.disabled {
            format!("Muted · {readout}")
        } else {
            readout
        };
        announcement.level(self.range_fill(), body_text)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([])
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
#[must_use]
pub fn percent_f(percent: i64) -> f32 {
    percent as f32
}

#[allow(clippy::cast_possible_truncation)]
#[must_use]
pub fn as_percent(value: f32) -> i64 {
    value.round() as i64
}
