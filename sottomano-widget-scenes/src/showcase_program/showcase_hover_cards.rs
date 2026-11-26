use iced::Task;
use sottomano_widgets::{
    group_menus::group_menu_state::GroupMenuMessage,
    hover_cards::{
        AnchorEvent, HoverCardMessage, HoverTarget, PointerPresence,
        hover_card_surfaces::HoverEffect, hover_card_transition::CardRetention,
        laid_out_anchors::WidgetLiveness,
    },
};

use super::{HOVER_FETCH_LATENCY, PendingFixtureReply, ShowcaseState};
use crate::{
    showcase_message::{ShowcaseInteraction, ShowcaseMessage},
    showcased_group::SHOWCASE_OUTPUT,
};

impl ShowcaseState {
    pub(super) fn update_hover_card(
        &mut self,
        hover_card: HoverCardMessage,
    ) -> Task<ShowcaseMessage> {
        match hover_card {
            HoverCardMessage::Anchor(AnchorEvent::LaidOut(widget_anchor)) => {
                let _hide_due = self.hover_card.remember_anchor(
                    SHOWCASE_OUTPUT,
                    widget_anchor,
                    self.clock.instant,
                );
                Task::none()
            }
            HoverCardMessage::Anchor(AnchorEvent::Entered { target }) => {
                if self.hover_card.transition.shown_retention() != CardRetention::UntilDismissed {
                    self.open_hover_card(target);
                }
                Task::none()
            }
            HoverCardMessage::Anchor(AnchorEvent::HandlePointed { target, pointing }) => {
                self.hover_card
                    .set_handle_pointing(SHOWCASE_OUTPUT, &target, pointing);
                if pointing == PointerPresence::PointedAt
                    && (self.hover_card.hover_effect(&target, SHOWCASE_OUTPUT)
                        == HoverEffect::SwitchesMenu)
                {
                    self.open_hover_card_until_dismissed(target);
                }
                Task::none()
            }
            HoverCardMessage::Anchor(AnchorEvent::HandlePressed { target }) => {
                self.hover_card.set_handle_pointing(
                    SHOWCASE_OUTPUT,
                    &target,
                    PointerPresence::PointedAt,
                );
                if self.hover_card.target() == Some(&target) {
                    self.close_held_open_card();
                } else {
                    self.open_hover_card_until_dismissed(target);
                }
                Task::none()
            }
            HoverCardMessage::ShownWidgetDropped => Task::done(ShowcaseMessage::from(
                ShowcaseInteraction::from(HoverCardMessage::ConfirmShownWidgetDropped),
            )),
            HoverCardMessage::ConfirmShownWidgetDropped => {
                if self.hover_card.shown_widget_liveness() == WidgetLiveness::Dropped {
                    self.dismiss_hover_card();
                }
                Task::none()
            }
            HoverCardMessage::DismissHeldOpen => {
                if self.hover_card.transition.shown_retention() == CardRetention::UntilDismissed {
                    self.close_held_open_card();
                }
                Task::none()
            }
            message => {
                let _hide_due = self.hover_card.update(message, self.clock.instant);
                Task::none()
            }
        }
    }

    pub(super) fn open_hover_card(&mut self, target: HoverTarget) {
        if self.hover_card.target() != Some(&target) {
            let _closed_menu = self.group_menus.update(GroupMenuMessage::Closed);
        }
        self.schedule_hover_replies(&target);
        let panel_edge = self.panel_edge();
        self.hover_card.open(&panel_edge, target);
    }

    pub(super) fn open_hover_card_until_dismissed(&mut self, target: HoverTarget) {
        if self.hover_card.target() != Some(&target) {
            let _closed_menu = self.group_menus.update(GroupMenuMessage::Closed);
        }
        self.schedule_hover_replies(&target);
        let panel_edge = self.panel_edge();
        self.hover_card.open_until_dismissed(&panel_edge, target);
    }

    pub(super) fn close_held_open_card(&mut self) {
        let _closed_menu = self.group_menus.update(GroupMenuMessage::Closed);
        self.dismiss_hover_card();
    }

    pub(super) fn dismiss_hover_card(&mut self) {
        let departing = self
            .hover_card
            .departure(|target| self.hover_content(target));
        self.hover_card.dismiss(departing, self.clock.instant);
    }

    pub(super) fn schedule_hover_replies(&mut self, target: &HoverTarget) {
        let due = self.clock.instant + HOVER_FETCH_LATENCY;
        self.pending_hover_replies = self
            .group
            .start_hovered_fetch(target, &self.config, self.clock)
            .into_iter()
            .map(|reply| PendingFixtureReply { due, reply })
            .collect();
    }
}
