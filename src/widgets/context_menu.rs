use std::collections::HashSet;

use iced::{
    Background, Color, Element, Length, Padding,
    alignment::Vertical,
    widget::{Column, Row, Space, button},
};

use crate::{
    config::Config,
    data::source::tray::{MenuKind, MenuNode},
    widgets::{
        DISABLED_ALPHA,
        activity_line::activity_line,
        bar_widget::Glyph,
        icon_handle::IconHandle,
        phosphor::{Phosphor, Weight},
        text::bar_text,
    },
};

const DIVIDER_THICKNESS: f32 = 1.0;
const HOVER_ALPHA: f32 = 0.22;

pub fn context_menu<'a, Message: Clone + 'a>(
    config: &Config,
    nodes: &[MenuNode],
    open: &HashSet<i32>,
    on_activate: &impl Fn(i32) -> Message,
    on_toggle: &impl Fn(i32) -> Message,
) -> Element<'a, Message> {
    Column::with_children(nested_rows(config, 0, nodes, open, on_activate, on_toggle))
        .spacing(config.layout.overlay.row_spacing)
        .width(Length::Fill)
        .into()
}

fn nested_rows<'a, Message: Clone + 'a>(
    config: &Config,
    depth: usize,
    nodes: &[MenuNode],
    open: &HashSet<i32>,
    on_activate: &impl Fn(i32) -> Message,
    on_toggle: &impl Fn(i32) -> Message,
) -> Vec<Element<'a, Message>> {
    clean_separators(nodes)
        .into_iter()
        .flat_map(|node| entry_rows(config, depth, node, open, on_activate, on_toggle))
        .collect()
}

fn entry_rows<'a, Message: Clone + 'a>(
    config: &Config,
    depth: usize,
    node: &MenuNode,
    open: &HashSet<i32>,
    on_activate: &impl Fn(i32) -> Message,
    on_toggle: &impl Fn(i32) -> Message,
) -> Vec<Element<'a, Message>> {
    match node.kind {
        MenuKind::Separator => vec![divider(config)],
        MenuKind::Submenu if open.contains(&node.id) => {
            let mut rows = vec![entry(config, depth, node, true, on_toggle(node.id))];
            rows.extend(nested_rows(
                config,
                depth + 1,
                &node.children,
                open,
                on_activate,
                on_toggle,
            ));
            rows
        }
        MenuKind::Submenu => vec![entry(config, depth, node, false, on_toggle(node.id))],
        MenuKind::Standard | MenuKind::Toggle(_) => {
            vec![entry(config, depth, node, false, on_activate(node.id))]
        }
    }
}

fn entry<'a, Message: Clone + 'a>(
    config: &Config,
    depth: usize,
    node: &MenuNode,
    submenu_open: bool,
    message: Message,
) -> Element<'a, Message> {
    let icon_size = config.theme.font.icon_size_px;
    let row_spacing = config.layout.overlay.row_spacing;
    let color = config
        .theme
        .palette
        .bar_foreground
        .scale_alpha(if node.enabled { 1.0 } else { DISABLED_ALPHA });

    let leading: Option<Element<'a, Message>> = node
        .icon
        .as_ref()
        .map(|source| IconHandle::from(source).view(icon_size, 1.0))
        .or_else(|| {
            matches!(node.kind, MenuKind::Toggle(true))
                .then(|| Glyph::from(Phosphor::Check(Weight::Regular)).view(icon_size, color))
        });

    let mut row = Row::new()
        .spacing(row_spacing)
        .align_y(Vertical::Center)
        .width(Length::Fill);
    if let Some(element) = leading {
        row = row.push(element);
    }
    if let Some(label) = &node.label {
        row = row.push(bar_text(&config.theme.font, label.clone()).color(color));
    }
    if matches!(node.kind, MenuKind::Submenu) {
        let chevron = if submenu_open {
            Phosphor::CaretDown(Weight::Regular)
        } else {
            Phosphor::CaretRight(Weight::Regular)
        };
        row = row
            .push(Space::new().width(Length::Fill))
            .push(Glyph::from(chevron.with_weight(Weight::Regular)).view(icon_size, color));
    }

    let glyph_column = icon_size + row_spacing;
    let inset = config.layout.overlay.content_inset;
    let padding = Padding {
        top: row_spacing,
        right: inset,
        bottom: row_spacing,
        left: inset + depth as f32 * glyph_column,
    };
    let accent = config.theme.palette.focus;
    let styled = button(row)
        .width(Length::Fill)
        .padding(padding)
        .style(move |_theme, status| button_style(status, color, accent));
    if node.enabled {
        styled.on_press(message).into()
    } else {
        styled.into()
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

fn clean_separators(nodes: &[MenuNode]) -> Vec<&MenuNode> {
    let mut out: Vec<&MenuNode> = Vec::new();
    for node in nodes {
        let is_separator = matches!(node.kind, MenuKind::Separator);
        if is_separator
            && out
                .last()
                .is_none_or(|last| matches!(last.kind, MenuKind::Separator))
        {
            continue;
        }
        out.push(node);
    }
    if out
        .last()
        .is_some_and(|last| matches!(last.kind, MenuKind::Separator))
    {
        out.pop();
    }
    out
}
