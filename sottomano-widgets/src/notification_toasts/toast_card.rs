use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use chrono::{DateTime, Utc};
use iced::{
    Alignment, Border, Color, Element, Length, Shadow, Theme, border,
    theme::palette::{Extended, Pair},
    widget::{Column, Row, button, container, mouse_area, progress_bar, space, text_input},
};
use reqwest::Url;
use sottomano_desktop_sources::{
    desktop_notifications::notification_data::{
        ActionTint, NotifAction, NotifImage, NotificationData, ReplyPrompt,
    },
    desktop_windows_apps::desktop_entry_app_icons::DesktopEntries,
};
use sottomano_toml_config::{Config, severity::Severity};
use uom::si::{f64::Ratio, ratio::ratio};

use super::{Notification, toast_card_backdrop::ToastCardBackdrop, toast_lifecycle::ToastKind};
use crate::{
    hover_cards::PointerPresence,
    notification_toasts::markup_body::MarkupBody,
    toast_messages::{NotificationMessage, ReplyMessage},
    widgets::{
        icon_glyph::IconGlyph,
        icon_handle::IconHandle,
        localized_durations::relative_moment_text,
        shared_metrics::{ACCENT_STROKE_PX, OUTLINE_WIDTH_PX, pill_corners},
        shell_layer_appearance::LayerAppearance,
        text::{font_faces_of, panel_text, text_face_of},
        typing_under_cards::CardBackgroundFade,
    },
};

#[derive(Debug, Clone, Copy)]
struct FamilyShades {
    weak: Pair,
    base: Pair,
    strong: Pair,
}

fn family_shades(palette: &Extended, severity: Severity) -> FamilyShades {
    match severity {
        Severity::Low => FamilyShades {
            weak: palette.secondary.weak,
            base: palette.secondary.base,
            strong: palette.secondary.strong,
        },
        Severity::Normal => FamilyShades {
            weak: palette.primary.weak,
            base: palette.primary.base,
            strong: palette.primary.strong,
        },
        Severity::Warning => FamilyShades {
            weak: palette.warning.weak,
            base: palette.warning.base,
            strong: palette.warning.strong,
        },
        Severity::Critical => FamilyShades {
            weak: palette.danger.weak,
            base: palette.danger.base,
            strong: palette.danger.strong,
        },
    }
}

fn action_family(palette: &Extended, tint: ActionTint) -> FamilyShades {
    match tint {
        ActionTint::Proceed => FamilyShades {
            weak: palette.success.weak,
            base: palette.success.base,
            strong: palette.success.strong,
        },
        ActionTint::Inspect => family_shades(palette, Severity::Normal),
        ActionTint::Hold => family_shades(palette, Severity::Warning),
        ActionTint::Refuse => family_shades(palette, Severity::Critical),
    }
}

pub(crate) const TOAST_CARD_WIDTH_PX: f32 = 440.0;
const TOAST_ICON_SIZE_PX: f32 = 44.0;
const TOAST_APP_BADGE_SIZE_PX: f32 = 20.0;
const TOAST_PREVIEW_MAX_HEIGHT_PX: f32 = 180.0;

#[derive(Debug, Clone)]
pub enum CardIcon {
    Handle(IconHandle),
    IconGlyph(IconGlyph),
}

#[derive(Debug, Clone)]
pub enum CardImage {
    Preview { icon: CardIcon, file: PathBuf },
    Icon(CardIcon),
}

impl CardImage {
    fn named(name: &str, entries: Option<&DesktopEntries>) -> Option<Self> {
        match icon_file(name) {
            Some(file) => Some(Self::from(file)),
            None => themed_icon(name, entries).map(Self::Icon),
        }
    }
}

