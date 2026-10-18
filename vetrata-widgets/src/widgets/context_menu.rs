use iced::{
    Background, Color, Element, Length, Padding,
    alignment::Vertical,
    widget::{Column, Row, Space, button},
};
use vetrata_config::Config;

use crate::widgets::{
    DISABLED_ALPHA,
    activity_line::activity_line,
    bar_widget::Glyph,
    phosphor::{Phosphor, Weight},
    text::bar_text,
};

const DIVIDER_THICKNESS: f32 = 1.0;
const HOVER_ALPHA: f32 = 0.22;

pub enum Trailing {
    Nothing,
    Note(String),
    Submenu { open: bool },
}

pub struct MenuRow<'a, M> {
    pub leading: Option<Element<'a, M>>,
    pub label: Option<String>,
    pub trailing: Trailing,
    pub enabled: bool,
    pub depth: usize,
    pub on_press: Option<M>,
}

pub enum MenuLine<'a, M> {
    Row(MenuRow<'a, M>),
    Divider,
}

pub fn context_menu<'a, M: Clone + 'a>(
    config: &Config,
    lines: impl IntoIterator<Item = MenuLine<'a, M>>,
) -> Element<'a, M> {
    Column::with_children(
        without_redundant_dividers(lines.into_iter().collect())
            .into_iter()
            .map(|line| match line {
                MenuLine::Row(row) => entry(config, row),
                MenuLine::Divider => divider(config),
            }),
    )
    .spacing(config.layout.overlay.row_spacing)
    .width(Length::Fill)
    .into()
}

fn entry<'a, M: Clone + 'a>(config: &Config, row: MenuRow<'a, M>) -> Element<'a, M> {
    let icon_size = config.theme.font.icon_size_px;
    let row_spacing = config.layout.overlay.row_spacing;
    let color = config
        .theme
        .palette
        .bar_foreground
        .scale_alpha(if row.enabled { 1.0 } else { DISABLED_ALPHA });

    let mut content = Row::new()
        .spacing(row_spacing)
        .align_y(Vertical::Center)
        .width(Length::Fill);
    if let Some(element) = row.leading {
        content = content.push(element);
    }
    if let Some(label) = row.label {
        content = content.push(bar_text(&config.theme.font, label).color(color));
    }
    match row.trailing {
        Trailing::Nothing => {}
        Trailing::Note(note) => {
            content = content.push(Space::new().width(Length::Fill)).push(
                bar_text(&config.theme.font, note)
                    .size(config.theme.font.small_text_size_px)
                    .color(config.theme.palette.dim),
            );
        }
        Trailing::Submenu { open } => {
            let chevron = if open {
                Phosphor::CaretDown(Weight::Regular)
            } else {
                Phosphor::CaretRight(Weight::Regular)
            };
            content = content
                .push(Space::new().width(Length::Fill))
                .push(Glyph::from(chevron).view(icon_size, color));
        }
    }

    let glyph_column = icon_size + row_spacing;
    let inset = config.layout.overlay.content_inset;
    let padding = Padding {
        top: row_spacing,
        right: inset,
        bottom: row_spacing,
        left: inset + row.depth as f32 * glyph_column,
    };
    let accent = config.theme.palette.focus;
    let styled = button(content)
        .width(Length::Fill)
        .padding(padding)
        .style(move |_theme, status| button_style(status, color, accent));
    match row.on_press.filter(|_| row.enabled) {
        Some(message) => styled.on_press(message).into(),
        None => styled.into(),
    }
}

fn button_style(status: button::Status, text: Color, accent: Color) -> button::Style {
    let background = matches!(status, button::Status::Hovered | button::Status::Pressed)
        .then(|| Background::Color(accent.scale_alpha(HOVER_ALPHA)));
    button::Style {
        background,
        text_color: text,
        ..Default::default()
    }
}

fn divider<'a, Message: 'a>(config: &Config) -> Element<'a, Message> {
    activity_line(config.theme.palette.surface_raised, 1.0, DIVIDER_THICKNESS)
}

fn without_redundant_dividers<M>(lines: Vec<MenuLine<'_, M>>) -> Vec<MenuLine<'_, M>> {
    let mut kept = lines.into_iter().fold(Vec::new(), |mut kept, line| {
        let leading_divider = matches!(line, MenuLine::Divider)
            && kept
                .last()
                .is_none_or(|last| matches!(last, MenuLine::Divider));
        if !leading_divider {
            kept.push(line);
        }
        kept
    });
    if kept
        .last()
        .is_some_and(|last| matches!(last, MenuLine::Divider))
    {
        kept.pop();
    }
    kept
}
