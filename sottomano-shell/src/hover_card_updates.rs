use std::time::Instant;

use iced::{Task, window::Id};
use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::{
    hover_cards::{
        HoverAnnouncement, HoverCardMessage, HoverTarget, PointerPresence,
        hover_card_surfaces::{CardSubstance, HoverEffect, ShownWidgetPresence},
        hover_card_transition::{CardRetention, Departed},
    },
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
            HoverCardMessage::LaidOut {
                surface,
                widget_anchor,
            } => {
                let Some(output) = self.surfaces.get(&surface).map(|host| host.output.clone())
                else {
                    return Task::none();
                };
                self.hover_card
                    .remember_anchor(&output, widget_anchor, self.clock.instant)
                    .map_or_else(Task::none, hide_hover_card_at)
            }
            HoverCardMessage::Enter {
                surface,
                position,
                target,
                announcement,
            } => {
                if self.hover_card.shown_retention() == CardRetention::UntilDismissed {
                    tracing::trace!(target: HOVER_LOG_TARGET, ?target, ?position, "ignored: a card is held open");
                    return Task::none();
                }
                let Some(parent) =
                    self.hover_card_parent(surface, position, &target, &announcement)
                else {
                    return Task::none();
                };
                if let Some(intent_elapsed) =
                    self.hover_card
                        .defer_switch(&parent, &target, announcement.clone())
                {
                    tracing::trace!(target: HOVER_LOG_TARGET, ?target, ?position, "deferred switch");
                    return intent_elapsed.map(Message::HoverCard);
                }
                tracing::trace!(target: HOVER_LOG_TARGET, ?target, ?position, "opened");
                self.hover_card.open(&parent, target, announcement);
                Task::none()
            }
            HoverCardMessage::MenuHandlePointed {
                surface,
                position,
                target,
                pointing,
                announcement,
            } => self.follow_menu_handle_pointer(surface, position, target, pointing, announcement),
            HoverCardMessage::MenuHandlePressed {
                surface,
                position,
                target,
                announcement,
            } => self.toggle_menu_of_handle(surface, position, target, announcement),
            HoverCardMessage::ShownWidgetDropped => Task::done(Message::HoverCard(
                HoverCardMessage::ConfirmShownWidgetDropped,
            )),
            HoverCardMessage::ConfirmShownWidgetDropped => {
                if self.hover_card.shown_widget_presence() == ShownWidgetPresence::Dropped {
                    self.dismiss_hover_card()
                } else {
                    Task::none()
                }
            }
            HoverCardMessage::DismissHeldOpen => {
                if self.hover_card.shown_retention() == CardRetention::UntilDismissed {
                    self.dismiss_hover_card()
                } else {
                    Task::none()
                }
            }
            HoverCardMessage::SwitchIntentElapsed => {
                match self.hover_card.take_pending_switch() {
                    Some(pending) => {
                        tracing::trace!(target: HOVER_LOG_TARGET, target = ?pending.target, "opened deferred switch");
                        self.hover_card
                            .open(&pending.parent, pending.target, pending.announcement);
                    }
                    None => {
                        tracing::trace!(target: HOVER_LOG_TARGET, "dropped deferred switch: no pending switch or its widget is gone");
                    }
                }
                Task::none()
            }
            unanchored @ (HoverCardMessage::EnterAnchor { .. }
            | HoverCardMessage::AnchorLaidOut(..)
            | HoverCardMessage::MenuHandlePointedAnchor { .. }
            | HoverCardMessage::MenuHandlePressedAnchor { .. }) => {
                tracing::trace!(target: HOVER_LOG_TARGET, message = ?unanchored, "dropped: reached the shell without its surface and screen edge");
                Task::none()
            }
            other => self
                .hover_card
                .update(other, self.clock.instant)
                .map_or_else(Task::none, hide_hover_card_at),
        }
    }

    fn follow_menu_handle_pointer(
        &mut self,
        surface: Id,
        position: Position,
        target: HoverTarget,
        pointing: PointerPresence,
        announcement: HoverAnnouncement,
    ) -> Task<Message> {
        let Some(output) = self.surfaces.get(&surface).map(|host| host.output.clone()) else {
            return Task::none();
        };
        self.hover_card
            .set_handle_pointing(&output, &target, pointing);
        if pointing == PointerPresence::PointedAt
            && self.hover_card.hover_effect(&target, &output) == HoverEffect::SwitchesMenu
            && let Some(parent) = self.hover_card_parent(surface, position, &target, &announcement)
        {
            self.hover_card
                .open_until_dismissed(&parent, target, announcement);
        }
        Task::none()
    }

    fn toggle_menu_of_handle(
        &mut self,
        surface: Id,
        position: Position,
        target: HoverTarget,
        announcement: HoverAnnouncement,
    ) -> Task<Message> {
        let Some(output) = self.surfaces.get(&surface).map(|host| host.output.clone()) else {
            return Task::none();
        };
        self.hover_card
            .set_handle_pointing(&output, &target, PointerPresence::PointedAt);
        if self.hover_card.target() == Some(&target) {
            return self.dismiss_hover_card();
        }
        if let Some(parent) = self.hover_card_parent(surface, position, &target, &announcement) {
            self.hover_card
                .open_until_dismissed(&parent, target, announcement);
        }
        Task::none()
    }

    fn hover_card_parent(
        &self,
        surface: Id,
        position: Position,
        target: &HoverTarget,
        announcement: &HoverAnnouncement,
    ) -> Option<PanelEdge> {
        if matches!(self.pointer_drag, PointerDrag::Dragging(_)) {
            tracing::trace!(target: HOVER_LOG_TARGET, ?target, "dropped: a slot is being dragged");
            return None;
        }
        let Some(host) = self.surfaces.get(&surface) else {
            tracing::trace!(target: HOVER_LOG_TARGET, ?target, ?surface, "dropped: unknown surface");
            return None;
        };
        let substance = self
            .hover_content(target)
            .map(|content| content.card_substance(announcement));
        if substance != Some(CardSubstance::HasContent) {
            tracing::trace!(target: HOVER_LOG_TARGET, ?target, ?substance, "dropped: no card content");
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
        let content = self
            .hover_card
            .target()
            .and_then(|target| self.hover_content(target))?;
        self.hover_card.departure(content)
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

    pub(crate) fn hovered_reading_target(&self) -> Option<&HoverTarget> {
        let open_target = self.hover_card.target();
        let inspected_in_menu = match open_target {
            Some(HoverTarget::GroupMenu(kind)) => {
                self.widget_groups.group_menus.inspected_target(*kind)
            }
            _ => None,
        };
        inspected_in_menu.or(open_target)
    }

    pub fn refresh_hovered_reading(&self) -> Task<Message> {
        let Some(target) = self.hovered_reading_target() else {
            return Task::none();
        };
        self.widget_groups
            .refresh_reading(target, &self.config, self.desktop_buses.as_ref())
    }

    pub fn retarget_hover_readings(&mut self) -> Task<Message> {
        let hovered_target = self.hovered_reading_target().cloned();
        let hovered = hovered_target.as_ref();
        let buses = self.desktop_buses.as_ref();
        Task::batch([
            self.widget_groups
                .window_group
                .hover(&self.compositor, hovered, &self.config, self.clock)
                .map(Message::WindowGroup),
            self.widget_groups
                .gpu_group
                .hover(hovered, &self.config, self.clock)
                .map(Message::Gpu),
            self.desktop_buses
                .as_ref()
                .map_or_else(Task::none, |buses| {
                    self.widget_groups
                        .wifi_adapters_group
                        .hover(hovered, &self.config, buses, self.clock)
                        .map(Message::WifiAdapters)
                }),
            self.desktop_buses
                .as_ref()
                .map_or_else(Task::none, |buses| {
                    self.widget_groups
                        .ethernet_ports_group
                        .hover(hovered, &self.config, buses, self.clock)
                        .map(Message::EthernetPorts)
                }),
            self.widget_groups
                .tailnet_group
                .hover(hovered, &self.config, buses, self.clock)
                .map(Message::Tailnet),
            self.widget_groups
                .wireguard_tunnels_group
                .hover(hovered, &self.config, buses, self.clock)
                .map(Message::WireguardTunnels),
            self.widget_groups
                .compute_group
                .hover(hovered, &self.config, self.clock)
                .map(Message::Compute),
            self.widget_groups
                .memory_board_group
                .hover(hovered, &self.config, self.clock)
                .map(Message::MemoryBoard),
            self.widget_groups
                .power_group
                .hover(hovered, &self.config, buses, self.clock)
                .map(Message::Power),
            self.widget_groups
                .claude_usage_group
                .hover(hovered, &self.config, buses, self.clock)
                .map(Message::ClaudeUsage),
            self.widget_groups
                .login_session_group
                .hover(hovered, &self.config, buses, self.clock)
                .map(Message::LoginSession),
            self.widget_groups
                .llama_server_group
                .hover(hovered, &self.config, self.clock)
                .map(Message::LlamaServer),
            self.widget_groups
                .voice_dictation_group
                .hover(hovered, &self.config, self.clock)
                .map(Message::VoiceDictation),
        ])
    }
}
