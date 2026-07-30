//! One notification drawn as a card. Chrome follows the tooltip/bar conventions
//! (opaque tinted fill, hairline border, config corner radius). Ports odin's
//! `NotificationCard`: app icon, summary, app name, markup body, action
//! buttons, and a relative arrival time riding the last content line.

use std::{path::PathBuf, time::SystemTime};

use iced::{
    Alignment, Border, Color, Element, Font, Length, Pixels, Size,
    advanced::{
        graphics::text::Paragraph,
        text::{Alignment as TextAlignment, LineHeight, Paragraph as _, Shaping, Text, Wrapping},
    },
    alignment::Vertical,
    widget::{Column, Row, button, container, image, mouse_area, rich_text, svg, text::Span},
};

use super::{Notification, NotificationMessage, markup};
use crate::{
    config::resolved::{ResolvedConfig, ResolvedFontConfig},
    data::{
        notifications::model::{NotifImage, Urgency},
        source::app_icon::DesktopEntries,
    },
    wayland_layers::tooltip::relative,
    widgets::text::bar_text,
};

const PADDING_PX: f32 = 10.0;
const SUBTEXT_SCALE: f32 = 0.85;
const COLUMN_SPACING: f32 = 4.0;
const BODY_ROW_SPACING: f32 = 10.0;
const ROW_SPACING: f32 = 8.0;
const BUTTON_PADDING: [f32; 2] = [6.0, 14.0];
const WIDEST_RELTIME: &str = "59 min ago";

/// A resolved icon, ready to render without touching the filesystem in `view`.
#[derive(Debug, Clone)]
pub enum CardIcon {
    Raster(image::Handle),
    Svg(svg::Handle),
}

/// The two icons a card can carry, resolved once at arrival: the notification's
/// own `image` (rendered large and central) and the `badge` app icon (a small
/// superscript over the image, or the sole icon when there is no image) —
/// mirroring odin's `NotificationCard`.
#[derive(Debug, Clone, Default)]
pub struct CardIcons {
    pub image: Option<CardIcon>,
    pub badge: Option<CardIcon>,
}