impl From<PathBuf> for CardImage {
    fn from(file: PathBuf) -> Self {
        Self::Preview {
            icon: CardIcon::at(&file),
            file,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CardIcons {
    pub image: Option<CardImage>,
    pub badge: Option<CardIcon>,
}

impl CardIcons {
    pub fn previewed_file(&self) -> Option<&Path> {
        match self.image.as_ref()? {
            CardImage::Preview { file, .. } => Some(file),
            CardImage::Icon(_) => None,
        }
    }
}

impl From<IconGlyph> for CardIcons {
    fn from(glyph: IconGlyph) -> Self {
        Self {
            image: Some(CardImage::Icon(CardIcon::IconGlyph(glyph))),
            badge: None,
        }
    }
}

impl CardIcon {
    fn at(path: &Path) -> Self {
        Self::Handle(IconHandle::at(path))
    }
}

pub fn resolve_icons(data: &NotificationData, entries: Option<&DesktopEntries>) -> CardIcons {
    let app_icon_file = data.app_icon.as_deref().and_then(icon_file);
    let image = match &data.image {
        Some(NotifImage::Rgba(pixels)) => Some(CardImage::Icon(CardIcon::Handle(
            IconHandle::from(pixels.as_ref()),
        ))),
        Some(NotifImage::Named(name)) => CardImage::named(name, entries),
        None => app_icon_file.map(CardImage::from),
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
    if !name.starts_with("file:") {
        return None;
    }
    Url::parse(name)
        .inspect_err(|parse_failure| {
            tracing::debug!("toast card: icon file url {name} is malformed: {parse_failure}");
        })
        .ok()?
        .to_file_path()
        .inspect_err(|()| tracing::debug!("toast card: icon url {name} is not a local file path"))
        .ok()
}

pub struct ToastCard<'a, Routed> {
    notif: &'a Notification<Routed>,
    config: &'a Config,
    theme: &'a Theme,
    overlay_appearance: LayerAppearance,
    now: SystemTime,
    pointer_presence: PointerPresence,
    reveal: f32,
    background_fade: CardBackgroundFade,
}

impl<'a, Routed> ToastCard<'a, Routed> {
    pub fn new(
        notif: &'a Notification<Routed>,
        config: &'a Config,
        theme: &'a Theme,
        overlay_appearance: LayerAppearance,
        now: SystemTime,
    ) -> Self {
        Self {
            notif,
            config,
            theme,
            overlay_appearance,
            now,
            pointer_presence: PointerPresence::NotPointedAt,
            reveal: 1.0,
            background_fade: CardBackgroundFade::Resting,
        }
    }

    #[must_use]
    pub fn pointer_presence(mut self, pointer_presence: PointerPresence) -> Self {
        self.pointer_presence = pointer_presence;
        self
    }

    #[must_use]
    pub fn background_fade(mut self, background_fade: CardBackgroundFade) -> Self {
        self.background_fade = background_fade;
        self
    }

    #[must_use]
    pub fn reveal(mut self, reveal: f32) -> Self {
        self.reveal = reveal;
        self
    }
}

impl<Routed> From<ToastCard<'_, Routed>> for Element<'_, NotificationMessage> {
    fn from(card: ToastCard<'_, Routed>) -> Self {
        let id = card.notif.data.id;
        let style = card.style();
        let text = style.text_color.unwrap_or_default();
        let mut body_row = Row::new()
            .spacing(card.config.theme.spacing.group_gap)
            .align_y(Alignment::Center);
        if let Some(icon) = icon_slot(&card.notif.icons, card.reveal, text) {
            body_row = body_row.push(icon);
        }
        body_row = body_row.push(card.column(text));

        let framed_card = container(body_row)
            .width(Length::Fixed(TOAST_CARD_WIDTH_PX))
            .padding(card.config.theme.spacing.surface_inset)
            .style(move |_| container::Style {
                text_color: style.text_color,
                ..container::Style::default()
            });
        let backdropped_card = ToastCardBackdrop::new(framed_card, style, card.background_fade);

        mouse_area(backdropped_card)
            .on_enter(NotificationMessage::HoverChanged {
                id,
                pointer_presence: PointerPresence::PointedAt,
            })
            .on_exit(NotificationMessage::HoverChanged {
                id,
                pointer_presence: PointerPresence::NotPointedAt,
            })
            .on_press(NotificationMessage::BodyClicked(id))
            .on_right_press(NotificationMessage::Dismiss(id))
            .into()
    }
}

impl<Routed> ToastCard<'_, Routed> {
    fn urgency_family(&self) -> FamilyShades {
        family_shades(self.theme.extended_palette(), self.notif.data.severity)
    }

    fn urgency_pair(&self) -> Pair {
        let family = self.urgency_family();
        match self.pointer_presence {
            PointerPresence::PointedAt => family.strong,
            PointerPresence::NotPointedAt => family.base,
        }
    }

    fn style(&self) -> container::Style {
        let urgency = self.urgency_pair();
        let opacity = match self.pointer_presence {
            PointerPresence::PointedAt => 1.0,
            PointerPresence::NotPointedAt => self.overlay_appearance.opacity,
        };
        let shadow = self.overlay_appearance.frame_style().lifted_drop_shadow();
        container::Style::default()
            .background(urgency.color.scale_alpha(opacity * self.reveal))
            .border(
                border::rounded(self.overlay_appearance.corner_radius_px)
                    .color(urgency.color.scale_alpha(self.reveal))
                    .width(OUTLINE_WIDTH_PX),
            )
            .color(urgency.text.scale_alpha(self.reveal))
            .shadow(Shadow {
                color: shadow.color.scale_alpha(self.reveal),
                ..shadow
            })
    }
}

impl<Routed> ToastCard<'_, Routed> {
    fn body(&self, body: &str) -> Element<'static, NotificationMessage> {
        let font = &self.config.theme.font;
        MarkupBody::new(body, text_face_of(font), self.urgency_pair().color)
            .size(font.small_text_size_px)
            .block_spacing(self.config.theme.spacing.item_gap)
            .column_gap(self.config.theme.spacing.group_gap)
            .corner_radius(self.overlay_appearance.corner_radius_px)
            .into()
    }

