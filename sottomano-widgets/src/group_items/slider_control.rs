use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    ops::RangeInclusive,
    sync::Arc,
};

use chrono::Locale;
use corredo::primitives::expand_animation::{ExpansionState, expand_animation};
pub use corredo::primitives::scroll_steps::scroll_direction;
use iced::{
    Alignment, Element, Length, Padding, Task,
    alignment::Horizontal,
    mouse::ScrollDelta,
    widget::{Row, button, container, mouse_area, text, text::Wrapping},
};
use sottomano_desktop_sources::blocking_background_work::{Panicked, blocking_poll};
use sottomano_toml_config::{Config, severity::Severity};
use uom::si::{f64::Ratio, ratio};

use crate::{
    clock::Clock,
    group_items::{
        device_setting::{DeviceSetting, DeviceTracking, Nudge, Write},
        group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
        source_reading::{labeled_reading, source_reading},
    },
    hover_cards::{
        HoverAnnouncement, HoverCardMessage, HoverOrigin, HoverTarget, PressHints,
        announcing_hoverable,
        hover_card_body::{
            HoverCardBody,
            hover_card_source_delivery::{DeliveryTiming, HoverCard},
        },
    },
    widgets::{
        announced_level::{AnnouncedLevel, LevelAnnouncement, LevelChangeOrigin},
        configured_theme::text_color,
        flat_button,
        icon_glyph::IconGlyph,
        inline_slider::inline_slider,
        phosphor::Phosphor,
        reveal_clip::reveal_x,
        text::{panel_text, text_face_of},
        value_cells::{LabeledValue, SliderReadout, ValueCell},
        widest_readout_text::{ReadoutLabels, widest_readout_text},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderActivity {
    Active,
    Inactive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeverityDirection {
    HigherWorse,
    LowerWorse,
}

pub struct SliderControl {
    name: &'static str,
    expansion_state: ExpansionState,
    accum: f32,
    device_setting: DeviceSetting,
    slider_activity: SliderActivity,
    live: f32,
    range: RangeInclusive<f32>,
    scroll_step: f32,
    slider_step: f32,
    readout: fn(f32, Locale) -> String,
    gauge: bool,
    label: Option<fn(f32, Locale) -> Option<String>>,
    icon: IconGlyph,
    glyph: Option<fn(f32, SliderActivity) -> Phosphor>,
    thresholds: Option<SeverityThresholds>,
    target: HoverTarget,
    right_click: Option<&'static str>,
    relative_scroll: bool,
    level_announcement: Option<LevelAnnouncement>,
}

struct SeverityThresholds {
    warn: f32,
    urgent: f32,
    severity_direction: SeverityDirection,
}

impl SeverityThresholds {
    fn level(&self, value: f32) -> Severity {
        let hit = |threshold: f32| match self.severity_direction {
            SeverityDirection::HigherWorse => value >= threshold,
            SeverityDirection::LowerWorse => value <= threshold,
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
    DeviceAccepted {
        value: f32,
        write_outcome: Result<(), ControlWriteFailure>,
    },
    Persisted {
        value: f32,
        write_outcome: Result<(), ControlWriteFailure>,
    },
    Adjusted(f32),
    AdjustFailed,
    Reading(f32),
    Level(f32),
    Activity(SliderActivity),
    Right,
    HoverCard(HoverCardMessage),
}

#[derive(Debug, Clone)]
pub enum ControlWriteFailure {
    WorkerPanicked,
    Rejected(Arc<dyn Error + Send + Sync>),
}

impl From<Panicked> for ControlWriteFailure {
    fn from(_: Panicked) -> Self {
        ControlWriteFailure::WorkerPanicked
    }
}

impl Display for ControlWriteFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ControlWriteFailure::WorkerPanicked => formatter.write_str("the write task panicked"),
            ControlWriteFailure::Rejected(write_error) => write!(formatter, "{write_error}"),
        }
    }
}

impl ControlMessage {
    pub fn commit<WriteError: Error + Send + Sync + 'static>(
        label: &'static str,
        value: f32,
        write: impl FnOnce() -> Result<(), WriteError> + Send + 'static,
    ) -> Task<ControlMessage> {
        let device_write = blocking_poll(move || {
            write().map_err(|write_error| ControlWriteFailure::Rejected(Arc::new(write_error)))
        });
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
        icon: impl Into<IconGlyph>,
    ) -> Self {
        Self {
            name,
            expansion_state: ExpansionState::Collapsed,
            accum: 0.0,
            device_setting: DeviceSetting::default(),
            slider_activity: SliderActivity::Active,
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
    pub fn glyph(mut self, glyph: fn(f32, SliderActivity) -> Phosphor) -> Self {
        self.glyph = Some(glyph);
        self
    }

    #[must_use]
    pub fn thresholds(
        mut self,
        warn: f32,
        urgent: f32,
        severity_direction: SeverityDirection,
    ) -> Self {
        self.thresholds = Some(SeverityThresholds {
            warn,
            urgent,
            severity_direction,
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
        self.expansion_state = ExpansionState::Collapsed;
    }

    #[must_use]
    pub fn target(&self) -> &HoverTarget {
        &self.target
    }

    pub fn set_icon(&mut self, icon: impl Into<IconGlyph>) {
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

    pub fn readout_hover_card(&self, label: impl Into<ValueCell>) -> HoverCard {
        HoverCard::sourced(
            HoverCardBody::dictionary([LabeledValue::new(label, self.readout_cell())]),
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

    fn effective_icon(&self) -> IconGlyph {
        self.glyph.map_or(self.icon, |glyph| {
            glyph(self.current(), self.slider_activity).into()
        })
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn update(&mut self, msg: ControlMessage) -> Action {
        match msg {
            ControlMessage::Press => match self.expansion_state {
                ExpansionState::Collapsed => {
                    self.expansion_state = ExpansionState::Expanded;
                    Action::Opened
                }
                ExpansionState::Expanded => {
                    self.expansion_state = ExpansionState::Collapsed;
                    Action::None
                }
            },
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
            ControlMessage::DeviceAccepted {
                value,
                write_outcome,
            } => into_action(
                self.device_setting
                    .absolute_write_finished(value, &write_outcome),
            ),
            ControlMessage::Persisted {
                value,
                write_outcome,
            } => {
                let follow_up = self
                    .device_setting
                    .absolute_write_finished(value, &write_outcome);
                match write_outcome {
                    Ok(()) => self.device_setting.device_reported(value, self.slider_step),
                    Err(store_failure) => tracing::warn!("slider value not saved: {store_failure}"),
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
                let tracking_before = self.device_setting.device_tracking();
                let shown_before = self.current();
                self.device_setting
                    .device_reported(reading, self.slider_step);
                let moved = (self.current() - shown_before).abs() >= self.slider_step / 2.0;
                let change_origin = match tracking_before {
                    DeviceTracking::FollowingDevice if moved => LevelChangeOrigin::OutsidePanel,
                    DeviceTracking::FollowingDevice | DeviceTracking::Diverging => {
                        LevelChangeOrigin::FromPanel
                    }
                };
                if let Some(announcement) = &mut self.level_announcement {
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
                if let Some(announcement) = &mut self.level_announcement {
                    announcement.device_reported(change_origin);
                }
                Action::None
            }
            ControlMessage::Right => Action::Right,
            ControlMessage::HoverCard(_) => Action::None,
        }
    }

    #[must_use]
    pub fn view<'a>(
        &'a self,
        config: &'a Config,
        reveal: f32,
        clock: Clock,
        hover_origin: &HoverOrigin,
    ) -> Element<'a, ControlMessage> {
        let expansion = config.group_items.slider_expansion_duration;
        let hover_origin = hover_origin.clone();
        expand_animation(self.expansion_state, expansion, move |expand| {
            if expand > 0.0 {
                self.expanded_view(config, expand, clock, hover_origin.clone())
            } else {
                self.collapsed_view(config, reveal, clock, hover_origin.clone())
            }
        })
    }

    #[must_use]
    pub fn always_open_view<'a>(
        &self,
        config: &Config,
        clock: Clock,
    ) -> Element<'a, ControlMessage> {
        let opened = Row::with_children([
            self.state_glyph(config, clock),
            self.inline_slider_view(config),
            self.readout_view(config),
        ])
        .spacing(config.theme.spacing.item_gap)
        .align_y(Alignment::Center);
        let mut area = mouse_area(opened).on_scroll(ControlMessage::Scroll);
        if self.right_click.is_some() {
            area = area.on_right_press(ControlMessage::Right);
        }
        area.into()
    }

    #[must_use]
    pub fn menu_view<'a>(&self, config: &Config) -> Element<'a, ControlMessage> {
        let opened =
            Row::with_children([self.inline_slider_view(config), self.readout_view(config)])
                .spacing(config.theme.spacing.item_gap)
                .align_y(Alignment::Center);
        mouse_area(opened).on_scroll(ControlMessage::Scroll).into()
    }

    fn expanded_view<'a>(
        &self,
        config: &Config,
        expand: f32,
        clock: Clock,
        hover_origin: HoverOrigin,
    ) -> Element<'a, ControlMessage> {
        let closing_button = |content: Element<'a, ControlMessage>| -> Element<'a, ControlMessage> {
            button(content)
                .padding(0)
                .style(flat_button(text_color))
                .on_press(ControlMessage::Press)
                .into()
        };
        let gap = config.theme.spacing.item_gap;
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
        self.scrollable_hover_area(
            opened.into(),
            HoverAnnouncement {
                press_hints,
                origin: hover_origin,
            },
        )
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
                context: config.calendar.locale,
            },
            text_face_of(&config.theme.font),
            panel_text(
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
        announcement: HoverAnnouncement,
    ) -> Element<'a, ControlMessage> {
        let hover = announcing_hoverable(
            content,
            self.target.clone(),
            announcement,
            ControlMessage::HoverCard,
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
        hover_origin: HoverOrigin,
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
        let hover = announcing_hoverable(
            reading,
            self.target.clone(),
            HoverAnnouncement {
                press_hints,
                origin: hover_origin,
            },
            ControlMessage::HoverCard,
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

impl GroupItemReading for SliderControl {
    fn name(&self) -> &'static str {
        self.name
    }

    fn icon(&self) -> IconGlyph {
        self.effective_icon()
    }

    fn label(&self, _clock: Clock, locale: Locale) -> Option<String> {
        self.label.and_then(|format| format(self.current(), locale))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.slider_activity {
            SliderActivity::Active => self.severity().into(),
            SliderActivity::Inactive => ReadingDisplayState::Off,
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
        let body_text = match self.slider_activity {
            SliderActivity::Active => readout,
            SliderActivity::Inactive => format!("Muted · {readout}"),
        };
        announcement.level(self.range_fill(), body_text)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
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
