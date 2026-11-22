mod slider_control_indicator;
mod slider_control_updates;
mod slider_control_views;

use std::{
    borrow::Cow,
    error::Error,
    fmt::{self, Display, Formatter},
    ops::RangeInclusive,
    sync::Arc,
};

use chrono::Locale;
use corredo::{
    transitions::ExpansionState,
    value_controls::{ExpandingSlider, ScrollWrites, SecondaryPress},
};
use iced::mouse::ScrollDelta;
use uom::si::{f64::Ratio, ratio};

use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::indicator_drawing::{IndicatorFace, IndicatorReadout},
    hover_cards::{
        MouseGesture, PressAction,
        hover_card_body::{Section, dictionary_table},
    },
    widgets::{
        announced_level::LevelAnnouncement,
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

impl From<SliderActivity> for Weight {
    fn from(activity: SliderActivity) -> Self {
        match activity {
            SliderActivity::Active => Weight::Fill,
            SliderActivity::Inactive => Weight::Regular,
        }
    }
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
    gauge: SliderGauge,
    label_format: SliderLabelFormat,
    icon: IconGlyph,
    glyph_source: SliderGlyphSource,
    severity_scale: SliderSeverityScale,
    right_click: SliderRightClick,
    level_announcing: SliderLevelAnnouncing,
    pub caption: SliderCaption,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SliderGauge {
    Gauged,
    Ungauged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SliderCaption {
    Uncaptioned,
    Captioned(Cow<'static, str>),
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
            gauge: SliderGauge::Gauged,
            label_format: SliderLabelFormat::Unlabeled,
            icon: icon.into(),
            glyph_source: SliderGlyphSource::FixedIcon,
            severity_scale: SliderSeverityScale::Unrated,
            right_click: SliderRightClick::Unbound,
            level_announcing: SliderLevelAnnouncing::Silent,
            caption: SliderCaption::Uncaptioned,
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
        self.gauge = SliderGauge::Ungauged;
        self
    }

    #[must_use]
    pub fn label_with_gauge(mut self, format: fn(f32, Locale) -> Option<String>) -> Self {
        self.label_format = SliderLabelFormat::Labeled(format);
        self
    }

    #[must_use]
    pub fn icon_only(mut self) -> Self {
        self.gauge = SliderGauge::Ungauged;
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
        (self.readout)(self.expanding_slider.synced_value.shown_value(), locale)
    }

    #[must_use]
    pub fn readout_cell(&self) -> ValueCell {
        ValueCell::Readout(SliderReadout::new(
            self.expanding_slider.synced_value.shown_value(),
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
