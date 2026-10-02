use std::{path::PathBuf, time::SystemTime};

use iced::{
    Alignment, Border, Color, Element, Font, Length, Shadow,
    font::Weight,
    theme::palette::{Extended, Pair},
    widget::{Column, Row, Space, button, container, image, mouse_area, rich_text, svg},
};

use super::{Notification, NotificationMessage, body_markup};
use crate::{
    config::{Config, Palette, toasts::ToastCardSizeConfig},
    data::{
        notifications::model::{NotifAction, NotifImage, NotificationData},
        severity::Severity,
        source::app_icon::DesktopEntries,
    },
    state::PanelLayout,
    wayland_layers::{bar::pill_shadow, tooltip::relative},
    widgets::text::{LINE_HEIGHT, bar_text},
};

const PADDING_PX: f32 = 10.0;
const SUBTEXT_SCALE: f32 = 0.85;
const COLUMN_SPACING: f32 = 4.0;
const BODY_ROW_SPACING: f32 = 10.0;
const ROW_SPACING: f32 = 8.0;
const BUTTON_PADDING: [f32; 2] = [6.0, 14.0];
const BORDER_WIDTH_PX: f32 = 1.5;
const MIN_BACKDROP_OPACITY: f32 = 0.7;

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

pub struct Card<'a> {
    notif: &'a Notification,
    config: &'a Config,
    palette: &'a Extended,
    layout: &'a PanelLayout,
    hovered: bool,
    reveal: f32,
}

impl<'a> Card<'a> {
    pub fn new(
        notif: &'a Notification,
        config: &'a Config,
        palette: &'a Extended,
        layout: &'a PanelLayout,
    ) -> Self {
        Self {
            notif,
            config,
            palette,
            layout,
            hovered: false,
            reveal: 1.0,
        }
    }

    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    pub fn reveal(mut self, reveal: f32) -> Self {
        self.reveal = reveal;
        self
    }
}

impl From<Card<'_>> for Element<'_, NotificationMessage> {
    fn from(card: Card<'_>) -> Self {
        let id = card.notif.data.id;
        let chrome = card.chrome();
        let mut body_row = Row::new()
            .spacing(BODY_ROW_SPACING)
            .align_y(Alignment::Center);
        if let Some(icon) = icon_slot(
            &card.notif.icons,
            &card.config.toasts.card_size,
            card.reveal,
        ) {
            body_row = body_row.push(icon);
        }
        body_row = body_row.push(card.column(chrome));

        let panel = container(body_row)
            .width(Length::Fixed(card.config.toasts.card_size.card_width_px))
            .padding(PADDING_PX)
            .style(move |_| container::Style {
                background: Some(chrome.background.into()),
                border: chrome.border,
                text_color: Some(chrome.text),
                shadow: chrome.shadow,
                ..Default::default()
            });

        mouse_area(panel)
            .on_enter(NotificationMessage::HoverChanged { id, hovered: true })
            .on_exit(NotificationMessage::HoverChanged { id, hovered: false })
            .on_press(NotificationMessage::BodyClicked(id))
            .on_right_press(NotificationMessage::Dismiss(id))
            .into()
    }
}

#[derive(Clone, Copy)]
struct CardChrome {
    background: Color,
    border: Border,
    text: Color,
    shadow: Shadow,
}

impl Card<'_> {
    fn chrome(&self) -> CardChrome {
        let pair = SeverityShades::of(self.notif.data.severity, self.palette).shown(self.hovered);
        let opacity = mix_opacity(MIN_BACKDROP_OPACITY, self.layout.tint_factor);
        let fade = |color: Color| color.scale_alpha(self.reveal);
        let shadow = pill_shadow(self.layout);
        CardChrome {
            background: fade(pair.color.scale_alpha(opacity)),
            border: Border {
                color: fade(pair.color),
                width: BORDER_WIDTH_PX,
                radius: self.layout.corner_radius,
            },
            text: fade(pair.text),
            shadow: Shadow {
                color: fade(shadow.color),
                ..shadow
            },
        }
    }
}

fn mix_opacity(floor: f32, tint_factor: f32) -> f32 {
    floor + (1.0 - floor) * tint_factor.clamp(0.0, 1.0)
}

