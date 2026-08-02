use std::{path::PathBuf, time::SystemTime};

use iced::{
    Alignment, Border, Color, Element, Font, Length,
    font::Weight,
    widget::{Column, Row, Space, button, container, image, mouse_area, rich_text, svg},
};

use super::{Notification, NotificationMessage, markup};
use crate::{
    config::{
        disk::NotificationConfig,
        resolved::{ResolvedConfig, ResolvedFontConfig, ResolvedPaletteConfig},
    },
    data::{
        notifications::model::{NotifAction, NotifImage, NotificationData, Urgency},
        source::app_icon::DesktopEntries,
    },
    wayland_layers::tooltip::relative,
    widgets::text::{TextStyle, bar_text},
};

const PADDING_PX: f32 = 10.0;
const SUBTEXT_SCALE: f32 = 0.85;
const COLUMN_SPACING: f32 = 4.0;
const BODY_ROW_SPACING: f32 = 10.0;
const ROW_SPACING: f32 = 8.0;
const BUTTON_PADDING: [f32; 2] = [6.0, 14.0];
const WIDEST_RELTIME: &str = "59 min ago";

#[derive(Debug, Clone)]
pub enum CardIcon {
    Raster(image::Handle),
    Svg(svg::Handle),
}

#[derive(Debug, Clone, Default)]
pub struct CardIcons {
    pub image: Option<CardIcon>,
    pub badge: Option<CardIcon>,
}

pub fn resolve_icons(data: &NotificationData, entries: Option<&DesktopEntries>) -> CardIcons {
    let image = match &data.image {
        Some(NotifImage::Rgba(handle)) => Some(CardIcon::Raster(handle.clone())),
        Some(NotifImage::Named(name)) => resolve_name(name, entries),
        None => None,
    };
    let badge = non_empty(&data.app_icon)
        .or(data.desktop_entry.as_deref())
        .and_then(|name| resolve_name(name, entries));
    CardIcons { image, badge }
}