    fn column<'b>(&self, text: Color) -> Column<'b, NotificationMessage> {
        let data = &self.notif.data;
        let font = &self.config.theme.font;

        let button_actions: Vec<&NotifAction> = data
            .actions
            .iter()
            .filter(|a| a.key != "default" && !a.label.trim().is_empty())
            .collect();
        let previewed_file = self.notif.icons.previewed_file();
        let has_actions = !button_actions.is_empty() || previewed_file.is_some();
        let now = self.now;
        let body_text = &data.body;
        let has_body = body_text.is_some();
        let started = match &self.notif.kind {
            ToastKind::Event { at, .. } => *at,
            ToastKind::Ongoing { since, .. } => *since,
        };
        let reltime = relative_moment_text(
            DateTime::<Utc>::from(started),
            DateTime::from(now),
            self.config.calendar.locale,
        );
        let reltime_span = |visible: bool| {
            visible.then(|| {
                panel_text(font, reltime.clone())
                    .size(font.small_text_size_px)
                    .font(font_faces_of(font).regular)
                    .color(text)
            })
        };

        let mut title = Row::new()
            .spacing(self.config.theme.spacing.group_gap)
            .align_y(Alignment::Center)
            .push(
                panel_text(font, data.summary.clone())
                    .color(text)
                    .font(font_faces_of(font).bold)
                    .width(Length::Fill),
            );
        if let Some(app_name) = &data.app_name {
            title = title.push(
                panel_text(font, app_name.clone())
                    .size(font.small_text_size_px)
                    .font(font_faces_of(font).regular)
                    .color(text),
            );
        }
        if let Some(time) = reltime_span(!has_actions && !has_body) {
            title = title.push(time);
        }

        let mut column = Column::new()
            .spacing(self.config.theme.spacing.item_gap)
            .push(title);
        if let Some(body) = body_text {
            let body = Row::new()
                .spacing(self.config.theme.spacing.group_gap)
                .align_y(Alignment::End)
                .push(self.body(body));
            column = column.push(match reltime_span(!has_actions) {
                Some(time) => body.push(time),
                None => body,
            });
        }
        if let Some(CardImage::Preview { icon, .. }) = &self.notif.icons.image {
            column = column.push(preview_widget(
                icon,
                TOAST_PREVIEW_MAX_HEIGHT_PX,
                self.reveal,
                text,
            ));
        }
        if let Some(progress) = data.progress {
            column = column.push(progress_widget(
                progress,
                text,
                self.urgency_family().weak.color,
            ));
        }
        if let ReplyPrompt::Offered {
            placeholder,
            submit,
        } = &data.reply
        {
            column = column.push(self.reply_row(placeholder, submit.clone()));
        }
        if has_actions {
            let actions = self
                .actions_row(&button_actions, previewed_file)
                .push(space::horizontal());
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
        previewed_file: Option<&Path>,
    ) -> Row<'b, NotificationMessage> {
        let id = self.notif.data.id;
        let palette = self.theme.extended_palette();
        let proceed = action_family(palette, ActionTint::Proceed);
        let app_buttons = button_actions.iter().map(|action| {
            self.action_button(
                action.label.clone(),
                NotificationMessage::ActionClicked {
                    id,
                    key: action.key.clone(),
                },
                action_family(palette, action.tint),
            )
        });
        let file_buttons = previewed_file
            .map(|_| {
                [
                    self.action_button(
                        "Open".to_owned(),
                        NotificationMessage::OpenFile(id),
                        proceed,
                    ),
                    self.action_button(
                        "Copy path".to_owned(),
                        NotificationMessage::CopyFilePath(id),
                        proceed,
                    ),
                ]
            })
            .into_iter()
            .flatten();
        Row::with_children(app_buttons.chain(file_buttons))
            .spacing(self.config.theme.spacing.item_gap)
            .align_y(Alignment::Center)
    }

