use std::{
    borrow::Cow,
    error::Error,
    fmt::{self, Display, Formatter},
    ops::RangeInclusive,
    sync::Arc,
};

use chrono::Locale;
use corredo::{
    changing_text::{MeasuredReadout, ReadoutLabels, steady_width_text},
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
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{
            IdentityOrdinal, IndicatorFace, IndicatorIcon, IndicatorReadout, ReadoutBadge,
            ReadoutGauge, ReadoutLabel, indicator_drawing, labeled_indicator,
        },
        indicator_state::IndicatorState,
    },
    hover_cards::{
        MouseGesture, PressAction,
        hover_card_body::{Section, dictionary_table},
    },
    widgets::{
        announced_level::{LevelAnnouncement, LevelChangeOrigin},
        icon_glyph::IconGlyph,
        ongoing_toast::{IndicatorAlerts, LevelNotice},
        phosphor::Phosphor,
        slider_look::slider_look,
        text::{panel_text, text_face_of},
        value_cells::{LabeledValue, SliderReadout, ValueCell},
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct SliderShownIndicator {
    pub face: IndicatorFace,
    pub readout: IndicatorReadout,
}

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
    pub name: Cow<'static, str>,
    pub(crate) expanding_slider: ExpandingSlider,
    slider_activity: SliderActivity,
    live: f32,
    readout: fn(f32, Locale) -> String,
    gauge: bool,
    label_format: SliderLabelFormat,
    icon: IconGlyph,
    glyph_source: SliderGlyphSource,
    severity_scale: SliderSeverityScale,
    right_click: SliderRightClick,
    level_announcing: SliderLevelAnnouncing,
}

#[derive(Clone, Copy)]
enum SliderLabelFormat {
    Unlabeled,
    Labeled(fn(f32, Locale) -> Option<String>),
}

#[derive(Clone, Copy)]
enum SliderGlyphSource {
    FixedIcon,
    ValueDriven(fn(f32, SliderActivity) -> Phosphor),
}

enum SliderSeverityScale {
    Unrated,
    Thresholded(SeverityThresholds),
}

#[derive(Clone, Copy)]
enum SliderRightClick {
    Unbound,
    Bound(&'static str),
}

enum SliderLevelAnnouncing {
    Silent,
    Announcing(LevelAnnouncement),
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
    Sample(f32),
    Level(f32),
    Activity(SliderActivity),
    Right,
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
        name: impl Into<Cow<'static, str>>,
        range: RangeInclusive<f32>,
        icon: impl Into<IconGlyph>,
    ) -> Self {
        Self {
            name: name.into(),
            expanding_slider: ExpandingSlider::new(range),
            slider_activity: SliderActivity::Active,
            live: 1.0,
            readout: |value, _| format!("{value:.0}"),
            gauge: true,
            label_format: SliderLabelFormat::Unlabeled,
            icon: icon.into(),
            glyph_source: SliderGlyphSource::FixedIcon,
            severity_scale: SliderSeverityScale::Unrated,
            right_click: SliderRightClick::Unbound,
            level_announcing: SliderLevelAnnouncing::Silent,
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
        self.label_format = SliderLabelFormat::Labeled(format);
        self.gauge = false;
        self
    }

    #[must_use]
    pub fn label_with_gauge(mut self, format: fn(f32, Locale) -> Option<String>) -> Self {
        self.label_format = SliderLabelFormat::Labeled(format);
        self
    }

    #[must_use]
    pub fn icon_only(mut self) -> Self {
        self.gauge = false;
        self
    }

    #[must_use]
    pub fn glyph(mut self, glyph: fn(f32, SliderActivity) -> Phosphor) -> Self {
        self.glyph_source = SliderGlyphSource::ValueDriven(glyph);
        self
    }

    #[must_use]
    pub fn thresholds(
        mut self,
        warn: f32,
        urgent: f32,
        severity_direction: SeverityDirection,
    ) -> Self {
        self.severity_scale = SliderSeverityScale::Thresholded(SeverityThresholds {
            warn,
            urgent,
            severity_direction,
        });
        self
    }

    #[must_use]
    pub fn right_click(self, does: &'static str) -> Self {
        Self {
            right_click: SliderRightClick::Bound(does),
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
        self.level_announcing =
            SliderLevelAnnouncing::Announcing(LevelAnnouncement::new(summary_text));
        self
    }

    fn range_fill(&self) -> Ratio {
        Ratio::new::<ratio::ratio>(f64::from(self.expanding_slider.fraction_of_range()))
    }

    pub fn close(&mut self) {
        self.expanding_slider.close();
    }

    #[must_use]
    pub fn press_actions(&self) -> Vec<PressAction> {
        let open_or_close = match self.expanding_slider.expansion_state {
            ExpansionState::Collapsed => "open the slider",
            ExpansionState::Expanded => "close the slider",
        };
        [
            PressAction::new(MouseGesture::LeftClick, open_or_close),
            PressAction::new(MouseGesture::Scroll, "adjust"),
        ]
        .into_iter()
        .chain(match self.right_click {
            SliderRightClick::Bound(does) => Some(PressAction::new(MouseGesture::RightClick, does)),
            SliderRightClick::Unbound => None,
        })
        .collect()
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

    pub(crate) fn readout_hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Value",
            dictionary_table([LabeledValue::new(
                self.name.clone().into_owned(),
                self.readout_cell(),
            )]),
        )]
    }

    fn severity(&self) -> Severity {
        match &self.severity_scale {
            SliderSeverityScale::Thresholded(thresholds) => {
                thresholds.level(self.expanding_slider.shown_value())
            }
            SliderSeverityScale::Unrated => Severity::Normal,
        }
    }

