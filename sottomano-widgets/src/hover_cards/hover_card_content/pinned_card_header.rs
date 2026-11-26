use corredo::{
    popovers::anchor_marker,
    theming::{OUTLINE_WIDTH_PX, primary_weak_color, text_color},
};
use iced::{
    Background, Element, Length,
    alignment::Vertical,
    widget::{Column, button, container, row, rule},
};

use super::HoverCardContent;
use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_menus::group_menu_state::{GroupMenuPin, PanelPin, PanelPinDuration},
    hover_cards::{CardHeader, PopupMessage},
    widgets::text::panel_text,
};

impl HoverCardContent<'_> {
    pub(super) fn card_header_rows(&self) -> Option<Element<'static, PopupMessage>> {
        let CardHeader::Pinned {
            group_title,
            pin: panel_pin,
        } = &self.hover_card.header
        else {
            return None;
        };
        let group_name = self.small_dim_text(group_title.to_string());
        let header: Element<'static, PopupMessage> = match &panel_pin.entry {
            GroupMenuPin::Group(_) => group_name.into(),
            GroupMenuPin::Item(indicator_key) => row([
                group_name.into(),
                self.small_dim_text("\u{203a}".to_owned()).into(),
                self.bold_title(&indicator_key.name).into(),
            ])
            .spacing(self.config.theme.spacing.item_gap)
            .align_y(Vertical::Center)
            .into(),
        };
        let unpin = self.release_action_row((**panel_pin).clone());
        let header_row: Element<'static, PopupMessage> =
            container(container(header).padding(self.title_padding()))
                .padding(self.edge_row_padding())
                .into();
        Some(
            Column::with_children([header_row, Self::separator_rule(), unpin])
                .spacing(self.config.theme.spacing.item_gap)
                .into(),
        )
    }

    fn separator_rule() -> Element<'static, PopupMessage> {
        rule::horizontal(OUTLINE_WIDTH_PX)
            .style(rule::default)
            .into()
    }

    fn release_action_row(&self, panel_pin: PanelPin) -> Element<'static, PopupMessage> {
        let font = &self.config.theme.font;
        let (release_glyph, release_text) = match panel_pin.duration {
            PanelPinDuration::Permanent => {
                (Phosphor::PushPin(Weight::Fill), "unpin from the panel")
            }
            PanelPinDuration::UntilAlertEnds => (
                Phosphor::PushPinSlash(Weight::Regular),
                "temporarily pinned while alerting: dismiss until it alerts again",
            ),
            PanelPinDuration::ForcedByConfig { setting_path } => {
                return self.forced_pin_row(setting_path);
            }
        };
        let label = row([
            IconGlyph::from(release_glyph).view(font.icon_size_px, text_color),
            panel_text(font, release_text).into(),
        ])
        .spacing(self.config.theme.spacing.item_gap)
        .align_y(Vertical::Center);
        let release_button = button(container(label).padding(self.title_padding()))
            .width(Length::Fill)
            .padding(
                self.edge_row_padding()
                    .vertical(self.config.theme.spacing.item_gap),
            )
            .style(|theme, status| button::Style {
                background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                    .then(|| Background::Color(primary_weak_color(theme))),
                text_color: text_color(theme),
                ..button::Style::default()
            })
            .on_press(PopupMessage::ReleaseFromPanel(panel_pin.clone()));
        anchor_marker(release_button.into(), panel_pin)
    }

    fn forced_pin_row(&self, setting_path: &'static str) -> Element<'static, PopupMessage> {
        let font = &self.config.theme.font;
        container(
            row([
                IconGlyph::from(Phosphor::LockSimple(Weight::Fill))
                    .view(font.icon_size_px, text_color),
                panel_text(font, format!("pinned by config: {setting_path}")).into(),
            ])
            .spacing(self.config.theme.spacing.item_gap)
            .align_y(Vertical::Center),
        )
        .padding(
            self.edge_row_padding()
                .vertical(self.config.theme.spacing.item_gap),
        )
        .width(Length::Fill)
        .into()
    }
}
