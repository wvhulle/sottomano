use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use chrono::{DateTime, Utc};
use corredo::changing_text::MarkupText;
use iced::{
    Alignment, Border, Color, Element, Length, Shadow, Theme, border,
    theme::palette::{Extended, Pair},
    widget::{Column, Row, Space, button, container, mouse_area, progress_bar, space, text_input},
};
use sottomano_desktop_sources::{
    desktop_notifications::{
        notification_data::{
            ActionTint, DesktopEntryHint, NotifAction, NotificationBody, NotificationData,
            NotificationProgress, NotifyingAppName, ReplyPrompt,
        },
        notification_image_policy::ImageRole,
        spec_hints::{IconReference, SpecAppIcon, SpecImage},
    },
    theme_icons::icon_requests::{
        IconRequest, app_icon_request, notification_file_icon_request,
        notification_image_icon_request,
    },
};
use sottomano_toml_config::{Config, severity::Severity};
use uom::si::{f64::Ratio, ratio::ratio};

use super::{Notification, toast_card_backdrop::ToastCardBackdrop, toast_lifecycle::ToastKind};
use crate::{
    glyph_icons::icon_glyph::IconGlyph,
    hover_cards::PointerPresence,
    icon_catalog::{drawable_icon_catalog::IconCatalog, icon_handle::IconHandle},
    toast_messages::{NotificationMessage, ReplyMessage},
    widgets::{
        localized_durations::relative_moment_text,
        shared_metrics::{ACCENT_STROKE_PX, OUTLINE_WIDTH_PX, pill_corners},
        shell_layer_appearance::LayerAppearance,
        text::{panel_text, resolved_font_faces, resolved_text_face},
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
    Themed(IconRequest),
}

impl CardIcon {
    #[must_use]
    pub fn icon_request(&self) -> Option<&IconRequest> {
        match self {
            CardIcon::Themed(request) => Some(request),
            CardIcon::Handle(_) | CardIcon::IconGlyph(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum CardImage {
    Preview { icon: CardIcon, file: PathBuf },
    Icon(CardIcon),
    Imageless,
}

#[derive(Debug, Clone)]
pub enum CardBadge {
    Badged(CardIcon),
    Unbadged,
}

impl CardImage {
    fn referenced(icon_reference: &IconReference, image_role: ImageRole) -> Self {
        match icon_reference {
            IconReference::ThemeName(icon_name) => {
                Self::Icon(CardIcon::Themed(notification_image_icon_request(icon_name)))
            }
            IconReference::FileUri(image_path) => match image_role {
                ImageRole::Picture => Self::Preview {
                    icon: CardIcon::Handle(IconHandle::at(image_path)),
                    file: image_path.clone(),
                },
                ImageRole::Icon => {
                    Self::Icon(CardIcon::Themed(notification_file_icon_request(image_path)))
                }
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct CardIcons {
    pub image: CardImage,
    pub badge: CardBadge,
}

impl CardIcons {
    pub fn previewed_file(&self) -> Option<&Path> {
        match &self.image {
            CardImage::Preview { file, .. } => Some(file),
            CardImage::Icon(_) | CardImage::Imageless => None,
        }
    }
}

impl From<IconGlyph> for CardIcons {
    fn from(glyph: IconGlyph) -> Self {
        Self {
            image: CardImage::Icon(CardIcon::IconGlyph(glyph)),
            badge: CardBadge::Unbadged,
        }
    }
}

#[must_use]
pub fn resolve_icons(data: &NotificationData) -> CardIcons {
    let image = match &data.image {
        SpecImage::ImageData(pixels) | SpecImage::LegacyIconData(pixels) => {
            CardImage::Icon(CardIcon::Handle(IconHandle::from(pixels.as_ref())))
        }
        SpecImage::ImagePath(icon_reference) | SpecImage::AppIcon(icon_reference) => {
            CardImage::referenced(icon_reference, data.image_role)
        }
        SpecImage::Absent => CardImage::Imageless,
    };
    let badge_app_icon = match &data.image {
        SpecImage::AppIcon(_) => None,
        SpecImage::ImageData(_)
        | SpecImage::ImagePath(_)
        | SpecImage::LegacyIconData(_)
        | SpecImage::Absent => match &data.app_icon {
            SpecAppIcon::Given(IconReference::ThemeName(icon_name)) => Some(icon_name.as_str()),
            SpecAppIcon::Given(IconReference::FileUri(_)) | SpecAppIcon::Absent => None,
        },
    };
    let desktop_entry = match &data.desktop_entry {
        DesktopEntryHint::Entry(entry) => Some(entry.as_str()),
        DesktopEntryHint::Unhinted => None,
    };
    let badge = match badge_app_icon.or(desktop_entry) {
        Some(app_id) => CardBadge::Badged(CardIcon::Themed(app_icon_request(app_id))),
        None => CardBadge::Unbadged,
    };
    CardIcons { image, badge }
}

pub struct ToastCard<'toast, Routed> {
    notif: &'toast Notification<Routed>,
    icon_catalog: &'toast IconCatalog,
    config: &'toast Config,
    theme: &'toast Theme,
    overlay_appearance: LayerAppearance,
    now: SystemTime,
    pointer_presence: PointerPresence,
    reveal: f32,
    background_fade: CardBackgroundFade,
}

impl<'toast, Routed> ToastCard<'toast, Routed> {
    pub fn new(
        notif: &'toast Notification<Routed>,
        icon_catalog: &'toast IconCatalog,
        config: &'toast Config,
        theme: &'toast Theme,
        overlay_appearance: LayerAppearance,
        now: SystemTime,
    ) -> Self {
        Self {
            notif,
            icon_catalog,
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
        if let Some(icon) = icon_slot(&card.notif.icons, card.icon_catalog, card.reveal, text) {
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
            PointerPresence::NotPointedAt => self.overlay_appearance.frame.opacity,
        };
        let shadow = self.overlay_appearance.frame.lifted_drop_shadow();
        container::Style::default()
            .background(urgency.color.scale_alpha(opacity * self.reveal))
            .border(
                border::rounded(self.overlay_appearance.frame.corner_radius_px)
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
        MarkupText::new(body, resolved_text_face(font), self.urgency_pair().color)
            .size(font.small_text_size_px)
            .block_spacing(self.config.theme.spacing.item_gap)
            .column_gap(self.config.theme.spacing.group_gap)
            .corner_radius(self.overlay_appearance.frame.corner_radius_px)
            .into()
    }

    fn column<'view>(&self, text: Color) -> Column<'view, NotificationMessage> {
        let data = &self.notif.data;
        let font = &self.config.theme.font;

        let button_actions: Vec<&NotifAction> = data
            .actions
            .iter()
            .filter(|offered_action| {
                offered_action.key != "default" && !offered_action.label.trim().is_empty()
            })
            .filter(|offered_action| {
                offered_action.key != "move"
                    || !self.notif.place.window_focus.makes_move_pointless()
            })
            .collect();
        let previewed_file = self.notif.icons.previewed_file();
        let has_actions = !button_actions.is_empty() || previewed_file.is_some();
        let now = self.now;
        let has_body = data.body != NotificationBody::Bodiless;
        let started = match &self.notif.kind {
            ToastKind::Event { at, .. } => *at,
            ToastKind::Ongoing { since, .. } => *since,
        };
        let reltime = relative_moment_text(
            DateTime::<Utc>::from(started),
            DateTime::from(now),
            self.config.calendar.locale,
        );
        let reltime_span = || {
            panel_text(font, reltime.clone())
                .size(font.small_text_size_px)
                .font(resolved_font_faces(font).regular)
                .color(text)
        };

        let mut title = Row::new()
            .spacing(self.config.theme.spacing.group_gap)
            .align_y(Alignment::Center)
            .push(
                panel_text(font, data.summary.clone())
                    .color(text)
                    .font(resolved_font_faces(font).bold)
                    .width(Length::Fill),
            );
        if let NotifyingAppName::Named(app_name) = &data.app_name {
            title = title.push(
                panel_text(font, app_name.clone())
                    .size(font.small_text_size_px)
                    .font(resolved_font_faces(font).regular)
                    .color(text),
            );
        }
        if !has_actions && !has_body {
            title = title.push(reltime_span());
        }

        let mut column = Column::new()
            .spacing(self.config.theme.spacing.item_gap)
            .push(title);
        if let NotificationBody::Text(body) = &data.body {
            let body = Row::new()
                .spacing(self.config.theme.spacing.group_gap)
                .align_y(Alignment::End)
                .push(self.body(body));
            column = column.push(if has_actions {
                body
            } else {
                body.push(reltime_span())
            });
        }
        if let CardImage::Preview { icon, .. } = &self.notif.icons.image {
            column = column.push(preview_widget(
                icon,
                self.icon_catalog,
                TOAST_PREVIEW_MAX_HEIGHT_PX,
                self.reveal,
                text,
            ));
        }
        if let NotificationProgress::Measured(progress) = data.progress {
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
            column = column.push(actions.push(reltime_span()));
        }
        column
    }

    fn actions_row<'view>(
        &self,
        button_actions: &[&NotifAction],
        previewed_file: Option<&Path>,
    ) -> Row<'view, NotificationMessage> {
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

    fn reply_row<'view>(
        &self,
        placeholder: &str,
        submit: String,
    ) -> Element<'view, NotificationMessage> {
        let id = self.notif.data.id;
        let font = &self.config.theme.font;
        let radius = self.overlay_appearance.frame.corner_radius_px;
        let reply = move |message| NotificationMessage::Reply { id, message };
        let family = self.urgency_family();
        let field = text_input(placeholder, &self.notif.reply_draft)
            .font(resolved_font_faces(font).regular)
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

    fn action_button<'view>(
        &self,
        label: String,
        message: NotificationMessage,
        tint: FamilyShades,
    ) -> Element<'view, NotificationMessage> {
        let font = &self.config.theme.font;
        let radius = self.overlay_appearance.frame.corner_radius_px;
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

fn icon_slot<'view>(
    icons: &CardIcons,
    icon_catalog: &IconCatalog,
    reveal: f32,
    text: Color,
) -> Option<Element<'view, NotificationMessage>> {
    let CardImage::Icon(image) = &icons.image else {
        return match &icons.badge {
            CardBadge::Badged(badge) => Some(icon_widget(
                badge,
                icon_catalog,
                TOAST_ICON_SIZE_PX,
                reveal,
                text,
            )),
            CardBadge::Unbadged => None,
        };
    };
    let main = icon_widget(image, icon_catalog, TOAST_ICON_SIZE_PX, reveal, text);
    let CardBadge::Badged(badge) = &icons.badge else {
        return Some(main);
    };
    let overlay = container(icon_widget(
        badge,
        icon_catalog,
        TOAST_APP_BADGE_SIZE_PX,
        reveal,
        text,
    ))
    .width(TOAST_ICON_SIZE_PX)
    .height(TOAST_ICON_SIZE_PX);
    Some(iced::widget::stack![main, overlay].clip(true).into())
}

fn preview_widget<'view>(
    icon: &CardIcon,
    icon_catalog: &IconCatalog,
    max_height: f32,
    reveal: f32,
    text: Color,
) -> Element<'view, NotificationMessage> {
    let preview: Element<'view, NotificationMessage> = match icon {
        CardIcon::Handle(handle) => handle.scaled_down(reveal),
        CardIcon::IconGlyph(glyph) => glyph.view(max_height, move |_| text.scale_alpha(reveal)),
        CardIcon::Themed(request) => match icon_catalog.drawable_icon(request) {
            Some(drawable_icon) => drawable_icon.scaled_down(reveal),
            None => Space::new().into(),
        },
    };
    container(preview).max_height(max_height).into()
}

#[allow(clippy::cast_possible_truncation)]
fn progress_widget<'view>(
    progress: Ratio,
    text: Color,
    track: Color,
) -> Element<'view, NotificationMessage> {
    progress_bar(0.0..=1.0, progress.get::<ratio>() as f32)
        .girth(ACCENT_STROKE_PX)
        .style(move |_| progress_bar::Style {
            background: track.into(),
            bar: text.into(),
            border: pill_corners(ACCENT_STROKE_PX),
        })
        .into()
}

fn icon_widget<'view>(
    icon: &CardIcon,
    icon_catalog: &IconCatalog,
    size: f32,
    reveal: f32,
    text: Color,
) -> Element<'view, NotificationMessage> {
    match icon {
        CardIcon::IconGlyph(glyph) => glyph.view(size, move |_| text.scale_alpha(reveal)),
        CardIcon::Handle(handle) => handle.view(size, reveal),
        CardIcon::Themed(request) => match icon_catalog.drawable_icon(request) {
            Some(drawable_icon) => drawable_icon.view(size, reveal),
            None => Space::new().width(size).height(size).into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn notification_with(image: SpecImage, image_role: ImageRole) -> NotificationData {
        NotificationData {
            image,
            image_role,
            ..NotificationData::default()
        }
    }

    fn screenshot_file() -> IconReference {
        IconReference::FileUri(PathBuf::from("/home/u/Pictures/Screenshot.png"))
    }

    #[test]
    fn a_picture_file_is_previewed() {
        let card_icons = resolve_icons(&notification_with(
            SpecImage::ImagePath(screenshot_file()),
            ImageRole::Picture,
        ));
        assert_eq!(
            card_icons.previewed_file(),
            Some(Path::new("/home/u/Pictures/Screenshot.png"))
        );
    }

    #[test]
    fn an_icon_file_goes_through_the_catalog_without_file_actions() {
        let card_icons = resolve_icons(&notification_with(
            SpecImage::ImagePath(screenshot_file()),
            ImageRole::Icon,
        ));
        assert_eq!(card_icons.previewed_file(), None);
        assert!(matches!(
            card_icons.image,
            CardImage::Icon(CardIcon::Themed(IconRequest::IconFile { .. }))
        ));
    }

    #[test]
    fn a_theme_name_is_a_named_catalog_icon() {
        let card_icons = resolve_icons(&notification_with(
            SpecImage::ImagePath(IconReference::ThemeName(
                "drive-harddisk-usb-symbolic".to_owned(),
            )),
            ImageRole::Icon,
        ));
        assert!(matches!(
            card_icons.image,
            CardImage::Icon(CardIcon::Themed(IconRequest::NamedIcon { .. }))
        ));
    }

    #[test]
    fn the_app_icon_is_not_repeated_as_a_badge() {
        let app_icon = IconReference::ThemeName("firefox".to_owned());
        let card_icons = resolve_icons(&NotificationData {
            image: SpecImage::AppIcon(app_icon.clone()),
            app_icon: SpecAppIcon::Given(app_icon),
            ..NotificationData::default()
        });
        assert!(matches!(card_icons.badge, CardBadge::Unbadged));
    }
}
