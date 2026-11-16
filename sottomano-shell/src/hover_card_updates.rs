use std::{mem::replace, time::Instant};

use iced::{Task, window::Id};
use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::{
    group_indicators::group_indicator::{HoverFetch, IndicatorRefresh},
    hover_cards::{
        AnchorEvent, HoverCardMessage, HoverTarget, PointerPresence,
        hover_card_surfaces::HoverEffect,
        hover_card_transition::{CardRetention, Departed},
        laid_out_anchors::WidgetLiveness,
    },
    indicator_groups::window_group::WindowInspection,
    layer_surfaces::PanelEdge,
    widgets::pointer_drag::PointerDrag,
};
use tokio::time::sleep_until;

use crate::{desktop_shell::DesktopShell, shell_message::Message};

const HOVER_LOG_TARGET: &str = "sottomano_shell::hover_cards";

fn hide_hover_card_at(due: Instant) -> Task<Message> {
    Task::future(async move {
        sleep_until(due.into()).await;
        Message::HoverCardHideElapsed
    })
}

impl DesktopShell {
    pub fn update_hover_card(&mut self, msg: HoverCardMessage) -> Task<Message> {
        match msg {
            HoverCardMessage::OnSurface {
                surface,
                position,
                event,
            } => self.on_surface_event(surface, position, *event),
            HoverCardMessage::ShownWidgetDropped => {
                Task::done(Message::from(HoverCardMessage::ConfirmShownWidgetDropped))
            }
            HoverCardMessage::ConfirmShownWidgetDropped => {
                if self.hover_card.shown_widget_liveness() == WidgetLiveness::Dropped {
                    self.dismiss_hover_card()
                } else {
                    Task::none()
                }
            }
            HoverCardMessage::DismissHeldOpen => {
                if self.hover_card.transition.shown_retention() == CardRetention::UntilDismissed {
                    self.dismiss_hover_card()
                } else {
                    Task::none()
                }
            }
            HoverCardMessage::SwitchIntentElapsed => {
                match self.hover_card.take_pending_switch() {
                    Some(pending) => {
                        tracing::trace!(target: HOVER_LOG_TARGET, target = ?pending.target, "opened deferred switch");
                        self.hover_card.open(&pending.parent, pending.target);
                    }
                    None => {
                        tracing::trace!(target: HOVER_LOG_TARGET, "dropped deferred switch: no pending switch or its widget is gone");
                    }
                }
                Task::none()
            }
            unanchored @ HoverCardMessage::Anchor(_) => {
                tracing::trace!(target: HOVER_LOG_TARGET, message = ?unanchored, "dropped: reached the shell without its surface and screen edge");
                Task::none()
            }
            other => self
                .hover_card
                .update(other, self.clock.instant)
                .map_or_else(Task::none, hide_hover_card_at),
        }
    }

    fn on_surface_event(
        &mut self,
        surface: Id,
        position: Position,
        event: AnchorEvent,
    ) -> Task<Message> {
        match event {
            AnchorEvent::LaidOut(widget_anchor) => {
                let Some(output) = self.surfaces.get(&surface).map(|host| host.output.clone())
                else {
                    return Task::none();
                };
                self.hover_card
                    .remember_anchor(&output, widget_anchor, self.clock.instant)
                    .map_or_else(Task::none, hide_hover_card_at)
            }
            AnchorEvent::Entered { target } => {
                if self.hover_card.transition.shown_retention() == CardRetention::UntilDismissed {
                    tracing::trace!(target: HOVER_LOG_TARGET, ?target, ?position, "ignored: a card is held open");
                    return Task::none();
                }
                let Some(parent) = self.hover_card_parent(surface, position, &target) else {
                    return Task::none();
                };
                if let Some(intent_elapsed) = self.hover_card.defer_switch(&parent, &target) {
                    tracing::trace!(target: HOVER_LOG_TARGET, ?target, ?position, "deferred switch");
                    return intent_elapsed.map(Message::from);
                }
                tracing::trace!(target: HOVER_LOG_TARGET, ?target, ?position, "opened");
                self.hover_card.open(&parent, target);
                Task::none()
            }
            AnchorEvent::HandlePointed { target, pointing } => {
                self.follow_menu_handle_pointer(surface, position, target, pointing)
            }
            AnchorEvent::HandlePressed { target } => {
                self.toggle_menu_of_handle(surface, position, target)
            }
        }
    }

    fn follow_menu_handle_pointer(
        &mut self,
        surface: Id,
        position: Position,
        target: HoverTarget,
        pointing: PointerPresence,
    ) -> Task<Message> {
        let Some(output) = self.surfaces.get(&surface).map(|host| host.output.clone()) else {
            return Task::none();
        };
        self.hover_card
            .set_handle_pointing(&output, &target, pointing);
        if pointing == PointerPresence::PointedAt
            && self.hover_card.hover_effect(&target, &output) == HoverEffect::SwitchesMenu
            && let Some(parent) = self.hover_card_parent(surface, position, &target)
        {
            self.hover_card.open_until_dismissed(&parent, target);
        }
        Task::none()
    }

