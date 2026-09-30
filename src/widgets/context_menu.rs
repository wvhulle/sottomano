use std::collections::HashSet;

use iced::{
    Background, Border, Color, Element, Length, Padding,
    alignment::Vertical,
    border::Radius,
    widget::{Column, Row, Space, button, container},
};

use crate::{
    config::Config,
    data::source::tray::{MenuKind, MenuNode},
    widgets::{
        DISABLED_ALPHA,
        activity_line::activity_line,
        bar_widget::Glyph,
        native_icon::native_icon,
        phosphor::{Phosphor, Weight},
        text::bar_text,
    },
};

const ROW_SPACING: f32 = 1.0;
const DIVIDER_THICKNESS: f32 = 1.0;
const DIVIDER_PADDING: [f32; 2] = [4.0, 0.0];
const ROW_PADDING: [f32; 2] = [5.0, 8.0];
const SUBMENU_INDENT: f32 = 16.0;
const HOVER_ALPHA: f32 = 0.22;
const CHEVRON_SCALE: f32 = 0.85;

pub fn context_menu<'a, Message: Clone + 'a>(
    config: &Config,
    corner_radius: Radius,
    nodes: &[MenuNode],
    open: &HashSet<i32>,
    on_activate: &impl Fn(i32) -> Message,
    on_toggle: &impl Fn(i32) -> Message,
) -> Element<'a, Message> {
    let rows = clean_separators(nodes)
        .into_iter()
        .flat_map(|node| entry_rows(config, corner_radius, node, open, on_activate, on_toggle));
    Column::with_children(rows)
        .spacing(ROW_SPACING)
        .width(Length::Fill)
        .into()
}

fn entry_rows<'a, Message: Clone + 'a>(
    config: &Config,
    corner_radius: Radius,
    node: &MenuNode,
    open: &HashSet<i32>,
    on_activate: &impl Fn(i32) -> Message,
    on_toggle: &impl Fn(i32) -> Message,
) -> Vec<Element<'a, Message>> {
    match node.kind {
        MenuKind::Separator => vec![divider(config)],
        MenuKind::Submenu if open.contains(&node.id) => vec![
            entry(config, corner_radius, node, true, on_toggle(node.id)),
            container(context_menu(
                config,
                corner_radius,
                &node.children,
                open,
                on_activate,
                on_toggle,
            ))
            .padding(Padding::ZERO.left(SUBMENU_INDENT))
            .into(),
        ],
        MenuKind::Submenu => vec![entry(
            config,
            corner_radius,
            node,
            false,
            on_toggle(node.id),
        )],
        MenuKind::Standard | MenuKind::Toggle(_) => {
            vec![entry(
                config,
                corner_radius,
                node,
                false,
                on_activate(node.id),
            )]
        }
    }
}

fn entry<'a, Message: Clone + 'a>(
    config: &Config,
    corner_radius: Radius,
    node: &MenuNode,
    submenu_open: bool,
    message: Message,
) -> Element<'a, Message> {
    let icon_size = config.font.icon_size_px;
    let color =
        config
            .palette
            .bar_foreground
            .scale_alpha(if node.enabled { 1.0 } else { DISABLED_ALPHA });

    let leading: Option<Element<'a, Message>> = node
        .icon
        .as_ref()
        .map(|source| native_icon(source, icon_size, 1.0))
        .or_else(|| {
            matches!(node.kind, MenuKind::Toggle(true))
                .then(|| Glyph::from(Phosphor::Check(Weight::Regular)).view(icon_size, color))
        });

    let mut row = Row::new()
        .spacing(config.spacing.widget_spacing_px)
        .align_y(Vertical::Center)
        .width(Length::Fill);
    if let Some(element) = leading {
        row = row.push(element);
    }
    row = row.push(bar_text(&config.font, node.label.clone()).color(color));
    if matches!(node.kind, MenuKind::Submenu) {
        let chevron = if submenu_open {
            Phosphor::CaretDown(Weight::Regular)
        } else {
            Phosphor::CaretRight(Weight::Regular)
        };
        row = row.push(Space::new().width(Length::Fill)).push(
            Glyph::from(chevron.with_weight(Weight::Regular))
                .view(icon_size * CHEVRON_SCALE, color),
        );
    }

    let accent = config.palette.focus;
    let styled = button(row)
        .width(Length::Fill)
        .padding(ROW_PADDING)
        .style(move |_theme, status| button_style(status, color, accent, corner_radius));
    if node.enabled {
        styled.on_press(message).into()
    } else {
        styled.into()
    }
}

fn button_style(
    status: button::Status,
    text: Color,
    accent: Color,
    radius: Radius,
) -> button::Style {
    let background = matches!(status, button::Status::Hovered | button::Status::Pressed)
        .then(|| Background::Color(accent.scale_alpha(HOVER_ALPHA)));
    button::Style {
        background,
        text_color: text,
        border: Border {
            radius,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn divider<'a, Message: 'a>(config: &Config) -> Element<'a, Message> {
    let color = config.palette.surface_raised;
    container(activity_line(color, 1.0, DIVIDER_THICKNESS))
        .width(Length::Fill)
        .padding(DIVIDER_PADDING)
        .into()
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
