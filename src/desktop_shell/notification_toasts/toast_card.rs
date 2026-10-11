use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use iced::{
    Alignment, Border, Color, ContentFit, Element, Font, Length, Shadow, Theme, border,
    widget::{
        Column, Row, Space, button, container, image, mouse_area, progress_bar, rich_text, svg,
        text::Wrapping, text_input,
    },
};
use reqwest::Url;
use uom::si::{f64::Ratio, ratio::ratio};

use super::{Notification, NotificationMessage, body_markup, toast_reply::ReplyMessage};
use crate::{
    config::{Config, Palette, notification_toasts::ToastCardConfig},
    data::{
        notifications::model::{
            ActionTint, NotifAction, NotifImage, NotificationData, ReplyPrompt,
        },
        severity::Severity,
        source::app_icon::DesktopEntries,
    },
    desktop_shell::hover_tooltips::relative,
    state::ShellSettings,
    widgets::{
        bar_widget::Glyph,
        text::{LINE_HEIGHT, bar_text},
    },
};

const PADDING_PX: f32 = 10.0;
const COLUMN_SPACING: f32 = 4.0;
const BODY_ROW_SPACING: f32 = 10.0;
const ROW_SPACING: f32 = 8.0;
const BUTTON_PADDING: [f32; 2] = [6.0, 14.0];
const BORDER_WIDTH_PX: f32 = 1.5;
const PROGRESS_GIRTH_PX: f32 = 6.0;
const CODE_BLOCK_PADDING_PX: f32 = 6.0;
const CODE_BLOCK_TINT_ALPHA: f32 = 0.08;

#[derive(Debug, Clone)]
pub enum CardIcon {
    Raster(image::Handle),
    Svg(svg::Handle),
    Glyph(Glyph),
}

#[derive(Debug, Clone)]
pub enum CardImage {
    Preview { icon: CardIcon, file: PathBuf },
    Icon(CardIcon),
}

impl CardImage {
    fn of_file(file: PathBuf) -> Self {
        Self::Preview {
            icon: CardIcon::at(&file),
            file,
        }
    }

