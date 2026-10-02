use std::{path::PathBuf, time::SystemTime};

use iced::{
    Alignment, Border, Color, Element, Font, Length, Shadow,
    font::Weight,
    Theme,
    theme::palette::Pair,
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
    desktop_shell::hover_tooltips::relative,
    state::PanelLayout,
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

pub struct ToastCard<'a> {
    notif: &'a Notification,
    config: &'a Config,
    theme: &'a Theme,
    layout: &'a PanelLayout,
    status: CardStatus,
    reveal: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardStatus {
    Resting,
    Hovered,
}

impl<'a> ToastCard<'a> {
    pub fn new(
        notif: &'a Notification,
        config: &'a Config,
        theme: &'a Theme,
        layout: &'a PanelLayout,
    ) -> Self {
        Self {
            notif,
            config,
            theme,
            layout,
            status: CardStatus::Resting,
            reveal: 1.0,
        }
    }

    pub fn hovered(mut self, hovered: bool) -> Self {
        self.status = if hovered {
            CardStatus::Hovered
        } else {
            CardStatus::Resting
        };
        self
    }

    pub fn reveal(mut self, reveal: f32) -> Self {
        self.reveal = reveal;
        self
    }
}

impl From<ToastCard<'_>> for Element<'_, NotificationMessage> {
    fn from(card: ToastCard<'_>) -> Self {
        let id = card.notif.data.id;
        let style = card.style();
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
        body_row = body_row.push(card.column(style.text_color.unwrap_or_default()));

        let framed_card = container(body_row)
            .width(Length::Fixed(card.config.toasts.card_size.card_width_px))
            .padding(PADDING_PX)
            .style(move |_| style);

        mouse_area(framed_card)
            .on_enter(NotificationMessage::HoverChanged { id, hovered: true })
            .on_exit(NotificationMessage::HoverChanged { id, hovered: false })
            .on_press(NotificationMessage::BodyClicked(id))
            .on_right_press(NotificationMessage::Dismiss(id))
            .into()
    }
}

impl ToastCard<'_> {
    fn severity_style(&self) -> fn(&Theme, CardStatus) -> container::Style {
        match self.notif.data.severity {
            Severity::Low => low,
            Severity::Normal => normal,
            Severity::Warning => warning,
            Severity::Critical => critical,
        }
    }

    fn style(&self) -> container::Style {
        let severity = (self.severity_style())(self.theme, self.status);
        let opacity = translucent_opacity(self.layout.tint_factor) * self.reveal;
        let shadow = self.layout.frame_shadow();
        container::Style {
            background: severity
                .background
                .map(|background| background.scale_alpha(opacity)),
            border: Border {
                color: severity.border.color.scale_alpha(self.reveal),
                radius: self.layout.corner_radius,
                ..severity.border
            },
            text_color: severity
                .text_color
                .map(|text| text.scale_alpha(self.reveal)),
            shadow: Shadow {
                color: shadow.color.scale_alpha(self.reveal),
                ..shadow
            },
            ..severity
        }
    }
}

fn translucent_opacity(tint_factor: f32) -> f32 {
    MIN_BACKDROP_OPACITY + (1.0 - MIN_BACKDROP_OPACITY) * tint_factor.clamp(0.0, 1.0)
}

pub fn low(theme: &Theme, status: CardStatus) -> container::Style {
    let palette = theme.extended_palette();
    severity_tinted(match status {
        CardStatus::Resting => palette.background.strong,
        CardStatus::Hovered => palette.background.strongest,
    })
}

pub fn normal(theme: &Theme, status: CardStatus) -> container::Style {
    let palette = theme.extended_palette();
    severity_tinted(match status {
        CardStatus::Resting => palette.primary.weak,
        CardStatus::Hovered => palette.primary.base,
    })
}

pub fn warning(theme: &Theme, status: CardStatus) -> container::Style {
    let palette = theme.extended_palette();
    severity_tinted(match status {
        CardStatus::Resting => palette.warning.weak,
        CardStatus::Hovered => palette.warning.base,
    })
}

pub fn critical(theme: &Theme, status: CardStatus) -> container::Style {
    let palette = theme.extended_palette();
    severity_tinted(match status {
        CardStatus::Resting => palette.danger.weak,
        CardStatus::Hovered => palette.danger.base,
    })
}

fn severity_tinted(pair: Pair) -> container::Style {
    container::Style {
        background: Some(pair.color.into()),
        text_color: Some(pair.text),
        border: Border {
            color: pair.color,
            width: BORDER_WIDTH_PX,
            ..Border::default()
        },
        ..container::Style::default()
    }
}

impl ToastCard<'_> {
    fn column<'b>(&self, text: Color) -> Column<'b, NotificationMessage> {
        let data = &self.notif.data;
        let id = data.id;
        let font = &self.config.font;
        let palette = &self.config.palette;

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