/// Resolve a notification's image and badge icons, reusing the shared
/// `.desktop` index. The `image` is the notification's own picture (the
/// `image-data` raster or `image-path` name/path — where `notify-send -i` puts
/// its icon); the `badge` is the application icon (the `app_icon` argument,
/// else the desktop entry). Does filesystem lookup — call once at arrival, not
/// per redraw.
pub fn resolve_icons(
    data: &crate::data::notifications::model::NotificationData,
    entries: Option<&DesktopEntries>,
) -> CardIcons {
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

/// Alpha-composite `fg` over an opaque `bg`, returning an opaque colour. Used
/// for the faint resting border: a translucent border would let the desktop
/// bleed through the 2px border ring on the otherwise-transparent toast
/// surface, so the faint look is baked into an opaque colour over the card fill
/// instead.
fn over(fg: Color, bg: Color) -> Color {
    Color {
        r: bg.r * (1.0 - fg.a) + fg.r * fg.a,
        g: bg.g * (1.0 - fg.a) + fg.g * fg.a,
        b: bg.b * (1.0 - fg.a) + fg.b * fg.a,
        a: 1.0,
    }
}

fn layout_text(content: &str, size: f32, font: Font, width: f32) -> Text<&str> {
    Text {
        content,
        bounds: Size::new(width, f32::INFINITY),
        size: Pixels(size),
        line_height: LineHeight::default(),
        font,
        align_x: TextAlignment::Left,
        align_y: Vertical::Top,
        shaping: Shaping::Advanced,
        wrapping: Wrapping::Word,
    }
}

fn text_block_height(content: &str, size: f32, font: Font, width: f32) -> f32 {
    if content.is_empty() {
        return 0.0;
    }
    Paragraph::with_text(layout_text(content, size, font, width))
        .min_bounds()
        .height
}

fn text_line_width(content: &str, size: f32, font: Font) -> f32 {
    if content.is_empty() {
        return 0.0;
    }
    Paragraph::with_text(layout_text(content, size, font, f32::INFINITY))
        .min_bounds()
        .width
}

fn spans_block_height(spans: &[Span<'_>], size: f32, font: Font, width: f32) -> f32 {
    if spans.is_empty() {
        return 0.0;
    }
    let base = layout_text("", size, font, width);
    Paragraph::with_spans(Text {
        content: spans,
        bounds: base.bounds,
        size: base.size,
        line_height: base.line_height,
        font: base.font,
        align_x: base.align_x,
        align_y: base.align_y,
        shaping: base.shaping,
        wrapping: base.wrapping,
    })
    .min_bounds()
    .height
}

pub fn measure_height(
    data: &crate::data::notifications::model::NotificationData,
    icons: &CardIcons,
    config: &ResolvedConfig,
) -> f32 {
    let font = &config.font;
    let notifications = &config.notifications;
    let bold = Font {
        weight: iced::font::Weight::Bold,
        ..font.iced_font
    };
    let subtext = font.text_size_px * SUBTEXT_SCALE;

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
    let reltime_width = text_line_width(WIDEST_RELTIME, subtext, font.iced_font) + ROW_SPACING;

    let app_name_beside_title = if data.app_name.is_empty() {
        0.0
    } else {
        text_line_width(&data.app_name, subtext, font.iced_font) + ROW_SPACING
    };
    let time_beside_title = if !has_actions && !has_body {
        reltime_width
    } else {
        0.0
    };
    let title = text_block_height(
        &data.summary,
        font.text_size_px,
        bold,
        column_width - app_name_beside_title - time_beside_title,
    );

    let body = if has_body {
        let time_beside_body = if has_actions { 0.0 } else { reltime_width };
        let spans = markup::body_spans(
            &data.body,
            config.palette.inactive,
            subtext,
            font,
            Font::MONOSPACE,
        );
        spans_block_height(
            &spans,
            subtext,
            font.iced_font,
            column_width - time_beside_body,
        )
    } else {
        0.0
    };

    let actions = if has_actions {
        LineHeight::default()
            .to_absolute(Pixels(font.text_size_px))
            .0
            + 2.0 * BUTTON_PADDING[0]
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
    let data = &notif.data;
    let id = data.id;
    let critical = data.urgency == Urgency::Critical;
    let font = &config.font;
    let palette = &config.palette;

    let foreground = palette.bar_foreground;
    let dim = palette.dim;
    let body_color = palette.inactive;

    // The buttons worth drawing: the client's actions minus the `default` action
    // (invoked by clicking the body) and any label-less blank.
    let button_actions: Vec<&crate::data::notifications::model::NotifAction> = data
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
                    weight: iced::font::Weight::Bold,
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
    // The time rides the title only when it is the last content line.
    if let Some(time) = reltime_span(!has_actions && !has_body) {
        title = title.push(time);
    }

    let mut column = Column::new().spacing(COLUMN_SPACING).push(title);
    if has_body {
        let mono = Font::MONOSPACE;
        let body = Row::new()
            .spacing(ROW_SPACING)
            .align_y(Alignment::End)
            .push(
                rich_text(markup::body_spans(
                    &data.body,
                    body_color,
                    font.text_size_px * SUBTEXT_SCALE,
                    font,
                    mono,
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
            let destructive = action.key == "quit" || action.key == "deny";
            let tint = if destructive {
                palette.urgent
            } else {
                palette.focus
            };
            actions = actions.push(action_button(
                action.label.clone(),
                NotificationMessage::ActionClicked {
                    id,
                    key: action.key.clone(),
                },
                tint,
                font,
            ));
        }
        actions = actions.push(iced::widget::Space::new().width(Length::Fill));
        if let Some(time) = reltime_span(true) {
            actions = actions.push(time);
        }
        column = column.push(actions);
    }

    let mut body_row = Row::new()
        .spacing(BODY_ROW_SPACING)
        .align_y(Alignment::Center);
    if let Some(icon) = icon_slot(&notif.icons, &config.notifications) {
        body_row = body_row.push(icon);
    }
    body_row = body_row.push(column);

    let radius = f32::from(config.layout.corner_radius_px);
    let fill = Color {
        a: 1.0,
        ..palette.bar_background
    };
    let background = fill;
    // Border: a faint hairline at rest, thicker in the accent tint on hover
    // (odin's `NotificationCard`). Critical keeps its urgent tint throughout;
    // others rest as a faint foreground hairline and reach for the focus accent
    // on hover. The resting hairline is composited opaque over the fill so the
    // border ring never lets the desktop bleed through it.
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
    let border_color = if hovered { accent } else { rest };
    let border_width = if hovered { 3.0 } else { 2.0 };

    let panel = container(body_row)
        .width(Length::Fixed(config.notifications.card_width))
        .padding(PADDING_PX)
        .style(move |_| container::Style {
            background: Some(background.into()),
            border: Border {
                color: border_color,
                width: border_width,
                radius: radius.into(),
            },
            text_color: Some(foreground),
            ..Default::default()
        });

    mouse_area(panel)
        .on_enter(NotificationMessage::HoverChanged { id, hovered: true })
        .on_exit(NotificationMessage::HoverChanged { id, hovered: false })
        .on_press(NotificationMessage::BodyClicked(id))
        .on_right_press(NotificationMessage::Dismiss(id))
        .into()
}

/// The card's icon slot: the notification image drawn large with the app icon
/// as a small badge over its top-left corner, or — when there is no image —
/// just the app icon at normal size. `None` when the notification carries
/// neither. Sizes come from the notification config so they can be tuned per
/// setup.
fn icon_slot<'a>(
    icons: &CardIcons,
    config: &crate::config::disk::NotificationConfig,
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