struct SeverityShades {
    resting: Pair,
    hovered: Pair,
}

impl SeverityShades {
    fn of(severity: Severity, palette: &Extended) -> Self {
        match severity {
            Severity::Low => Self {
                resting: palette.background.strong,
                hovered: palette.background.strongest,
            },
            Severity::Normal => Self {
                resting: palette.primary.weak,
                hovered: palette.primary.base,
            },
            Severity::Warning => Self {
                resting: palette.warning.weak,
                hovered: palette.warning.base,
            },
            Severity::Critical => Self {
                resting: palette.danger.weak,
                hovered: palette.danger.base,
            },
        }
    }

    fn shown(self, hovered: bool) -> Pair {
        if hovered { self.hovered } else { self.resting }
    }
}

impl Card<'_> {
    fn column<'b>(&self, chrome: CardChrome) -> Column<'b, NotificationMessage> {
        let data = &self.notif.data;
        let id = data.id;
        let font = &self.config.font;
        let palette = &self.config.palette;

        let text = chrome.text;

        let button_actions: Vec<&NotifAction> = data
            .actions
            .iter()
            .filter(|a| a.key != "default" && !a.label.trim().is_empty())
            .collect();
        let has_actions = !button_actions.is_empty();
        let has_body = !data.body.is_empty();
        let reltime = relative(self.notif.created_at, SystemTime::now());
        let reltime_span = |visible: bool| {
            visible.then(|| {
                bar_text(font, reltime.clone())
                    .size(font.text_size_px * SUBTEXT_SCALE)
                    .color(text)
            })
        };

        let mut title = Row::new()
            .spacing(ROW_SPACING)
            .align_y(Alignment::Center)
            .push(
                bar_text(font, data.summary.clone())
                    .color(text)
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
                    .color(text),
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
                    rich_text(body_markup::body_spans(
                        &data.body,
                        text,
                        font.text_size_px * SUBTEXT_SCALE,
                        font,
                        Font::MONOSPACE,
                    ))
                    .line_height(LINE_HEIGHT)
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
                actions = actions.push(self.action_button(
                    action.label.clone(),
                    NotificationMessage::ActionClicked {
                        id,
                        key: action.key.clone(),
                    },
                    action_tint(&action.key, palette),
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

    fn action_button<'b>(
        &self,
        label: String,
        message: NotificationMessage,
        tint: Color,
    ) -> Element<'b, NotificationMessage> {
        let font = &self.config.font;
        let radius = self.layout.corner_radius;
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
                        radius,
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .on_press(message)
            .into()
    }
}

fn icon_slot<'a>(
    icons: &CardIcons,
    card_size: &ToastCardSizeConfig,
    reveal: f32,
) -> Option<Element<'a, NotificationMessage>> {
    match (&icons.image, &icons.badge) {
        (Some(image), badge) => {
            let main_size = card_size.image_size_px;
            let main = icon_widget(image, main_size, reveal);
            Some(match badge {
                Some(badge) => {
                    let overlay = container(icon_widget(badge, card_size.badge_size_px, reveal))
                        .width(main_size)
                        .height(main_size)
                        .align_x(Alignment::Start)
                        .align_y(Alignment::Start);
                    iced::widget::stack![main, overlay].clip(true).into()
                }
                None => main,
            })
        }
        (None, Some(badge)) => Some(icon_widget(badge, card_size.app_icon_size_px, reveal)),
        (None, None) => None,
    }
}

fn icon_widget<'a>(icon: &CardIcon, size: f32, reveal: f32) -> Element<'a, NotificationMessage> {
    match icon {
        CardIcon::Raster(handle) => image(handle.clone())
            .width(size)
            .height(size)
            .opacity(reveal)
            .into(),
        CardIcon::Svg(handle) => svg(handle.clone())
            .width(size)
            .height(size)
            .opacity(reveal)
            .into(),
    }
}

fn action_tint(key: &str, palette: &Palette) -> Color {
    match key {
        "quit" | "deny" | "end" | "reject" => palette.urgent,
        "stop" | "cancel" | "later" | "snooze" => palette.warning,
        _ => palette.success,
    }
}