fn non_empty(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

fn resolve_name(name: &str, entries: Option<&DesktopEntries>) -> Option<CardIcon> {
    let path = if name.starts_with('/') {
        PathBuf::from(name)
    } else if let Some(rest) = name.strip_prefix("file://") {
        PathBuf::from(rest)
    } else {
        let entries = entries?;
        entries
            .theme_icon(name)
            .or_else(|| entries.icon_for(name))?
    };
    let is_svg = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg") || ext.eq_ignore_ascii_case("svgz"));
    Some(if is_svg {
        CardIcon::Svg(svg::Handle::from_path(path))
    } else {
        CardIcon::Raster(image::Handle::from_path(path))
    })
}

fn over(fg: Color, bg: Color) -> Color {
    Color {
        r: bg.r * (1.0 - fg.a) + fg.r * fg.a,
        g: bg.g * (1.0 - fg.a) + fg.g * fg.a,
        b: bg.b * (1.0 - fg.a) + fg.b * fg.a,
        a: 1.0,
    }
}

pub fn measure_height(data: &NotificationData, icons: &CardIcons, config: &ResolvedConfig) -> f32 {
    let font = &config.font;
    let notifications = &config.notifications;
    let base = TextStyle::of(font);
    let subtext = base.scaled(SUBTEXT_SCALE);

    let icon_extent = match (&icons.image, &icons.badge) {
        (Some(_), _) => Some(notifications.image_size),
        (None, Some(_)) => Some(notifications.app_icon_size),
        (None, None) => None,
    };
    let icon_column = icon_extent.map_or(0.0, |size| size + BODY_ROW_SPACING);
    let column_width = notifications.card_width - 2.0 * PADDING_PX - icon_column;

    let has_actions = data
        .actions
        .iter()
        .any(|a| a.key != "default" && !a.label.trim().is_empty());
    let has_body = !data.body.is_empty();
    let reltime_width = subtext.measure(WIDEST_RELTIME).width + ROW_SPACING;

    let app_name_beside_title = if data.app_name.is_empty() {
        0.0
    } else {
        subtext.measure(&data.app_name).width + ROW_SPACING
    };
    let time_beside_title = if !has_actions && !has_body {
        reltime_width
    } else {
        0.0
    };
    let title = base
        .bold()
        .measure_wrapped(
            &data.summary,
            column_width - app_name_beside_title - time_beside_title,
        )
        .height;

    let body = if has_body {
        let time_beside_body = if has_actions { 0.0 } else { reltime_width };
        let spans = markup::body_spans(
            &data.body,
            config.palette.inactive,
            subtext.size(),
            font,
            Font::MONOSPACE,
        );
        subtext
            .measure_spans(&spans, column_width - time_beside_body)
            .height
    } else {
        0.0
    };

    let actions = if has_actions {
        base.line_height() + 2.0 * BUTTON_PADDING[0]
    } else {
        0.0
    };

    let rows = [title, body, actions];
    let present = rows.iter().filter(|height| **height > 0.0).count();
    let gaps = COLUMN_SPACING * present.saturating_sub(1) as f32;
    let column = title + body + actions + gaps;

    column.max(icon_extent.unwrap_or(0.0)) + 2.0 * PADDING_PX
}

pub fn card<'a>(
    notif: &Notification,
    hovered: bool,
    config: &ResolvedConfig,
) -> Element<'a, NotificationMessage> {
    let id = notif.data.id;
    let mut body_row = Row::new()
        .spacing(BODY_ROW_SPACING)
        .align_y(Alignment::Center);
    if let Some(icon) = icon_slot(&notif.icons, &config.notifications) {
        body_row = body_row.push(icon);
    }
    body_row = body_row.push(card_column(notif, config));

    let chrome = card_chrome(hovered, notif.data.urgency == Urgency::Critical, config);
    let panel = container(body_row)
        .width(Length::Fixed(config.notifications.card_width))
        .padding(PADDING_PX)
        .style(move |_| container::Style {
            background: Some(chrome.background.into()),
            border: chrome.border,
            text_color: Some(chrome.foreground),
            ..Default::default()
        });

    mouse_area(panel)
        .on_enter(NotificationMessage::HoverChanged { id, hovered: true })
        .on_exit(NotificationMessage::HoverChanged { id, hovered: false })
        .on_press(NotificationMessage::BodyClicked(id))
        .on_right_press(NotificationMessage::Dismiss(id))
        .into()
}

#[derive(Clone, Copy)]
struct CardChrome {
    background: Color,
    border: Border,
    foreground: Color,
}

fn card_chrome(hovered: bool, critical: bool, config: &ResolvedConfig) -> CardChrome {
    let palette = &config.palette;
    let fill = Color {
        a: 1.0,
        ..palette.bar_background
    };
    let rest = if critical {
        palette.urgent
    } else {
        over(
            Color {
                a: 0.25,
                ..palette.bar_foreground
            },
            fill,
        )
    };
    let accent = if critical {
        palette.urgent
    } else {
        palette.focus
    };
    CardChrome {
        background: fill,
        border: Border {
            color: if hovered { accent } else { rest },
            width: if hovered { 3.0 } else { 2.0 },
            radius: f32::from(config.layout.corner_radius_px).into(),
        },
        foreground: palette.bar_foreground,
    }
}

