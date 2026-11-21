use std::borrow::Cow;

use corredo::changing_text::{LabelShape, steady_width_text};
use iced::{
    Alignment, Element,
    widget::{Row, row, text},
};
use sottomano_toml_config::Config;
use uom::si::f64::Ratio;

use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        gauge::Gauge,
        indicator_state::{IndicatorState, IndicatorStateTint},
    },
    icon_catalog::icon_handle::IconHandle,
    widgets::text::{panel_text, resolved_font_faces},
};

pub(crate) fn state_glyph_view<'view, Message: 'view>(
    glyph: IconGlyph,
    state: IndicatorState,
    config: &Config,
    opacity: f32,
) -> Element<'view, Message> {
    let tint = IndicatorStateTint { state, opacity };
    glyph
        .with_weight(state.weight())
        .view(config.theme.font.icon_size_px, move |theme| {
            tint.color(theme)
        })
}

#[derive(Debug, Clone, PartialEq)]
pub enum IndicatorIcon {
    IconGlyph {
        glyph: IconGlyph,
        state: IndicatorState,
    },
    Image(IconHandle),
    Absent,
}

impl IndicatorIcon {
    #[must_use]
    pub fn display_state(&self) -> IndicatorState {
        match self {
            IndicatorIcon::IconGlyph { state, .. } => *state,
            IndicatorIcon::Image(_) | IndicatorIcon::Absent => IndicatorState::Idle,
        }
    }

