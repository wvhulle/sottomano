use std::{
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::{DateTime, Locale, TimeDelta, Utc};
use iced::{
    Color, Element, Font, Theme,
    alignment::Vertical,
    font::Style as FontStyle,
    mouse::Interaction,
    widget::{
        Container, Row, Space, container, rich_text, span,
        text::{Span, Style as TextStyle, Wrapping},
    },
};
use sottomano_toml_config::{severity::Severity, theme::font::ResolvedFont};

use crate::widgets::{
    configured_theme::AccentShades,
    icon_glyph::IconGlyph,
    localized_durations::{
        LengthOfTime, duration_text, relative_moment_text, signed_duration_text,
    },
    localized_quantities::Quantity,
    localized_time::LocalizedTimeFormat,
    pressable_text::pressable_text,
    text::{FontFaces, LINE_HEIGHT, panel_text},
    transitioning_text::{TextPresence, TransitionStyle, TransitioningText},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ValueCell {
    Blank,
    Text(String),
    Path(PathBuf),
    IconGlyph(IconGlyph),
    Time {
        at: DateTime<Utc>,
        format: LocalizedTimeFormat,
    },
    Duration(Duration),
    SignedDuration(TimeDelta),
    RelativeMoment(DateTime<Utc>),
    Readout(SliderReadout),
    Quantity(Quantity),
    Link {
        label: String,
        url: String,
    },
    BrandedLink {
        label: String,
        url: String,
        glyph: IconGlyph,
    },
    BrandedText {
        text: String,
        glyph: IconGlyph,
    },
    Hint(String),
}

#[derive(Debug, Clone, Copy)]
pub struct SliderReadout {
    value_bits: u32,
    format: fn(f32, Locale) -> String,
}

impl PartialEq for SliderReadout {
    fn eq(&self, other: &Self) -> bool {
        self.value_bits == other.value_bits
            && (self.format)(f32::from_bits(self.value_bits), Locale::POSIX)
                == (other.format)(f32::from_bits(other.value_bits), Locale::POSIX)
    }
}

impl Eq for SliderReadout {}

impl Hash for SliderReadout {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value_bits.hash(state);
    }
}

impl SliderReadout {
    #[must_use]
    pub fn new(value: f32, format: fn(f32, Locale) -> String) -> Self {
        Self {
            value_bits: value.to_bits(),
            format,
        }
    }

    #[must_use]
    pub fn text(self, locale: Locale) -> String {
        (self.format)(f32::from_bits(self.value_bits), locale)
    }
}

impl ValueCell {
    #[must_use]
    pub fn duration(length: impl LengthOfTime) -> Self {
        ValueCell::Duration(length.elapsed_length())
    }
}

impl From<String> for ValueCell {
    fn from(text: String) -> Self {
        ValueCell::Text(text)
    }
}

impl From<Quantity> for ValueCell {
    fn from(quantity: Quantity) -> Self {
        ValueCell::Quantity(quantity)
    }
}

impl From<&str> for ValueCell {
    fn from(text: &str) -> Self {
        ValueCell::Text(text.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabeledValue {
    key: ValueCell,
    value: ValueCell,
    level: Severity,
}

impl LabeledValue {
    pub fn new(key: impl Into<ValueCell>, value: impl Into<ValueCell>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            level: Severity::Normal,
        }
    }

    #[must_use]
    pub fn level(self, level: Severity) -> Self {
        Self { level, ..self }
    }

    #[must_use]
    pub fn key(&self) -> &ValueCell {
        &self.key
    }

    #[must_use]
    pub fn value(&self) -> &ValueCell {
        &self.value
    }

    #[must_use]
    pub fn severity(&self) -> Severity {
        self.level
    }
}

pub struct ValueCellPressMessages<Message> {
    pub copy_text: fn(String) -> Message,
    pub open_url: fn(String) -> Message,
}

impl<Message> Clone for ValueCellPressMessages<Message> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Message> Copy for ValueCellPressMessages<Message> {}

pub struct ValueCellView<Message> {
    pub figure_font: ResolvedFont,
    pub prose_font: ResolvedFont,
    pub max_width: f32,
    pub glyph_gap: f32,
    pub wrapping: Wrapping,
    pub locale: Locale,
    pub now: DateTime<Utc>,
    pub flash: fn(&Theme) -> AccentShades,
    pub link: fn(&Theme) -> AccentShades,
    pub hint: fn(&Theme) -> Color,
    pub transition: TransitionStyle,
    pub copy_flash: Duration,
    pub press_messages: ValueCellPressMessages<Message>,
    pub font_style: FontStyle,
}

impl<Message> Clone for ValueCellView<Message> {
    fn clone(&self) -> Self {
        Self {
            figure_font: self.figure_font.clone(),
            prose_font: self.prose_font.clone(),
            max_width: self.max_width,
            glyph_gap: self.glyph_gap,
            wrapping: self.wrapping,
            locale: self.locale,
            now: self.now,
            flash: self.flash,
            link: self.link,
            hint: self.hint,
            transition: self.transition,
            copy_flash: self.copy_flash,
            press_messages: self.press_messages,
            font_style: self.font_style,
        }
    }
}

#[derive(Clone, Copy)]
enum CellTypeFace {
    Prose,
    Figures,
}

impl<Message: 'static> ValueCellView<Message> {
    #[must_use]
    pub fn key_view(&self) -> Self {
        Self {
            figure_font: self.prose_font.clone(),
            wrapping: Wrapping::Word,
            font_style: FontStyle::Italic,
            transition: TransitionStyle {
                presence: TextPresence::Shown,
                ..self.transition
            },
            ..self.clone()
        }
    }

    #[must_use]
    pub fn key(
        &self,
        key: ValueCell,
        color: impl Fn(&Theme) -> Color + Copy + 'static,
    ) -> Element<'static, Message> {
        let row = key.clone();
        self.value(key, color, &row)
    }

    #[must_use]
    pub fn value(
        &self,
        value: ValueCell,
        color: impl Fn(&Theme) -> Color + Copy + 'static,
        row: &ValueCell,
    ) -> Element<'static, Message> {
        match value {
            ValueCell::Blank => Space::new().into(),
            ValueCell::IconGlyph(icon) => self.glyph(icon, color),
            ValueCell::Text(text) => self.copyable_text(text, CellTypeFace::Prose, color, row),
            ValueCell::Path(path) => self.path(&path, color, row),
            ValueCell::Time { at, format } => {
                self.copyable_figures(format.format(&at, self.locale), color, row)
            }
            ValueCell::Duration(length) => {
                self.copyable_figures(duration_text(length, self.locale), color, row)
            }
            ValueCell::RelativeMoment(at) => {
                self.copyable_figures(relative_moment_text(at, self.now, self.locale), color, row)
            }
            ValueCell::SignedDuration(offset) => {
                self.copyable_figures(signed_duration_text(offset, self.locale), color, row)
            }
            ValueCell::Readout(readout) => {
                self.copyable_figures(readout.text(self.locale), color, row)
            }
            ValueCell::Quantity(quantity) => {
                self.copyable_figures(quantity.localized_text(self.locale), color, row)
            }
            ValueCell::Link { label, url } => {
                let underlined = self.underlined_label(label, color, row);
                self.openable(underlined, url)
            }
            ValueCell::BrandedLink { label, url, glyph } => {
                let underlined = self.underlined_label(label, color, row);
                let branded = Row::with_children([self.glyph(glyph, color), underlined])
                    .spacing(self.glyph_gap)
                    .align_y(Vertical::Center);
                self.openable(branded.into(), url)
            }
            ValueCell::BrandedText { text, glyph } => {
                let copyable = self.copyable_text(text, CellTypeFace::Prose, color, row);
                Row::with_children([self.glyph(glyph, color), copyable])
                    .spacing(self.glyph_gap)
                    .align_y(Vertical::Center)
                    .into()
            }
            ValueCell::Hint(text) => self.text(text, CellTypeFace::Prose, self.hint, row),
        }
    }

    fn styled_face(&self, font: &ResolvedFont) -> Font {
        Font {
            style: self.font_style,
            ..FontFaces::from(font).regular
        }
    }

    fn face(&self, face: CellTypeFace) -> &ResolvedFont {
        match face {
            CellTypeFace::Prose => &self.prose_font,
            CellTypeFace::Figures => &self.figure_font,
        }
    }

    fn text(
        &self,
        text: String,
        face: CellTypeFace,
        color: impl Fn(&Theme) -> Color + Copy + 'static,
        row: &ValueCell,
    ) -> Element<'static, Message> {
        let font = self.face(face);
        TransitioningText::new(
            container(
                panel_text(font, text.clone())
                    .font(self.styled_face(font))
                    .style(move |theme| TextStyle {
                        color: Some(color(theme)),
                    })
                    .wrapping(self.wrapping),
            )
            .max_width(self.max_width),
            text,
            self.transition,
            font,
            color,
        )
        .identified_by(row)
        .into()
    }

    fn path(
        &self,
        path: &Path,
        color: impl Fn(&Theme) -> Color + Copy + 'static,
        row: &ValueCell,
    ) -> Element<'static, Message> {
        let text = path.display().to_string();
        let unwrapped = panel_text(&self.prose_font, text.clone())
            .font(self.styled_face(&self.prose_font))
            .style(move |theme| TextStyle {
                color: Some(color(theme)),
            })
            .wrapping(Wrapping::None);
        let value = TransitioningText::new(
            unwrapped,
            text.clone(),
            self.transition,
            &self.prose_font,
            color,
        )
        .identified_by(row);
        pressable_text(
            value,
            text,
            self.press_messages.copy_text,
            self.flash,
            self.copy_flash,
            Interaction::Copy,
        )
    }

    fn underlined_label(
        &self,
        label: String,
        color: impl Fn(&Theme) -> Color + Copy + 'static,
        link_cell: &ValueCell,
    ) -> Element<'static, Message> {
        let underlined_label: Span<'static> = span(label.clone())
            .font(self.styled_face(&self.prose_font))
            .underline(true);
        let underlined = container(
            rich_text([underlined_label])
                .style(move |theme| TextStyle {
                    color: Some(color(theme)),
                })
                .size(self.prose_font.text_size_px)
                .line_height(LINE_HEIGHT)
                .wrapping(self.wrapping),
        )
        .max_width(self.max_width);
        TransitioningText::new(underlined, label, self.transition, &self.prose_font, color)
            .identified_by(link_cell)
            .into()
    }

    fn openable(&self, label: Element<'static, Message>, url: String) -> Element<'static, Message> {
        pressable_text(
            label,
            url,
            self.press_messages.open_url,
            self.link,
            self.copy_flash,
            Interaction::Pointer,
        )
    }

    fn glyph(
        &self,
        icon: IconGlyph,
        color: impl Fn(&Theme) -> Color + Copy + 'static,
    ) -> Element<'static, Message> {
        icon.view(self.prose_font.text_size_px, color)
    }

    fn copyable_figures(
        &self,
        text: String,
        color: impl Fn(&Theme) -> Color + Copy + 'static,
        row: &ValueCell,
    ) -> Element<'static, Message> {
        self.copyable_text(text, CellTypeFace::Figures, color, row)
    }

    fn copyable_text(
        &self,
        text: String,
        face: CellTypeFace,
        color: impl Fn(&Theme) -> Color + Copy + 'static,
        row: &ValueCell,
    ) -> Element<'static, Message> {
        let value = self.text(text.clone(), face, color, row);
        if text.trim().is_empty() {
            return value;
        }
        pressable_text(
            value,
            text,
            self.press_messages.copy_text,
            self.flash,
            self.copy_flash,
            Interaction::Copy,
        )
    }
}

#[must_use]
pub fn wrapped_text<Message>(
    font: &ResolvedFont,
    text: String,
    color: impl Fn(&Theme) -> Color + Copy + 'static,
    max_width: f32,
    wrapping: Wrapping,
) -> Container<'static, Message> {
    container(
        panel_text(font, text)
            .style(move |theme| TextStyle {
                color: Some(color(theme)),
            })
            .wrapping(wrapping),
    )
    .max_width(max_width)
}
