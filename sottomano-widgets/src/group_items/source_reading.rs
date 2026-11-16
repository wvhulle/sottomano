use std::borrow::Cow;

use corredo::changing_text::{LabelShape, steady_width_text};
use iced::{
    Alignment, Element,
    widget::{Row, row, text},
};
use sottomano_toml_config::Config;
use uom::si::f64::Ratio;

use crate::{
    group_items::{
        gauge::Gauge,
        reading_display_state::{ReadingDisplayState, ReadingStateTint},
    },
    widgets::{
        icon_glyph::IconGlyph,
        icon_handle::IconHandle,
        phosphor::{Phosphor, Weight},
        text::{font_faces_of, panel_text},
    },
};

pub(crate) fn state_glyph_view<'a, Message: 'a>(
    glyph: IconGlyph,
    state: ReadingDisplayState,
    config: &Config,
    opacity: f32,
) -> Element<'a, Message> {
    let tint = ReadingStateTint { state, opacity };
    glyph
        .with_weight(state.weight())
        .view(config.theme.font.icon_size_px, move |theme| {
            tint.color(theme)
        })
}

#[derive(Debug, Clone, PartialEq)]
pub enum GroupItemIcon {
    IconGlyph {
        glyph: IconGlyph,
        state: ReadingDisplayState,
    },
    Image(IconHandle),
    Absent,
}

impl GroupItemIcon {
    #[must_use]
    pub fn display_state(&self) -> ReadingDisplayState {
        match self {
            GroupItemIcon::IconGlyph { state, .. } => *state,
            GroupItemIcon::Image(_) | GroupItemIcon::Absent => ReadingDisplayState::Idle,
        }
    }

    pub(crate) fn view<'a, Message: 'a>(
        &self,
        config: &Config,
        opacity: f32,
    ) -> Option<Element<'a, Message>> {
        let side = config.theme.font.icon_size_px;
        match self {
            GroupItemIcon::IconGlyph { glyph, state } => {
                Some(state_glyph_view(*glyph, *state, config, opacity))
            }
            GroupItemIcon::Image(handle) => Some(handle.view(side, opacity)),
            GroupItemIcon::Absent => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityOrdinal {
    Unnumbered,
    Numbered(u8),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowFace {
    pub name: Cow<'static, str>,
    pub icon: GroupItemIcon,
    pub ordinal: IdentityOrdinal,
}

impl RowFace {
    fn tint(&self, opacity: f32) -> ReadingStateTint {
        ReadingStateTint {
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
pub struct RowReadout {
    pub label: ReadoutLabel,
    pub gauge: ReadoutGauge,
    pub badge: ReadoutBadge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadoutPreference {
    GaugeFirst,
    LabelOnly,
}

pub fn source_reading<'a, Message: 'a>(
    config: &Config,
    face: &RowFace,
    readout: &RowReadout,
    opacity: f32,
) -> Element<'a, Message> {
    composed_reading(
        config,
        face,
        readout,
        opacity,
        ReadoutPreference::GaugeFirst,
    )
}

pub fn labeled_reading<'a, Message: 'a>(
    config: &Config,
    face: &RowFace,
    readout: &RowReadout,
    opacity: f32,
) -> Element<'a, Message> {
    composed_reading(config, face, readout, opacity, ReadoutPreference::LabelOnly)
}

fn composed_reading<'a, Message: 'a>(
    config: &Config,
    face: &RowFace,
    readout: &RowReadout,
    opacity: f32,
    preference: ReadoutPreference,
) -> Element<'a, Message> {
    let tint = face.tint(opacity);
    let icon = face_icon(config, face, opacity).unwrap_or_else(|| Row::new().into());
    let reading = match preferred_value(config, face, readout, tint, preference) {
        Some(value) => row![icon, value]
            .spacing(config.theme.spacing.item_gap)
            .align_y(Alignment::Center)
            .into(),
        None => icon,
    };
    if face.icon.display_state() == ReadingDisplayState::Failed {
        return reading;
    }
    badged(config, reading, readout.badge, tint)
}

pub fn face_icon<'a, Message: 'a>(
    config: &Config,
    face: &RowFace,
    opacity: f32,
) -> Option<Element<'a, Message>> {
    let icon = face.icon.view(config, opacity)?;
    Some(match face.ordinal {
        IdentityOrdinal::Numbered(ordinal) => superscripted(
            config,
            icon,
            ordinal.to_string(),
            ReadingStateTint {
                state: ReadingDisplayState::Off,
                opacity,
            },
        ),
        IdentityOrdinal::Unnumbered => icon,
    })
}

#[must_use]
pub fn reading_value<'a, Message: 'a>(
    config: &Config,
    face: &RowFace,
    readout: &RowReadout,
    opacity: f32,
) -> Option<Element<'a, Message>> {
    preferred_value(
        config,
        face,
        readout,
        face.tint(opacity),
        ReadoutPreference::GaugeFirst,
    )
}

fn preferred_value<'a, Message: 'a>(
    config: &Config,
    face: &RowFace,
    readout: &RowReadout,
    tint: ReadingStateTint,
    preference: ReadoutPreference,
) -> Option<Element<'a, Message>> {
    if face.icon.display_state() == ReadingDisplayState::Failed {
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

fn label_text<'a, Message: 'a>(
    config: &Config,
    face: &RowFace,
    label: &ReadoutLabel,
    tint: ReadingStateTint,
) -> Option<Element<'a, Message>> {
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

fn failure_marker<'a, Message: 'a>(
    config: &Config,
    tint: ReadingStateTint,
) -> Element<'a, Message> {
    IconGlyph::from(Phosphor::Warning(Weight::Thin))
        .view(config.theme.font.icon_size_px, move |theme| {
            tint.color(theme)
        })
}

fn badged<'a, Message: 'a>(
    config: &Config,
    reading: Element<'a, Message>,
    badge: ReadoutBadge,
    tint: ReadingStateTint,
) -> Element<'a, Message> {
    match badge {
        ReadoutBadge::Counted(count) => superscripted(config, reading, count.to_string(), tint),
        ReadoutBadge::Unbadged => reading,
    }
}

fn superscripted<'a, Message: 'a>(
    config: &Config,
    marked: Element<'a, Message>,
    digits: String,
    tint: ReadingStateTint,
) -> Element<'a, Message> {
    let superscript = text(digits)
        .size(config.theme.font.small_text_size_px)
        .font(font_faces_of(&config.theme.font).regular)
        .style(tint.text_style());
    Row::with_children([marked, superscript.into()])
        .align_y(Alignment::Start)
        .into()
}