    pub(crate) fn view<'view, Message: 'view>(
        &self,
        config: &Config,
        opacity: f32,
    ) -> Option<Element<'view, Message>> {
        let side = config.theme.font.icon_size_px;
        match self {
            IndicatorIcon::IconGlyph { glyph, state } => {
                Some(state_glyph_view(*glyph, *state, config, opacity))
            }
            IndicatorIcon::Image(handle) => Some(handle.view(side, opacity)),
            IndicatorIcon::Absent => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityOrdinal {
    Unnumbered,
    Numbered(u8),
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndicatorFace {
    pub name: Cow<'static, str>,
    pub icon: IndicatorIcon,
    pub ordinal: IdentityOrdinal,
}

impl IndicatorFace {
    fn tint(&self, opacity: f32) -> IndicatorStateTint {
        IndicatorStateTint {
            state: self.icon.display_state(),
            opacity,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReadoutLabel {
    Unlabeled,
    Labeled(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReadoutGauge {
    Ungauged,
    Gauged { fill: Ratio, live: Ratio },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadoutBadge {
    Unbadged,
    Counted(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndicatorReadout {
    pub label: ReadoutLabel,
    pub gauge: ReadoutGauge,
    pub badge: ReadoutBadge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadoutPreference {
    GaugeFirst,
    LabelOnly,
}

pub fn indicator_drawing<'view, Message: 'view>(
    config: &Config,
    face: &IndicatorFace,
    readout: &IndicatorReadout,
    opacity: f32,
) -> Element<'view, Message> {
    composed_indicator(
        config,
        face,
        readout,
        opacity,
        ReadoutPreference::GaugeFirst,
    )
}

pub fn labeled_indicator<'view, Message: 'view>(
    config: &Config,
    face: &IndicatorFace,
    readout: &IndicatorReadout,
    opacity: f32,
) -> Element<'view, Message> {
    composed_indicator(config, face, readout, opacity, ReadoutPreference::LabelOnly)
}

fn composed_indicator<'view, Message: 'view>(
    config: &Config,
    face: &IndicatorFace,
    readout: &IndicatorReadout,
    opacity: f32,
    preference: ReadoutPreference,
) -> Element<'view, Message> {
    let tint = face.tint(opacity);
    let icon = face_icon(config, face, opacity).unwrap_or_else(|| Row::new().into());
    let sample = match preferred_value(config, face, readout, tint, preference) {
        Some(value) => row![icon, value]
            .spacing(config.theme.spacing.item_gap)
            .align_y(Alignment::Center)
            .into(),
        None => icon,
    };
    if face.icon.display_state() == IndicatorState::Failed {
        return sample;
    }
    badged(config, sample, readout.badge, tint)
}

pub fn face_icon<'view, Message: 'view>(
    config: &Config,
    face: &IndicatorFace,
    opacity: f32,
) -> Option<Element<'view, Message>> {
    let icon = face.icon.view(config, opacity)?;
    Some(match face.ordinal {
        IdentityOrdinal::Numbered(ordinal) => superscripted(
            config,
            icon,
            ordinal.to_string(),
            IndicatorStateTint {
                state: IndicatorState::Off,
                opacity,
            },
        ),
        IdentityOrdinal::Unnumbered => icon,
    })
}

pub(crate) fn badged_face_icon<'view, Message: 'view>(
    config: &Config,
    face: &IndicatorFace,
    badge: ReadoutBadge,
) -> Element<'view, Message> {
    let icon = face_icon(config, face, 1.0).unwrap_or_else(|| Row::new().into());
    badged(config, icon, badge, face.tint(1.0))
}

#[must_use]
pub fn readout_view<'view, Message: 'view>(
    config: &Config,
    face: &IndicatorFace,
    readout: &IndicatorReadout,
    opacity: f32,
) -> Option<Element<'view, Message>> {
    preferred_value(
        config,
        face,
        readout,
        face.tint(opacity),
        ReadoutPreference::GaugeFirst,
    )
}

fn preferred_value<'view, Message: 'view>(
    config: &Config,
    face: &IndicatorFace,
    readout: &IndicatorReadout,
    tint: IndicatorStateTint,
    preference: ReadoutPreference,
) -> Option<Element<'view, Message>> {
    if face.icon.display_state() == IndicatorState::Failed {
        return Some(failure_marker(config, tint));
    }
    match preference {
        ReadoutPreference::GaugeFirst => match readout.gauge {
            ReadoutGauge::Gauged { fill, live } => Some(
                Gauge::new(fill, tint, config.theme.font.icon_size_px)
                    .live(live)
                    .into(),
            ),
            ReadoutGauge::Ungauged => label_text(config, face, &readout.label, tint),
        },
        ReadoutPreference::LabelOnly => label_text(config, face, &readout.label, tint),
    }
}

fn label_text<'view, Message: 'view>(
    config: &Config,
    face: &IndicatorFace,
    label: &ReadoutLabel,
    tint: IndicatorStateTint,
) -> Option<Element<'view, Message>> {
    let ReadoutLabel::Labeled(label) = label else {
        return None;
    };
    let shape = LabelShape {
        widget_name: face.name.clone(),
        character_count: label.chars().count(),
    };
    Some(
        steady_width_text(
            shape,
            panel_text(&config.theme.font, label.clone()).style(tint.text_style()),
        )
        .into(),
    )
}

fn failure_marker<'view, Message: 'view>(
    config: &Config,
    tint: IndicatorStateTint,
) -> Element<'view, Message> {
    IconGlyph::from(Phosphor::Warning(Weight::Thin))
        .view(config.theme.font.icon_size_px, move |theme| {
            tint.color(theme)
        })
}

fn badged<'view, Message: 'view>(
    config: &Config,
    sample: Element<'view, Message>,
    badge: ReadoutBadge,
    tint: IndicatorStateTint,
) -> Element<'view, Message> {
    match badge {
        ReadoutBadge::Counted(count) => superscripted(config, sample, count.to_string(), tint),
        ReadoutBadge::Unbadged => sample,
    }
}

fn superscripted<'view, Message: 'view>(
    config: &Config,
    marked: Element<'view, Message>,
    digits: String,
    tint: IndicatorStateTint,
) -> Element<'view, Message> {
    let superscript = text(digits)
        .size(config.theme.font.small_text_size_px)
        .font(resolved_font_faces(&config.theme.font).regular)
        .style(tint.text_style());
    Row::with_children([marked, superscript.into()])
        .align_y(Alignment::Start)
        .into()
}
