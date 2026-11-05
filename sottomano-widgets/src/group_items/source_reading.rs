use iced::{
    Alignment, Element,
    widget::{Row, row, text},
};
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
    group_items::{
        gauge::Gauge,
        group_item_reading::{GroupItemReading, state_weighted_icon},
        reading_display_state::{ReadingDisplayState, ReadingStateTint},
    },
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        steady_label_width::{LabelShape, steady_label_width},
        text::{FontFaces, panel_text},
    },
};

pub fn source_reading<'a, Message: 'a>(
    config: &Config,
    widget: &dyn GroupItemReading,
    opacity: f32,
    clock: Clock,
) -> Element<'a, Message> {
    let tint = reading_tint(widget, opacity, clock);
    composed_reading(
        config,
        widget,
        reading_icon(config, widget, tint, clock),
        tint,
        clock,
        reading_value,
    )
}

pub fn labeled_reading<'a, Message: 'a>(
    config: &Config,
    widget: &dyn GroupItemReading,
    opacity: f32,
    clock: Clock,
) -> Element<'a, Message> {
    let tint = reading_tint(widget, opacity, clock);
    composed_reading(
        config,
        widget,
        reading_icon(config, widget, tint, clock),
        tint,
        clock,
        label_value,
    )
}

fn reading_tint(widget: &dyn GroupItemReading, opacity: f32, clock: Clock) -> ReadingStateTint {
    ReadingStateTint {
        state: widget.state(clock),
        opacity,
    }
}

fn composed_reading<'a, Message: 'a>(
    config: &Config,
    widget: &dyn GroupItemReading,
    icon: Element<'a, Message>,
    tint: ReadingStateTint,
    clock: Clock,
    value: fn(
        &Config,
        &dyn GroupItemReading,
        ReadingStateTint,
        Clock,
    ) -> Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let reading = match value(config, widget, tint, clock) {
        Some(value) => row![icon, value]
            .spacing(config.theme.spacing.item_gap)
            .align_y(Alignment::Center)
            .into(),
        None => icon,
    };

    if widget.state(clock) == ReadingDisplayState::Failed {
        return reading;
    }
    badged(config, reading, widget.badge(), tint)
}

pub fn reading_icon<'a, Message: 'a>(
    config: &Config,
    widget: &dyn GroupItemReading,
    tint: ReadingStateTint,
    clock: Clock,
) -> Element<'a, Message> {
    let icon = state_weighted_icon(widget, clock)
        .view(config.theme.font.icon_size_px, move |theme| {
            tint.color(theme)
        });
    with_identity_ordinal(config, widget, icon, tint)
}

fn with_identity_ordinal<'a, Message: 'a>(
    config: &Config,
    widget: &dyn GroupItemReading,
    icon: Element<'a, Message>,
    tint: ReadingStateTint,
) -> Element<'a, Message> {
    match widget.identity_ordinal() {
        Some(ordinal) => superscripted(
            config,
            icon,
            ordinal.to_string(),
            ReadingStateTint {
                state: ReadingDisplayState::Off,
                opacity: tint.opacity,
            },
        ),
        None => icon,
    }
}

#[must_use]
pub fn reading_value<'a, Message: 'a>(
    config: &Config,
    widget: &dyn GroupItemReading,
    tint: ReadingStateTint,
    clock: Clock,
) -> Option<Element<'a, Message>> {
    if widget.state(clock) == ReadingDisplayState::Failed {
        return Some(failure_marker(config, tint));
    }
    match widget.gauge() {
        Some(fill) => Some(
            Gauge::new(fill, tint, config.theme.font.icon_size_px)
                .live(widget.live_gauge().unwrap_or(fill))
                .into(),
        ),
        None => label_text(config, widget, tint, clock),
    }
}

fn label_value<'a, Message: 'a>(
    config: &Config,
    widget: &dyn GroupItemReading,
    tint: ReadingStateTint,
    clock: Clock,
) -> Option<Element<'a, Message>> {
    if widget.state(clock) == ReadingDisplayState::Failed {
        return Some(failure_marker(config, tint));
    }
    label_text(config, widget, tint, clock)
}

fn label_text<'a, Message: 'a>(
    config: &Config,
    widget: &dyn GroupItemReading,
    tint: ReadingStateTint,
    clock: Clock,
) -> Option<Element<'a, Message>> {
    widget
        .label(clock, config.calendar.locale)
        .filter(|label| !label.is_empty())
        .map(|label| {
            let shape = LabelShape {
                widget_name: widget.name(),
                character_count: label.chars().count(),
            };
            steady_label_width(
                shape,
                panel_text(&config.theme.font, label)
                    .style(tint.text_style())
                    .into(),
            )
        })
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
    badge: Option<u32>,
    tint: ReadingStateTint,
) -> Element<'a, Message> {
    match badge {
        Some(count) if count > 0 => superscripted(config, reading, count.to_string(), tint),
        _ => reading,
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
        .font(FontFaces::from(&config.theme.font).regular)
        .style(tint.text_style());
    Row::with_children([marked, superscript.into()])
        .align_y(Alignment::Start)
        .into()
}