    fn toggle_menu_of_handle(
        &mut self,
        surface: Id,
        position: Position,
        target: HoverTarget,
    ) -> Task<Message> {
        let Some(output) = self.surfaces.get(&surface).map(|host| host.output.clone()) else {
            return Task::none();
        };
        self.hover_card
            .set_handle_pointing(&output, &target, PointerPresence::PointedAt);
        if self.hover_card.target() == Some(&target) {
            return self.dismiss_hover_card();
        }
        if let Some(parent) = self.hover_card_parent(surface, position, &target) {
            self.hover_card.open_until_dismissed(&parent, target);
        }
        Task::none()
    }

    fn hover_card_parent(
        &self,
        surface: Id,
        position: Position,
        target: &HoverTarget,
    ) -> Option<PanelEdge> {
        if matches!(self.pointer_drag, PointerDrag::Dragging(_)) {
            tracing::trace!(target: HOVER_LOG_TARGET, ?target, "dropped: a slot is being dragged");
            return None;
        }
        let Some(host) = self.surfaces.get(&surface) else {
            tracing::trace!(target: HOVER_LOG_TARGET, ?target, ?surface, "dropped: unknown surface");
            return None;
        };
        if self.hover_content(target).is_none() {
            tracing::trace!(target: HOVER_LOG_TARGET, ?target, "dropped: no card content");
            return None;
        }
        Some(PanelEdge {
            output: host.output.clone(),
            position,
        })
    }

    pub fn leave_hover_card(&mut self) -> Task<Message> {
        hide_hover_card_at(self.hover_card.leave_after_grace(self.clock.instant))
    }

    fn hover_card_departure(&self) -> Option<Departed> {
        self.hover_card
            .departure(|target| self.hover_content(target))
    }

    pub fn dismiss_hover_card(&mut self) -> Task<Message> {
        let departing = self.hover_card_departure();
        self.hover_card.dismiss(departing, self.clock.instant);
        Task::none()
    }

    pub fn leave_hover_card_after_grace(&mut self) -> Task<Message> {
        let departing = self.hover_card_departure();
        self.hover_card.leave_elapsed(self.clock.instant, departing);
        Task::none()
    }

    pub(crate) fn hovered_indicator_target(&self) -> Option<HoverTarget> {
        match self.hover_card.target() {
            Some(HoverTarget::GroupMenu(topic)) => Some(
                self.indicator_groups
                    .group_menus
                    .inspected_indicator(*topic)
                    .cloned()
                    .map_or(HoverTarget::GroupMenu(*topic), HoverTarget::Indicator),
            ),
            open_target => open_target.cloned(),
        }
    }

    fn hovered_row_fetch(&self) -> HoverFetch {
        self.hovered_indicator_target()
            .and_then(|target| self.hovered_indicator_requests(&target))
            .map_or(HoverFetch::NothingToFetch, |requests| requests.hover_fetch)
    }

    fn start_hovered_row_fetch(&mut self) -> Task<Message> {
        self.active_hover_fetch = self.hovered_row_fetch();
        match &self.active_hover_fetch {
            HoverFetch::Fetch { start, .. } => {
                let start = start.as_ref().clone();
                self.dispatch_message(Message::from(start))
            }
            HoverFetch::NothingToFetch => Task::none(),
        }
    }

    pub fn refresh_hovered_indicator(&mut self) -> Task<Message> {
        let refreshed = match self
            .hovered_indicator_target()
            .and_then(|target| self.hovered_indicator_requests(&target))
            .map_or(IndicatorRefresh::Unrefreshable, |requests| requests.refresh)
        {
            IndicatorRefresh::Refreshes(refresh) => self.dispatch_message(Message::from(refresh)),
            IndicatorRefresh::Unrefreshable => Task::none(),
        };
        Task::batch([refreshed, self.start_hovered_row_fetch()])
    }

    pub fn retarget_hover_indicators(&mut self) -> Task<Message> {
        let stopped = match replace(&mut self.active_hover_fetch, HoverFetch::NothingToFetch) {
            HoverFetch::Fetch { stop, .. } => self.dispatch_message(Message::from(*stop)),
            HoverFetch::NothingToFetch => Task::none(),
        };
        let hovered_target = self.hovered_indicator_target();
        let watched_window = self
            .indicator_groups
            .window_group
            .follow_window_inspection(
                &self.compositor,
                WindowInspection::from(hovered_target.as_ref()),
                &self.config,
                self.clock,
            )
            .map(Message::WindowGroup);
        Task::batch([stopped, watched_window, self.start_hovered_row_fetch()])
    }
}