    fn named(name: &str, entries: Option<&DesktopEntries>) -> Option<Self> {
        match icon_file(name) {
            Some(file) => Some(Self::of_file(file)),
            None => themed_icon(name, entries).map(Self::Icon),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CardIcons {
    pub image: Option<CardImage>,
    pub badge: Option<CardIcon>,
}

impl CardIcons {
    pub fn of_glyph(glyph: Glyph) -> Self {
        Self {
            image: Some(CardImage::Icon(CardIcon::Glyph(glyph))),
            badge: None,
        }
    }

    pub fn previewed_file(&self) -> Option<&Path> {
        match self.image.as_ref()? {
            CardImage::Preview { file, .. } => Some(file),
            CardImage::Icon(_) => None,
        }
    }
}

impl CardIcon {
    fn at(path: &Path) -> Self {
        let is_svg = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("svg") || ext.eq_ignore_ascii_case("svgz"));
        if is_svg {
            Self::Svg(svg::Handle::from_path(path))
        } else {
            Self::Raster(image::Handle::from_path(path))
        }
    }
}

pub fn resolve_icons(data: &NotificationData, entries: Option<&DesktopEntries>) -> CardIcons {
    let app_icon_file = data.app_icon.as_deref().and_then(icon_file);
    let image = match &data.image {
        Some(NotifImage::Rgba(handle)) => Some(CardImage::Icon(CardIcon::Raster(handle.clone()))),
        Some(NotifImage::Named(name)) => CardImage::named(name, entries),
        None => app_icon_file.map(CardImage::of_file),
    };
    let badge = data
        .app_icon
        .as_deref()
        .filter(|name| icon_file(name).is_none())
        .or(data.desktop_entry.as_deref())
        .and_then(|name| themed_icon(name, entries));
    CardIcons { image, badge }
}

fn themed_icon(name: &str, entries: Option<&DesktopEntries>) -> Option<CardIcon> {
    let entries = entries?;
    let path = entries
        .theme_icon(name)
        .or_else(|| entries.icon_for(name))?;
    Some(CardIcon::at(&path))
}

fn icon_file(name: &str) -> Option<PathBuf> {
    if name.starts_with('/') {
        return Some(PathBuf::from(name));
    }
    Url::parse(name)
        .ok()
        .filter(|url| url.scheme() == "file")?
        .to_file_path()
        .ok()
}

pub struct ToastCard<'a> {
    notif: &'a Notification,
    config: &'a Config,
    theme: &'a Theme,
    settings: &'a ShellSettings,
    hovered: bool,
    reveal: f32,
}

impl<'a> ToastCard<'a> {
    pub fn new(
        notif: &'a Notification,
        config: &'a Config,
        theme: &'a Theme,
        settings: &'a ShellSettings,
    ) -> Self {
        Self {
            notif,
            config,
            theme,
            settings,
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

impl From<ToastCard<'_>> for Element<'_, NotificationMessage> {
    fn from(card: ToastCard<'_>) -> Self {
        let id = card.notif.data.id;
        let style = card.style();
        let text = style.text_color.unwrap_or_default();
        let mut body_row = Row::new()
            .spacing(BODY_ROW_SPACING)
            .align_y(Alignment::Center);
        if let Some(icon) = icon_slot(
            &card.notif.icons,
            &card.config.notification_toasts.card,
            card.reveal,
            text,
        ) {
            body_row = body_row.push(icon);
        }
        body_row = body_row.push(card.column(text));

        let framed_card = container(body_row)
            .width(Length::Fixed(card.config.notification_toasts.card.width_px))
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
    fn style(&self) -> container::Style {
        let palette = self.theme.extended_palette();
        let hovered = self.hovered;
        let urgency = match self.notif.data.severity {
            Severity::Low if hovered => palette.secondary.strong,
            Severity::Low => palette.secondary.base,
            Severity::Normal if hovered => palette.primary.strong,
            Severity::Normal => palette.primary.base,
            Severity::Warning if hovered => palette.warning.strong,
            Severity::Warning => palette.warning.base,
            Severity::Critical if hovered => palette.danger.strong,
            Severity::Critical => palette.danger.base,
        };
        let opacity = if hovered {
            1.0
        } else {
            self.settings.overlays.opacity
        };
        let shadow = self.settings.shadow.drop_shadow();
        container::Style::default()
            .background(urgency.color.scale_alpha(opacity * self.reveal))
            .border(
                border::rounded(self.settings.overlays.corner_radius)
                    .color(urgency.color.scale_alpha(self.reveal))
                    .width(BORDER_WIDTH_PX),
            )
            .color(urgency.text.scale_alpha(self.reveal))
            .shadow(Shadow {
                color: shadow.color.scale_alpha(self.reveal),
                ..shadow
            })
    }
}

impl ToastCard<'_> {
    fn body_paragraphs<'b>(&self, body: &str, text: Color) -> Column<'b, NotificationMessage> {
        let font = &self.config.theme.font;
        let corner_radius = self.settings.overlays.corner_radius;
        Column::with_children(
            body_markup::body_paragraphs(
                body,
                text,
                font.small_text_size_px,
                font,
                Font::MONOSPACE,
            )
            .into_iter()
            .map(|paragraph| {
                let flowing = rich_text(paragraph.spans)
                    .line_height(LINE_HEIGHT)
                    .wrapping(Wrapping::WordOrGlyph)
                    .width(Length::Fill);
                if paragraph.preformatted {
                    container(flowing)
                        .padding(CODE_BLOCK_PADDING_PX)
                        .width(Length::Fill)
                        .style(move |_| {
                            container::Style::default()
                                .background(text.scale_alpha(CODE_BLOCK_TINT_ALPHA))
                                .border(border::rounded(corner_radius))
                        })
                        .into()
                } else {
                    flowing.into()
                }
            }),
        )
        .spacing(COLUMN_SPACING)
        .width(Length::Fill)
    }

    fn column<'b>(&self, text: Color) -> Column<'b, NotificationMessage> {
        let data = &self.notif.data;
        let font = &self.config.theme.font;

        let button_actions: Vec<&NotifAction> = data
            .actions
            .iter()
            .filter(|a| a.key != "default" && !a.label.trim().is_empty())
            .collect();
        let previewed = self.notif.icons.previewed_file().is_some();
        let has_actions = !button_actions.is_empty() || previewed;
        let has_body = data.body.is_some();
        let reltime = relative(self.notif.created_at, SystemTime::now());
        let reltime_span = |visible: bool| {
            visible.then(|| {
                bar_text(font, reltime.clone())
                    .size(font.small_text_size_px)
                    .font(font.italic())
                    .color(text)
            })
        };

        let mut title = Row::new()
            .spacing(ROW_SPACING)
            .align_y(Alignment::Center)
            .push(
                bar_text(font, data.summary.clone())
                    .color(text)
                    .font(font.bold())
                    .width(Length::Fill),
            );
        if let Some(app_name) = &data.app_name {
            title = title.push(
                bar_text(font, app_name.clone())
                    .size(font.small_text_size_px)
                    .font(font.italic())
                    .color(text),
            );
        }
        if let Some(time) = reltime_span(!has_actions && !has_body) {
            title = title.push(time);
        }

        let mut column = Column::new().spacing(COLUMN_SPACING).push(title);
        if let Some(body) = &data.body {
            let body = Row::new()
                .spacing(ROW_SPACING)
                .align_y(Alignment::End)
                .push(self.body_paragraphs(body, text));
            column = column.push(match reltime_span(!has_actions) {
                Some(time) => body.push(time),
                None => body,
            });
        }
        if let Some(CardImage::Preview { icon, .. }) = &self.notif.icons.image {
            column = column.push(preview_widget(
                icon,
                self.config
                    .notification_toasts
                    .card
                    .image_preview_max_height_px,
                self.reveal,
                text,
            ));
        }
        if let Some(progress) = data.progress {
            column = column.push(progress_widget(progress, text));
        }
        if let ReplyPrompt::Offered {
            placeholder,
            submit,
        } = &data.reply
        {
            column = column.push(self.reply_row(placeholder, submit.clone(), text));
        }
        if has_actions {
            let actions = self
                .actions_row(&button_actions, previewed, text)
                .push(Space::new().width(Length::Fill));
            column = column.push(match reltime_span(true) {
                Some(time) => actions.push(time),
                None => actions,
            });
        }
        column
    }

    fn actions_row<'b>(
        &self,
        button_actions: &[&NotifAction],
        previewed: bool,
        text: Color,
    ) -> Row<'b, NotificationMessage> {
        let id = self.notif.data.id;
        let palette = &self.config.theme.palette;
        let app_buttons = button_actions.iter().map(|action| {
            self.action_button(
                action.label.clone(),
                NotificationMessage::ActionClicked {
                    id,
                    key: action.key.clone(),
                },
                action.tint.color(palette),
                text,
            )
        });
        let file_buttons = previewed
            .then(|| {
                [
                    self.action_button(
                        "Open".to_owned(),
                        NotificationMessage::OpenFile(id),
                        palette.success,
                        text,
                    ),
                    self.action_button(
                        "Copy path".to_owned(),
                        NotificationMessage::CopyFilePath(id),
                        palette.success,
                        text,
                    ),
                ]
            })
            .into_iter()
            .flatten();
        Row::with_children(app_buttons.chain(file_buttons))
            .spacing(6.0)
            .align_y(Alignment::Center)
    }

    fn reply_row<'b>(
        &self,
        placeholder: &str,
        submit: String,
        text: Color,
    ) -> Element<'b, NotificationMessage> {
        let id = self.notif.data.id;
        let font = &self.config.theme.font;
        let radius = self.settings.overlays.corner_radius;
        let reply = move |message| NotificationMessage::Reply { id, message };
        let field = text_input(placeholder, &self.notif.reply_draft)
            .font(font.iced_font)
            .size(font.text_size_px)
            .padding(BUTTON_PADDING)
            .on_input(move |draft| reply(ReplyMessage::Edited(draft)))
            .on_submit(reply(ReplyMessage::Submitted))
            .style(move |_, _| text_input::Style {
                background: text.scale_alpha(0.12).into(),
                border: border::rounded(radius)
                    .color(text.scale_alpha(0.5))
                    .width(1.0),
                icon: text,
                placeholder: text.scale_alpha(0.6),
                value: text,
                selection: text.scale_alpha(0.3),
            });
        let row = Row::new()
            .spacing(6.0)
            .align_y(Alignment::Center)
            .push(field)
            .push(self.action_button(
                submit,
                reply(ReplyMessage::Submitted),
                self.config.theme.palette.success,
                text,
            ));
        mouse_area(row)
            .on_enter(reply(ReplyMessage::PointerEntered))
            .on_exit(reply(ReplyMessage::PointerLeft))
            .into()
    }

