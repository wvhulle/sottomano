//! A reusable context-menu popup body (odin's dbusmenu popup, but domain-
//! agnostic): a vertical list of entries — rows that highlight on hover,
//! section dividers, in-place submenus, and check toggles. The caller supplies
//! the tree and maps a node id to its own message; the set of expanded submenus
//! lives with the caller. Knows nothing about D-Bus, so any future menu can
//! reuse it.

use std::collections::HashSet;

use iced::{
    Background, Border, Color, Element, Length, Padding,
    alignment::Vertical,
    widget::{Column, Row, Space, button, container},
};

use crate::{
    config::resolved::ResolvedConfig,
    data::source::font_awesome::Solid,
    widgets::{
        native_icon::{IconSource, native_icon},
        text::bar_text,
    },
};

const ROW_SPACING: f32 = 1.0;
const ROW_RADIUS: f32 = 5.0;
const ROW_PADDING: [f32; 2] = [5.0, 8.0];
/// Left indent applied to a revealed submenu's rows.
const SUBMENU_INDENT: f32 = 16.0;
const DIVIDER_ALPHA: f32 = 0.3;
const HOVER_ALPHA: f32 = 0.22;
/// Submenu chevrons render a touch smaller than the text icons.
const CHEVRON_SCALE: f32 = 0.85;

/// One entry in the menu tree. `id` is an opaque handle the caller maps to a
/// message; `icon` is the entry's own icon, if any.
#[derive(Debug, Clone)]
pub struct MenuNode {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub kind: MenuKind,
    pub icon: Option<IconSource>,
    pub children: Vec<MenuNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKind {
    Standard,
    Separator,
    Submenu,
    /// A checkbox/radio entry and whether it is currently on.
    Toggle(bool),
}

/// Render the menu as a `Column`. `open` holds the ids of expanded submenus;
/// `paint` fades the whole body (0 while a popup surface settles).
/// `on_activate` maps a clicked standard/toggle entry's id to a message;
/// `on_toggle` maps a submenu header's id.
pub fn context_menu<'a, Message: Clone + 'a>(
    config: &ResolvedConfig,
    nodes: &[MenuNode],
    open: &HashSet<i32>,
    paint: f32,
    on_activate: &impl Fn(i32) -> Message,
    on_toggle: &impl Fn(i32) -> Message,
) -> Element<'a, Message> {
    let rows = clean_separators(nodes)
        .into_iter()
        .flat_map(|node| entry_rows(config, node, open, paint, on_activate, on_toggle));
    Column::with_children(rows)
        .spacing(ROW_SPACING)
        .width(Length::Fill)
        .into()
}

/// The element(s) a node contributes: a divider, a plain entry, or — for an
/// open submenu — the header followed by its indented children.
fn entry_rows<'a, Message: Clone + 'a>(
    config: &ResolvedConfig,
    node: &MenuNode,
    open: &HashSet<i32>,
    paint: f32,
    on_activate: &impl Fn(i32) -> Message,
    on_toggle: &impl Fn(i32) -> Message,
) -> Vec<Element<'a, Message>> {
    match node.kind {
        MenuKind::Separator => vec![divider(config, paint)],
        MenuKind::Submenu if open.contains(&node.id) => vec![
            entry(config, node, true, paint, on_toggle(node.id)),
            container(context_menu(
                config,
                &node.children,
                open,
                paint,
                on_activate,
                on_toggle,
            ))
            .padding(Padding::ZERO.left(SUBMENU_INDENT))
            .into(),
        ],
        MenuKind::Submenu => vec![entry(config, node, false, paint, on_toggle(node.id))],
        MenuKind::Standard | MenuKind::Toggle(_) => {
            vec![entry(config, node, false, paint, on_activate(node.id))]
        }
    }
}

/// One clickable row: a leading icon (the entry's own, else a check for a
/// toggled item), the label, and — for a submenu — a right-aligned chevron.
/// Rendered as a `button` so it highlights on hover; a disabled entry is dimmed
/// and inert.
fn entry<'a, Message: Clone + 'a>(
    config: &ResolvedConfig,
    node: &MenuNode,
    submenu_open: bool,
    paint: f32,
    message: Message,
) -> Element<'a, Message> {
    let icon_size = config.font.icon_size_px;
    let color = config
        .palette
        .bar_foreground
        .scale_alpha(paint * if node.enabled { 1.0 } else { 0.4 });

    let leading: Option<Element<'a, Message>> = node
        .icon
        .as_ref()
        .map(|source| native_icon(source, icon_size, 1.0))
        .or_else(|| {
            matches!(node.kind, MenuKind::Toggle(true))
                .then(|| Solid::Check.widget().size(icon_size).color(color).into())
        });

    let mut row = Row::new()
        .spacing(config.layout.horizontal_widget_spacing_px)
        .align_y(Vertical::Center)
        .width(Length::Fill);
    if let Some(element) = leading {
        row = row.push(element);
    }
    row = row.push(bar_text(&config.font, node.label.clone()).color(color));
    if matches!(node.kind, MenuKind::Submenu) {
        let chevron = if submenu_open {
            Solid::ChevronDown
        } else {
            Solid::ChevronRight
        };
        row = row.push(Space::new().width(Length::Fill)).push(
            chevron
                .widget()
                .size(icon_size * CHEVRON_SCALE)
                .color(color),
        );
    }

    let accent = config.palette.focus;
    let styled = button(row)
        .width(Length::Fill)
        .padding(ROW_PADDING)
        .style(move |_theme, status| button_style(status, color, accent, paint));
    if node.enabled {
        styled.on_press(message).into()
    } else {
        styled.into()
    }
}

fn button_style(status: button::Status, text: Color, accent: Color, paint: f32) -> button::Style {
    let background = matches!(status, button::Status::Hovered | button::Status::Pressed)
        .then(|| Background::Color(accent.scale_alpha(HOVER_ALPHA * paint)));
    button::Style {
        background,
        text_color: text,
        border: Border {
            radius: ROW_RADIUS.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// A subtle full-width 1px rule between sections, edge to edge, with a little
/// vertical breathing room. Its alpha is set directly on the colour.
fn divider<'a, Message: 'a>(config: &ResolvedConfig, paint: f32) -> Element<'a, Message> {
    let color = config
        .palette
        .bar_foreground
        .scale_alpha(DIVIDER_ALPHA * paint);
    let rule = container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(move |_| container::Style {
            background: Some(Background::Color(color)),
            ..Default::default()
        });
    container(rule)
        .width(Length::Fill)
        .padding([4.0, 0.0])
        .into()
}

/// Drop redundant separators — leading, trailing, and runs of consecutive ones
/// (apps often leave empty sections) — so every rendered rule is a real divide.
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
