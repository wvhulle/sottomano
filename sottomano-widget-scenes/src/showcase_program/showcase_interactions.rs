use iced::Task;
use sottomano_widgets::{
    group_indicators::indicator_group_interaction::IndicatorGroupInteraction,
    group_menus::group_menu_state::{GroupMenuMessage, PanelPin, PanelPinDuration},
    hover_cards::{HoverTarget, PopupMessage},
    panel_menus::settings_menu::appearance_presets_group::AppearancePresetsMessage,
};

use super::{
    PRESS_REPLY_LATENCY, PendingFixtureReply, ShowcaseState, configured_theme, scheme_palette,
};
use crate::{
    fixtures::press_replies,
    showcase_message::{ShowcaseInteraction, ShowcaseMessage},
};

impl ShowcaseState {
    pub(super) fn interact(&mut self, interaction: ShowcaseInteraction) -> Task<ShowcaseMessage> {
        match interaction {
            ShowcaseInteraction::Group(IndicatorGroupInteraction::GroupMenus(message)) => {
                self.update_group_menus(message)
            }
            ShowcaseInteraction::Group(group_interaction) => {
                if let IndicatorGroupInteraction::AppearancePresets(
                    AppearancePresetsMessage::Apply(preset),
                ) = &group_interaction
                {
                    self.appearance = preset.applied_to(self.appearance);
                    self.theme =
                        configured_theme(&scheme_palette(&self.config, self.appearance.scheme));
                }
                let due = self.clock.instant + PRESS_REPLY_LATENCY;
                self.pending_press_replies.extend(
                    press_replies(&group_interaction, self.clock)
                        .into_iter()
                        .map(|reply| PendingFixtureReply { due, reply }),
                );
                let _interaction_for_another_group =
                    self.group
                        .interact(group_interaction, &self.config, self.clock);
                if let Some(scheme) = self.group.color_scheme()
                    && scheme != self.appearance.scheme
                {
                    self.appearance.scheme = scheme;
                    self.theme = configured_theme(&scheme_palette(&self.config, scheme));
                }
                Task::none()
            }
            ShowcaseInteraction::Popup(popup_message) => self.update_popup(popup_message),
            ShowcaseInteraction::Notification(message) => self.update_notification(message),
            ShowcaseInteraction::OutsideShowcase => Task::none(),
        }
    }

    pub(super) fn update_popup(&mut self, popup_message: PopupMessage) -> Task<ShowcaseMessage> {
        match popup_message {
            PopupMessage::HoverCard(hover_card) => self.update_hover_card(hover_card),
            PopupMessage::OutputWindows(window_interaction) => {
                let _interaction_for_another_group = self.group.interact(
                    IndicatorGroupInteraction::OutputWindows(window_interaction),
                    &self.config,
                    self.clock,
                );
                Task::none()
            }
            PopupMessage::ReleaseFromPanel(panel_pin) => self.release_from_panel(panel_pin),
            PopupMessage::CopyText(_)
            | PopupMessage::OpenUrl(_)
            | PopupMessage::FocusWindow(_)
            | PopupMessage::RefreshHoveredIndicator => Task::none(),
        }
    }

    fn release_from_panel(&mut self, panel_pin: PanelPin) -> Task<ShowcaseMessage> {
        let PanelPin {
            menu,
            entry,
            duration,
        } = panel_pin;
        let release = match duration {
            PanelPinDuration::Permanent => GroupMenuMessage::TogglePin { menu, entry },
            PanelPinDuration::UntilAlertEnds => GroupMenuMessage::DismissAlertPin { menu, entry },
            PanelPinDuration::ForcedByConfig { .. } => return Task::none(),
        };
        self.dismiss_hover_card();
        self.update_group_menus(release)
    }

    pub(super) fn update_group_menus(
        &mut self,
        message: GroupMenuMessage,
    ) -> Task<ShowcaseMessage> {
        if let GroupMenuMessage::InspectRow { indicator, .. } = &message {
            self.schedule_hover_replies(&HoverTarget::Indicator(indicator.clone()));
        }
        let _menu_task = self.group_menus.update(message);
        Task::none()
    }
}