    fn action_button<'b>(
        &self,
        label: String,
        message: NotificationMessage,
        tint: Color,
        text: Color,
    ) -> Element<'b, NotificationMessage> {
        let font = &self.config.theme.font;
        let radius = self.settings.overlays.corner_radius;
        button(bar_text(font, label).size(font.text_size_px).color(text))
            .padding(BUTTON_PADDING)
            .style(move |_, status| {
                let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: Some(
                        Color {
                            a: if hovered { 0.4 } else { 0.25 },
                            ..tint
                        }
                        .into(),
                    ),
                    text_color: text,
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
    card_size: &ToastCardConfig,
    reveal: f32,
    text: Color,
) -> Option<Element<'a, NotificationMessage>> {
    let Some(CardImage::Icon(image)) = &icons.image else {
        return icons
            .badge
            .as_ref()
            .map(|badge| icon_widget(badge, card_size.icon_size_px, reveal, text));
    };
    let main_size = card_size.icon_size_px;
    let main = icon_widget(image, main_size, reveal, text);
    let Some(badge) = &icons.badge else {
        return Some(main);
    };
    let overlay = container(icon_widget(
        badge,
        card_size.app_badge_size_px,
        reveal,
        text,
    ))
    .width(main_size)
    .height(main_size)
    .align_x(Alignment::Start)
    .align_y(Alignment::Start);
    Some(iced::widget::stack![main, overlay].clip(true).into())
}

