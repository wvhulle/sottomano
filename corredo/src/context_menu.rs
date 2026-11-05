use iced::{
    Background, Border, Color, Element, Length, Padding, Theme,
    alignment::Vertical,
    border::Radius,
    widget::{Row, button, container, space, text},
};

use crate::{
    activity_line::activity_line,
    font_faces::TextFace,
    list_end_corners::ListEndCorners,
    shared_metrics::OUTLINE_WIDTH_PX,
    theme_colors::{
        background_strong_color, background_weak_color, secondary_text_color, text_color,
    },
    uniform_width_column::uniform_width_column,
};

pub enum Trailing {
    Nothing,
    Note(String),
    Submenu { open: bool },
}

pub enum ContextMenuRowPress<M> {
    Enabled(M),
    Disabled,
}

pub struct ContextMenuRow<'a, M> {
    pub leading: Option<Element<'a, M>>,
    pub label: Option<String>,
    pub trailing: Trailing,
    pub depth: usize,
    pub press: ContextMenuRowPress<M>,
}

pub enum ContextMenuLine<'a, M> {
    Row(ContextMenuRow<'a, M>),
    Divider,
}

pub struct ContextMenuStyle<'style, 'a, M> {
    pub face: TextFace,
    pub note_size_px: f32,
    pub icon_size_px: f32,
    pub item_gap_px: f32,
    pub surface_inset_px: f32,
    pub submenu_chevron: &'style dyn Fn(bool, fn(&Theme) -> Color) -> Element<'a, M>,
}

const CONTEXT_MENU_MAX_WIDTH_PX: f32 = 400.0;

pub fn context_menu<'a, M: Clone + 'a>(
    style: &ContextMenuStyle<'_, 'a, M>,
    popup_corners: Radius,
    lines: impl IntoIterator<Item = ContextMenuLine<'a, M>>,
) -> Element<'a, M> {
    let kept_lines = without_redundant_dividers(lines.into_iter().collect());
    let end_corners = ListEndCorners::new(kept_lines.len(), popup_corners);
    container(
        uniform_width_column(kept_lines.into_iter().enumerate().map(
            |(position, line)| match line {
                ContextMenuLine::Row(row) => entry(style, row, end_corners.radius_at(position)),
                ContextMenuLine::Divider => divider(),
            },
        ))
        .spacing(style.item_gap_px),
    )
    .max_width(CONTEXT_MENU_MAX_WIDTH_PX)
    .into()
}

fn entry<'a, M: Clone + 'a>(
    style: &ContextMenuStyle<'_, 'a, M>,
    row: ContextMenuRow<'a, M>,
    highlight_radius: Radius,
) -> Element<'a, M> {
    let icon_size = style.icon_size_px;
    let row_spacing = style.item_gap_px;
    let color: fn(&Theme) -> Color = match row.press {
        ContextMenuRowPress::Enabled(_) => text_color,
        ContextMenuRowPress::Disabled => secondary_text_color,
    };

    let mut content = Row::new()
        .spacing(row_spacing)
        .align_y(Vertical::Center)
        .width(Length::Fill);
    if let Some(element) = row.leading {
        content = content.push(element);
    }
    if let Some(label) = row.label {
        content = content.push(style.face.text(label).style(move |theme| text::Style {
            color: Some(color(theme)),
        }));
    }
    match row.trailing {
        Trailing::Nothing => {}
        Trailing::Note(note) => {
            content = content.push(space::horizontal()).push(
                style
                    .face
                    .text(note)
                    .size(style.note_size_px)
                    .style(|theme| text::Style {
                        color: Some(background_strong_color(theme)),
                    }),
            );
        }
        Trailing::Submenu { open } => {
            content = content
                .push(space::horizontal())
                .push((style.submenu_chevron)(open, color));
        }
    }

    let glyph_column = icon_size + row_spacing;
    let inset = style.surface_inset_px;
    let padding = Padding {
        top: row_spacing,
        right: inset,
        bottom: row_spacing,
        left: inset + row.depth as f32 * glyph_column,
    };
    let styled = button(content)
        .width(Length::Fill)
        .padding(padding)
        .style(move |theme, status| button_style(theme, status, color, highlight_radius));
    match row.press {
        ContextMenuRowPress::Enabled(message) => styled.on_press(message).into(),
        ContextMenuRowPress::Disabled => styled.into(),
    }
}

fn button_style(
    theme: &Theme,
    status: button::Status,
    text_role: fn(&Theme) -> Color,
    highlight_radius: Radius,
) -> button::Style {
    let background = matches!(status, button::Status::Hovered | button::Status::Pressed)
        .then(|| Background::Color(background_weak_color(theme)));
    button::Style {
        background,
        text_color: text_role(theme),
        border: Border {
            radius: highlight_radius,
            ..Border::default()
        },
        ..Default::default()
    }
}

fn divider<'a, Message: 'a>() -> Element<'a, Message> {
    activity_line(background_weak_color, 1.0, OUTLINE_WIDTH_PX)
}

fn without_redundant_dividers<M>(
    lines: Vec<ContextMenuLine<'_, M>>,
) -> Vec<ContextMenuLine<'_, M>> {
    let mut kept = lines.into_iter().fold(Vec::new(), |mut kept, line| {
        let leading_divider = matches!(line, ContextMenuLine::Divider)
            && kept
                .last()
                .is_none_or(|last| matches!(last, ContextMenuLine::Divider));
        if !leading_divider {
            kept.push(line);
        }
        kept
    });
    if kept
        .last()
        .is_some_and(|last| matches!(last, ContextMenuLine::Divider))
    {
        kept.pop();
    }
    kept
}
