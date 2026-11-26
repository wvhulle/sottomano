mod toast_card_actions;
pub mod toast_card_icons;

use std::time::SystemTime;

use chrono::{DateTime, Utc};
use corredo::changing_text::MarkupText;
use iced::{
    Alignment, Color, Element, Length, Shadow, Theme, border,
    theme::palette::{Extended, Pair},
    widget::{Column, Row, container, mouse_area, progress_bar, space},
};
use sottomano_desktop_sources::desktop_notifications::notification_data::{
    NotifAction, NotificationBody, NotificationProgress, NotifyingAppName, ReplyPrompt,
};
use sottomano_toml_config::{Config, severity::Severity};
use uom::si::{f64::Ratio, ratio::ratio};

use self::toast_card_icons::{CardImage, icon_slot, preview_widget};
use super::{Notification, toast_card_backdrop::ToastCardBackdrop, toast_lifecycle::ToastKind};
use crate::{
    hover_cards::PointerPresence,
    icon_catalog::drawable_icon_catalog::IconCatalog,
    notification_toasts::toast_messages::NotificationMessage,
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

pub(crate) const TOAST_CARD_WIDTH_PX: f32 = 440.0;
const TOAST_PREVIEW_MAX_HEIGHT_PX: f32 = 180.0;

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
