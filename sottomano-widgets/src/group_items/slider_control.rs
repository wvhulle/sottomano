use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    ops::RangeInclusive,
    sync::Arc,
};

use chrono::Locale;
use corredo::{
    changing_text::{ReadoutLabels, steady_width_readout},
    transitions::ExpansionState,
    value_controls::{
        ExpandingSlider, ExpandingSliderParts, ScrollWrites, SecondaryPress, SliderEffect,
        SliderGesture, SliderMessage, SourceTracking, ValueWrite, WriteOutcome,
    },
};
use iced::{
    Alignment, Element, Length, Task,
    alignment::Horizontal,
    mouse::ScrollDelta,
    widget::{Row, mouse_area, text, text::Wrapping},
};
use sottomano_toml_config::{Config, severity::Severity};
use uom::si::{f64::Ratio, ratio};

use crate::{
    clock::Clock,
    group_items::{
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
        icon_glyph::IconGlyph,
        phosphor::Phosphor,
        slider_look::slider_look,
        text::{panel_text, text_face_of},
        value_cells::{LabeledValue, SliderReadout, ValueCell},
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
    pub(crate) expanding_slider: ExpandingSlider,
    slider_activity: SliderActivity,
    live: f32,
    readout: fn(f32, Locale) -> String,
    gauge: bool,
    label: Option<fn(f32, Locale) -> Option<String>>,
    icon: IconGlyph,
    glyph: Option<fn(f32, SliderActivity) -> Phosphor>,
    thresholds: Option<SeverityThresholds>,
    target: HoverTarget,
    right_click: Option<&'static str>,
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
    Rejected(Arc<dyn Error + Send + Sync>),
}

impl Display for ControlWriteFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ControlWriteFailure::Rejected(write_error) => write!(formatter, "{write_error}"),
        }
    }
}

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
            expanding_slider: ExpandingSlider::new(range),
            slider_activity: SliderActivity::Active,
            live: 1.0,
            readout: |value, _| format!("{value:.0}"),
            gauge: true,
            label: None,
            icon: icon.into(),
            glyph: None,
            thresholds: None,
            target: target.into(),
            right_click: None,
            level_announcement: None,
        }
    }

    #[must_use]
    pub fn readout(mut self, format: fn(f32, Locale) -> String) -> Self {
        self.readout = format;
        self
    }

    #[must_use]
    pub fn scroll_step(self, step: f32) -> Self {
        Self {
            expanding_slider: self.expanding_slider.scroll_step(step),
            ..self
        }
    }

    #[must_use]
    pub fn slider_step(self, step: f32) -> Self {
        Self {
            expanding_slider: self.expanding_slider.slider_step(step),
            ..self
        }
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
    pub fn right_click(self, does: &'static str) -> Self {
        Self {
            right_click: Some(does),
            expanding_slider: self
                .expanding_slider
                .secondary_press(SecondaryPress::Reported),
            ..self
        }
    }

    #[must_use]
    pub fn relative_scroll(self) -> Self {
        Self {
            expanding_slider: self.expanding_slider.scroll_writes(ScrollWrites::Relative),
            ..self
        }
    }

    #[must_use]
    pub fn announces_level(mut self, summary_text: &'static str) -> Self {
        self.level_announcement = Some(LevelAnnouncement::new(summary_text));
        self
    }

    fn range_fill(&self) -> Ratio {
        Ratio::new::<ratio::ratio>(f64::from(self.expanding_slider.fraction_of_range()))
    }

    pub fn close(&mut self) {
        self.expanding_slider.close();
    }

    #[must_use]
    pub fn target(&self) -> &HoverTarget {
        &self.target
    }

    pub fn set_icon(&mut self, icon: impl Into<IconGlyph>) {
        self.icon = icon.into();
    }

    #[must_use]
    pub fn readout_text(&self, locale: Locale) -> String {
        (self.readout)(self.expanding_slider.shown_value(), locale)
    }

    #[must_use]
    pub fn readout_cell(&self) -> ValueCell {
        ValueCell::Readout(SliderReadout::new(
            self.expanding_slider.shown_value(),
            self.readout,
        ))
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
                thresholds.level(self.expanding_slider.shown_value())
            })
    }

    fn effective_icon(&self) -> IconGlyph {
        self.glyph.map_or(self.icon, |glyph| {
            glyph(self.expanding_slider.shown_value(), self.slider_activity).into()
        })
    }

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
                        outcome: write_outcome_of(&write_outcome),
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
                        outcome: write_outcome_of(&write_outcome),
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
            ControlMessage::Reading(reading) => {
                let tracking_before = self.expanding_slider.source_tracking();
                let shown_before = self.expanding_slider.shown_value();
                self.expanding_slider
                    .update(SliderMessage::SourceReported(reading));
                let moved = (self.expanding_slider.shown_value() - shown_before).abs()
                    >= self.expanding_slider.slider_step_size() / 2.0;
                let change_origin = match tracking_before {
                    SourceTracking::FollowingSource if moved => LevelChangeOrigin::OutsidePanel,
                    SourceTracking::FollowingSource | SourceTracking::Diverging => {
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

    fn gestured(&mut self, gesture: SliderGesture) -> Action {
        into_action(
            self.expanding_slider
                .update(SliderMessage::Gesture(gesture)),
        )
    }

    #[must_use]
    pub fn view<'a>(
        &'a self,
        config: &'a Config,
        reveal: f32,
        clock: Clock,
        hover_origin: &HoverOrigin,
    ) -> Element<'a, ControlMessage> {
        let hover_origin = hover_origin.clone();
        let right_click = self.right_click;
        self.expanding_slider.view(
            ExpandingSliderParts {
                collapsed: Box::new(move || match self.label {
                    Some(_) => labeled_reading(config, self, reveal, clock),
                    None => source_reading(config, self, reveal, clock),
                }),
                face: Box::new(move || self.state_glyph(config, clock)),
                readout: Box::new(move || self.readout_view(config)),
                look: Box::new(move || slider_look(config, self.severity())),
                decorate: Box::new(move |content, expansion_state| {
                    let left = match expansion_state {
                        ExpansionState::Collapsed => "open the slider",
                        ExpansionState::Expanded => "close the slider",
                    };
                    announcing_hoverable(
                        content,
                        self.target.clone(),
                        HoverAnnouncement {
                            press_hints: PressHints {
                                left: Some(left),
                                right: right_click,
                                scroll: Some("adjust"),
                                ..PressHints::default()
                            },
                            origin: hover_origin.clone(),
                        },
                        ControlMessage::HoverCard,
                    )
                }),
                gap_px: config.theme.spacing.item_gap,
                expansion_duration: config.group_items.slider_expansion_duration,
            },
            control_message_of,
        )
    }

    #[must_use]
    pub fn always_open_view<'a>(
        &self,
        config: &Config,
        clock: Clock,
    ) -> Element<'a, ControlMessage> {
        let opened = Row::with_children([self.state_glyph(config, clock), self.menu_view(config)])
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
        self.expanding_slider.open_row(
            slider_look(config, self.severity()),
            self.readout_view(config),
            config.theme.spacing.item_gap,
            control_message_of,
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

    fn readout_view<'a>(&self, config: &Config) -> Element<'a, ControlMessage> {
        steady_width_readout(
            ReadoutLabels {
                owner: self.target.clone(),
                range: self.expanding_slider.range(),
                step: self.expanding_slider.slider_step_size(),
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
}

fn control_message_of(gesture: SliderGesture) -> ControlMessage {
    match gesture {
        SliderGesture::Toggle => ControlMessage::Press,
        SliderGesture::Scroll(delta) => ControlMessage::Scroll(delta),
        SliderGesture::Drag(position) => ControlMessage::Set(position),
        SliderGesture::Release => ControlMessage::Release,
        SliderGesture::SecondaryPress => ControlMessage::Right,
    }
}

fn write_outcome_of(write_outcome: &Result<(), ControlWriteFailure>) -> WriteOutcome {
    match write_outcome {
        Ok(()) => WriteOutcome::Accepted,
        Err(_) => WriteOutcome::Rejected,
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
        self.label
            .and_then(|format| format(self.expanding_slider.shown_value(), locale))
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

fn into_action(slider_effect: SliderEffect) -> Action {
    match slider_effect {
        SliderEffect::Write(ValueWrite::Absolute(value)) => Action::Commit(value),
        SliderEffect::Write(ValueWrite::Relative(delta)) => Action::Adjust(delta),
        SliderEffect::Opened => Action::Opened,
        SliderEffect::SecondaryPressed => Action::Right,
        SliderEffect::Closed | SliderEffect::Unchanged => Action::None,
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