    #[must_use]
    pub fn shown_state(&self) -> IndicatorState {
        match self.slider_activity {
            SliderActivity::Active => self.severity().into(),
            SliderActivity::Inactive => IndicatorState::Off,
        }
    }

    fn shown_label(&self, locale: Locale) -> ReadoutLabel {
        let SliderLabelFormat::Labeled(format) = self.label_format else {
            return ReadoutLabel::Unlabeled;
        };
        match format(self.expanding_slider.shown_value(), locale) {
            Some(label) if !label.is_empty() => ReadoutLabel::Labeled(label),
            Some(_) | None => ReadoutLabel::Unlabeled,
        }
    }

    fn shown_gauge(&self) -> ReadoutGauge {
        if !self.gauge {
            return ReadoutGauge::Ungauged;
        }
        let fill = self.range_fill();
        ReadoutGauge::Gauged {
            fill,
            live: fill * Ratio::new::<ratio::ratio>(f64::from(self.live)),
        }
    }

    fn level_notice(&self, locale: Locale) -> LevelNotice {
        let SliderLevelAnnouncing::Announcing(announcement) = &self.level_announcing else {
            return LevelNotice::Unannounced;
        };
        let readout = self.readout_text(locale);
        let body_text = match self.slider_activity {
            SliderActivity::Active => readout,
            SliderActivity::Inactive => format!("Muted · {readout}"),
        };
        match announcement.level(self.range_fill(), body_text) {
            Some(level) => LevelNotice::Announced {
                level,
                glyph: self
                    .effective_icon()
                    .with_weight(self.shown_state().weight()),
            },
            None => LevelNotice::Unannounced,
        }
    }

    #[must_use]
    pub fn indicator(&self, locale: Locale) -> GroupIndicator<'_> {
        GroupIndicator::glyph_only(self.name.clone(), self.effective_icon(), self.shown_state())
            .with_readout(self.indicator_readout(locale))
            .with_alerts(self.indicator_alerts(locale))
            .with_hover_card(Box::new(|_| self.readout_hover_card()))
    }

    pub(crate) fn indicator_face(&self) -> IndicatorFace {
        IndicatorFace {
            name: self.name.clone(),
            icon: IndicatorIcon::IconGlyph {
                glyph: self.effective_icon(),
                state: self.shown_state(),
            },
            ordinal: IdentityOrdinal::Unnumbered,
        }
    }

    pub(crate) fn indicator_readout(&self, locale: Locale) -> IndicatorReadout {
        IndicatorReadout {
            label: self.shown_label(locale),
            gauge: self.shown_gauge(),
            badge: ReadoutBadge::Unbadged,
        }
    }

    pub(crate) fn indicator_alerts(&self, locale: Locale) -> IndicatorAlerts {
        IndicatorAlerts {
            level: self.level_notice(locale),
            ..IndicatorAlerts::default()
        }
    }

    fn effective_icon(&self) -> IconGlyph {
        match self.glyph_source {
            SliderGlyphSource::ValueDriven(glyph) => {
                glyph(self.expanding_slider.shown_value(), self.slider_activity).into()
            }
            SliderGlyphSource::FixedIcon => self.icon,
        }
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
            ControlMessage::Sample(sample) => {
                let tracking_before = self.expanding_slider.source_tracking();
                let shown_before = self.expanding_slider.shown_value();
                self.expanding_slider
                    .update(SliderMessage::SourceReported(sample));
                let moved = (self.expanding_slider.shown_value() - shown_before).abs()
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

    #[must_use]
    pub fn view<'a>(
        &'a self,
        config: &'a Config,
        shown_indicator: SliderShownIndicator,
        reveal: f32,
    ) -> Element<'a, ControlMessage> {
        let SliderShownIndicator { face, readout } = shown_indicator;
        self.expanding_slider.view(
            ExpandingSliderParts {
                collapsed: Box::new(move || match self.label_format {
                    SliderLabelFormat::Labeled(_) => {
                        labeled_indicator(config, &face, &readout, reveal)
                    }
                    SliderLabelFormat::Unlabeled => {
                        indicator_drawing(config, &face, &readout, reveal)
                    }
                }),
                face: Box::new(move || self.state_glyph(config)),
                readout: Box::new(move || self.readout_view(config)),
                look: Box::new(move || slider_look(config, self.severity())),
                decorate: Box::new(|content, _| content),
                gap_px: config.theme.spacing.item_gap,
                expansion_duration: config.group_items.slider_expansion_duration,
            },
            control_message_of,
        )
    }

    #[must_use]
    pub fn always_open_view<'a>(&self, config: &Config) -> Element<'a, ControlMessage> {
        let opened = Row::with_children([self.state_glyph(config), self.menu_view(config)])
            .spacing(config.theme.spacing.item_gap)
            .align_y(Alignment::Center);
        let mut area = mouse_area(opened).on_scroll(ControlMessage::Scroll);
        if let SliderRightClick::Bound(_) = self.right_click {
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

    fn state_glyph<'a>(&self, config: &Config) -> Element<'a, ControlMessage> {
        let state = self.shown_state();
        self.effective_icon()
            .with_weight(state.weight())
            .view(config.theme.font.icon_size_px, move |theme| {
                state.color(theme)
            })
    }

    fn readout_view<'a>(&self, config: &Config) -> Element<'a, ControlMessage> {
        steady_width_text(
            MeasuredReadout::new(
                ReadoutLabels {
                    owner: self.name.clone(),
                    range: self.expanding_slider.range.clone(),
                    step: self.expanding_slider.slider_step,
                    format: self.readout,
                    context: config.calendar.locale,
                },
                text_face_of(&config.theme.font),
            ),
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