fn preview_widget<'a>(
    icon: &CardIcon,
    max_height: f32,
    reveal: f32,
    text: Color,
) -> Element<'a, NotificationMessage> {
    let preview: Element<'a, NotificationMessage> = match icon {
        CardIcon::Raster(handle) => image(handle.clone())
            .width(Length::Fill)
            .content_fit(ContentFit::ScaleDown)
            .opacity(reveal)
            .into(),
        CardIcon::Svg(handle) => svg(handle.clone())
            .width(Length::Fill)
            .content_fit(ContentFit::ScaleDown)
            .opacity(reveal)
            .into(),
        CardIcon::Glyph(glyph) => glyph.view(max_height, text.scale_alpha(reveal)),
    };
    container(preview).max_height(max_height).into()
}

#[allow(clippy::cast_possible_truncation)]
fn progress_widget<'a>(progress: Ratio, text: Color) -> Element<'a, NotificationMessage> {
    progress_bar(0.0..=1.0, progress.get::<ratio>() as f32)
        .girth(PROGRESS_GIRTH_PX)
        .style(move |_| progress_bar::Style {
            background: text.scale_alpha(0.2).into(),
            bar: text.into(),
            border: border::rounded(PROGRESS_GIRTH_PX / 2.0),
        })
        .into()
}

fn icon_widget<'a>(
    icon: &CardIcon,
    size: f32,
    reveal: f32,
    text: Color,
) -> Element<'a, NotificationMessage> {
    match icon {
        CardIcon::Glyph(glyph) => glyph.view(size, text.scale_alpha(reveal)),
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

impl ActionTint {
    fn color(self, palette: &Palette) -> Color {
        match self {
            ActionTint::Proceed => palette.success,
            ActionTint::Hold => palette.warning,
            ActionTint::Refuse => palette.urgent,
            ActionTint::Inspect => palette.info,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::icon_file;

    #[test]
    fn percent_encoded_file_url_resolves_to_decoded_path() {
        assert_eq!(
            icon_file("file:///home/u/Pictures/Screenshot%20from%202026-10-07.png"),
            Some(PathBuf::from(
                "/home/u/Pictures/Screenshot from 2026-10-07.png"
            ))
        );
    }

    #[test]
    fn absolute_path_is_taken_as_is() {
        assert_eq!(
            icon_file("/usr/share/icons/a b.png"),
            Some(PathBuf::from("/usr/share/icons/a b.png"))
        );
    }

    #[test]
    fn theme_icon_name_is_not_a_file() {
        assert_eq!(icon_file("dialog-information"), None);
        assert_eq!(icon_file("https://example.com/a.png"), None);
    }
}