fn card_column<'a>(
    notif: &Notification,
    config: &ResolvedConfig,
) -> Column<'a, NotificationMessage> {
    let data = &notif.data;
    let id = data.id;
    let font = &config.font;
    let palette = &config.palette;

    let foreground = palette.bar_foreground;
    let dim = palette.dim;
    let body_color = palette.inactive;

    let button_actions: Vec<&NotifAction> = data
        .actions
        .iter()
        .filter(|a| a.key != "default" && !a.label.trim().is_empty())
        .collect();
    let has_actions = !button_actions.is_empty();
    let has_body = !data.body.is_empty();
    let reltime = relative(notif.created_at, SystemTime::now());
    let reltime_span = |visible: bool| {
        visible.then(|| {
            bar_text(font, reltime.clone())
                .size(font.text_size_px * SUBTEXT_SCALE)
                .color(dim)
        })
    };

    let mut title = Row::new()
        .spacing(ROW_SPACING)
        .align_y(Alignment::Center)
        .push(
            bar_text(font, data.summary.clone())
                .color(foreground)
                .font(Font {
                    weight: Weight::Bold,
                    ..font.iced_font
                })
                .width(Length::Fill),
        );
    if !data.app_name.is_empty() {
        title = title.push(
            bar_text(font, data.app_name.clone())
                .size(font.text_size_px * SUBTEXT_SCALE)
                .color(dim),
        );
    }
    if let Some(time) = reltime_span(!has_actions && !has_body) {
        title = title.push(time);
    }

    let mut column = Column::new().spacing(COLUMN_SPACING).push(title);
    if has_body {
        let body = Row::new()
            .spacing(ROW_SPACING)
            .align_y(Alignment::End)
            .push(
                rich_text(markup::body_spans(
                    &data.body,
                    body_color,
                    font.text_size_px * SUBTEXT_SCALE,
                    font,
                    Font::MONOSPACE,
                ))
                .width(Length::Fill),
            );
        column = column.push(match reltime_span(!has_actions) {
            Some(time) => body.push(time),
            None => body,
        });
    }
    if has_actions {
        let mut actions = Row::new().spacing(6.0).align_y(Alignment::Center);
        for action in &button_actions {
            actions = actions.push(action_button(
                action.label.clone(),
                NotificationMessage::ActionClicked {
                    id,
                    key: action.key.clone(),
                },
                action_tint(&action.key, palette),
                font,
            ));
        }
        actions = actions.push(Space::new().width(Length::Fill));
        if let Some(time) = reltime_span(true) {
            actions = actions.push(time);
        }
        column = column.push(actions);
    }
    column
}

fn icon_slot<'a>(
    icons: &CardIcons,
    config: &NotificationConfig,
) -> Option<Element<'a, NotificationMessage>> {
    match (&icons.image, &icons.badge) {
        (Some(image), badge) => {
            let main_size = config.image_size;
            let main = icon_widget(image, main_size);
            Some(match badge {
                Some(badge) => {
                    let overlay = container(icon_widget(badge, config.badge_size))
                        .width(main_size)
                        .height(main_size)
                        .align_x(Alignment::Start)
                        .align_y(Alignment::Start);
                    iced::widget::stack![main, overlay].into()
                }
                None => main,
            })
        }
        (None, Some(badge)) => Some(icon_widget(badge, config.app_icon_size)),
        (None, None) => None,
    }
}

fn icon_widget<'a>(icon: &CardIcon, size: f32) -> Element<'a, NotificationMessage> {
    match icon {
        CardIcon::Raster(handle) => image(handle.clone()).width(size).height(size).into(),
        CardIcon::Svg(handle) => svg(handle.clone()).width(size).height(size).into(),
    }
}

fn action_tint(key: &str, palette: &ResolvedPaletteConfig) -> Color {
    match key {
        "quit" | "deny" | "end" | "reject" => palette.urgent,
        "stop" | "cancel" | "later" | "snooze" => palette.warning,
        _ => palette.focus,
    }
}

fn action_button<'a>(
    label: String,
    message: NotificationMessage,
    tint: Color,
    font: &ResolvedFontConfig,
) -> Element<'a, NotificationMessage> {
    button(bar_text(font, label).size(font.text_size_px).color(tint))
        .padding(BUTTON_PADDING)
        .style(move |_, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(
                    Color {
                        a: if hovered { 0.22 } else { 0.12 },
                        ..tint
                    }
                    .into(),
                ),
                text_color: tint,
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .on_press(message)
        .into()
}