    fn reply_row<'b>(&self, placeholder: &str, submit: String) -> Element<'b, NotificationMessage> {
        let id = self.notif.data.id;
        let font = &self.config.theme.font;
        let radius = self.overlay_appearance.corner_radius_px;
        let reply = move |message| NotificationMessage::Reply { id, message };
        let family = self.urgency_family();
        let field = text_input(placeholder, &self.notif.reply_draft)
            .font(font_faces_of(font).regular)
            .size(font.text_size_px)
            .padding([
                self.config.theme.spacing.item_gap,
                self.config.theme.spacing.group_gap,
            ])
            .on_input(move |draft| reply(ReplyMessage::Edited(draft)))
            .on_submit(reply(ReplyMessage::Submitted))
            .style(move |_, _| text_input::Style {
                background: family.weak.color.into(),
                border: border::rounded(radius)
                    .color(family.strong.color)
                    .width(OUTLINE_WIDTH_PX),
                icon: family.weak.text,
                placeholder: family.strong.color,
                value: family.weak.text,
                selection: family.strong.color,
            });
        let row = Row::new()
            .spacing(self.config.theme.spacing.item_gap)
            .align_y(Alignment::Center)
            .push(field)
            .push(self.action_button(
                submit,
                reply(ReplyMessage::Submitted),
                action_family(self.theme.extended_palette(), ActionTint::Proceed),
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
        tint: FamilyShades,
    ) -> Element<'b, NotificationMessage> {
        let font = &self.config.theme.font;
        let radius = self.overlay_appearance.corner_radius_px;
        button(panel_text(font, label))
            .padding([
                self.config.theme.spacing.item_gap,
                self.config.theme.spacing.group_gap,
            ])
            .style(move |_, status| {
                let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
                let shade = if hovered { tint.base } else { tint.weak };
                button::Style {
                    background: Some(shade.color.into()),
                    text_color: shade.text,
                    border: Border {
                        radius: radius.into(),
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
    reveal: f32,
    text: Color,
) -> Option<Element<'a, NotificationMessage>> {
    let Some(CardImage::Icon(image)) = &icons.image else {
        return icons
            .badge
            .as_ref()
            .map(|badge| icon_widget(badge, TOAST_ICON_SIZE_PX, reveal, text));
    };
    let main = icon_widget(image, TOAST_ICON_SIZE_PX, reveal, text);
    let Some(badge) = &icons.badge else {
        return Some(main);
    };
    let overlay = container(icon_widget(badge, TOAST_APP_BADGE_SIZE_PX, reveal, text))
        .width(TOAST_ICON_SIZE_PX)
        .height(TOAST_ICON_SIZE_PX);
    Some(iced::widget::stack![main, overlay].clip(true).into())
}

fn preview_widget<'a>(
    icon: &CardIcon,
    max_height: f32,
    reveal: f32,
    text: Color,
) -> Element<'a, NotificationMessage> {
    let preview: Element<'a, NotificationMessage> = match icon {
        CardIcon::Handle(handle) => handle.scaled_down(reveal),
        CardIcon::IconGlyph(glyph) => glyph.view(max_height, move |_| text.scale_alpha(reveal)),
    };
    container(preview).max_height(max_height).into()
}

#[allow(clippy::cast_possible_truncation)]
fn progress_widget<'a>(
    progress: Ratio,
    text: Color,
    track: Color,
) -> Element<'a, NotificationMessage> {
    progress_bar(0.0..=1.0, progress.get::<ratio>() as f32)
        .girth(ACCENT_STROKE_PX)
        .style(move |_| progress_bar::Style {
            background: track.into(),
            bar: text.into(),
            border: pill_corners(ACCENT_STROKE_PX),
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
        CardIcon::IconGlyph(glyph) => glyph.view(size, move |_| text.scale_alpha(reveal)),
        CardIcon::Handle(handle) => handle.view(size, reveal),
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
