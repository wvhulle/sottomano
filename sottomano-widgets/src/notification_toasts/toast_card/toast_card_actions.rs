use std::path::Path;

use iced::{
    Alignment, Border, Element, border,
    theme::palette::Extended,
    widget::{Row, button, mouse_area, text_input},
};
use sottomano_desktop_sources::desktop_notifications::notification_data::{
    ActionTint, NotifAction,
};
use sottomano_toml_config::severity::Severity;

use super::{FamilyShades, ToastCard, family_shades};
use crate::{
    toast_messages::{NotificationMessage, ReplyMessage},
    widgets::{
        shared_metrics::OUTLINE_WIDTH_PX,
        text::{panel_text, resolved_font_faces},
    },
};

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

impl<Routed> ToastCard<'_, Routed> {
    pub(super) fn actions_row<'view>(
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

    pub(super) fn reply_row<'view>(
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
