use std::time::Instant;

use iced::{Task, window::Id};
use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::{
    hover_cards::{
        HoverAnnouncement, HoverCardMessage, HoverTarget, PointerPresence,
        hover_card_body::{
            HoverCardBody,
            hover_card_source_delivery::{DeliveryTiming, HoverCard},
        },
        hover_card_surfaces::{CardSubstance, HoverEffect, ShownWidgetPresence},
        hover_card_transition::{CardRetention, Departed},
    },
    layer_surfaces::PanelEdge,
    widget_groups::{
        bluetooth_adapters_group::BluetoothTarget,
        window_group::{WindowGroup, WindowTarget},
        workspaces_group::workspace_hover_card,
    },
    widgets::pointer_drag::PointerDrag,
};
use tokio::time::sleep_until;

use crate::{desktop_shell::DesktopShell, shell_message::Message};

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
                    return intent_elapsed.map(Message::HoverCard);
                }
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
                if let Some(pending) = self.hover_card.take_pending_switch() {
                    self.hover_card
                        .open(&pending.parent, pending.target, pending.announcement);
                }
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
            return None;
        }
        let host = self.surfaces.get(&surface).filter(|_| {
            self.hover_content(target).is_some_and(|content| {
                content.card_substance(announcement) == CardSubstance::HasContent
            })
        })?;
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

    #[must_use]
    pub fn hover_card_for(&self, target: &HoverTarget) -> Option<HoverCard> {
        match target {
            HoverTarget::PanelSettings(target) => Some(
                self.widget_groups
                    .panel_settings_group
                    .hover_card_for(*target, self.clock),
            ),
            HoverTarget::AppearancePresets(target) => Some(
                self.widget_groups
                    .appearance_presets_group
                    .hover_card_for(*target),
            ),
            HoverTarget::SidebarSettings(target) => Some(
                self.widget_groups
                    .sidebar_settings_group
                    .hover_card_for(*target, self.clock),
            ),
            HoverTarget::OverlaySettings(target) => Some(
                self.widget_groups
                    .overlay_settings_group
                    .hover_card_for(*target, self.clock),
            ),
            HoverTarget::Notification(target) => {
                Some(self.widget_groups.notification_group.hover_card_for(
                    *target,
                    self.notification_status(),
                    self.clock,
                ))
            }
            HoverTarget::AppLauncher(target) => Some(
                self.widget_groups
                    .app_launcher_group
                    .hover_card_for(target, self.clock),
            ),
            HoverTarget::GroupMenuSettings(target) => Some(
                self.widget_groups
                    .group_menus
                    .settings_hover_card(*target, self.clock),
            ),
            HoverTarget::GroupMenu(_) | HoverTarget::GroupFace(_) => Some(HoverCard::sourced(
                HoverCardBody::sections([]),
                DeliveryTiming::LivePush,
            )),
            HoverTarget::Session(target) => Some(
                self.widget_groups
                    .session_power_group
                    .hover_card_for(*target),
            ),
            HoverTarget::Tray(address) => self.widget_groups.tray_group.hover_card_for(address),
            HoverTarget::GlobalMenu(target) => self
                .widget_groups
                .global_menu_group
                .hover_card_for(target, self.clock),
            HoverTarget::Window(WindowTarget::Pick(window)) => {
                WindowGroup::pick_hover_card(&self.compositor, *window, self.clock)
            }
            HoverTarget::Window(WindowTarget::Inspect(_)) => Some(
                self.widget_groups
                    .window_group
                    .inspect_hover_card(self.clock),
            ),
            HoverTarget::Window(WindowTarget::Usage(window)) => Some(
                self.widget_groups
                    .session_focus_group
                    .window_usage_hover_card(*window, &self.config, self.clock),
            ),
            HoverTarget::Workspace(workspace) => workspace_hover_card::hover_card(
                &self.window_snapshot,
                &self.desktop_entries,
                self.widget_groups
                    .compositor_styling_group
                    .app_window_layout(),
                *workspace,
            ),
            HoverTarget::WorkspaceOrder | HoverTarget::WorkspaceFolding => Some(HoverCard::empty()),
            source_target => self.source_group_hover_card_for(source_target),
        }
    }

    fn bluetooth_hover_card_for(&self, target: &BluetoothTarget) -> Option<HoverCard> {
        self.widget_groups.bluetooth_adapters_group.hover_card_for(
            target,
            &|hardware_address| {
                self.widget_groups
                    .gamepad_group
                    .report_timing_for_address(hardware_address)
            },
            self.clock,
        )
    }

    #[allow(clippy::too_many_lines)]
    fn source_group_hover_card_for(&self, target: &HoverTarget) -> Option<HoverCard> {
        match target {
            HoverTarget::Compute(target) => self
                .widget_groups
                .compute_group
                .hover_card_for(*target, self.clock),
            HoverTarget::MemoryBoard(target) => self
                .widget_groups
                .memory_board_group
                .hover_card_for(target, self.clock),
            HoverTarget::Gpu(target) => self
                .widget_groups
                .gpu_group
                .hover_card_for(target, self.clock),
            HoverTarget::Storage(target) => self
                .widget_groups
                .storage_space_group
                .hover_card_for(target, self.clock),
            HoverTarget::Drive(target) => self
                .widget_groups
                .physical_drive_group
                .hover_card_for(target, self.clock),
            HoverTarget::Power(target) => self
                .widget_groups
                .power_group
                .hover_card_for(*target, self.clock),
            HoverTarget::CompositorStyling(target) => Some(
                self.widget_groups
                    .compositor_styling_group
                    .hover_card_for(*target, self.clock),
            ),
            HoverTarget::Systemd(target) => self
                .widget_groups
                .systemd_manager_group
                .hover_card_for(target, self.clock),
            HoverTarget::Journal(target) => self
                .widget_groups
                .journal_events_group
                .hover_card_for(*target, self.clock),
            HoverTarget::PendingUpgrades(target) => self
                .widget_groups
                .pending_upgrades_group
                .hover_card_for(*target, self.clock),
            HoverTarget::WifiAdapter(target) => self
                .widget_groups
                .wifi_adapters_group
                .hover_card_for(target, self.clock),
            HoverTarget::MobileModem(target) => self
                .widget_groups
                .mobile_modems_group
                .hover_card_for(target, self.clock),
            HoverTarget::EthernetPort(target) => self
                .widget_groups
                .ethernet_ports_group
                .hover_card_for(target, self.clock),
            HoverTarget::Bluetooth(target) => self.bluetooth_hover_card_for(target),
            HoverTarget::Audio(kind) => self
                .widget_groups
                .audio_group
                .hover_card_for(*kind, self.clock),
            HoverTarget::Monitor(target) => {
                self.widget_groups
                    .monitor_group
                    .hover_card_for(target, &self.outputs, self.clock)
            }
            HoverTarget::ColorTemperature(target) => self
                .widget_groups
                .color_temperature_group
                .hover_card_for(*target, self.clock),
            HoverTarget::DarkMode(target) => self
                .widget_groups
                .dark_mode_group
                .hover_card_for(*target, self.clock),
            HoverTarget::VoiceDictation(target) => self
                .widget_groups
                .voice_dictation_group
                .hover_card_for(*target, self.clock),
            HoverTarget::IdleTimeout(kind) => Some(
                self.widget_groups
                    .idle_pause_groups
                    .hover_card_for(*kind, self.clock),
            ),
            HoverTarget::PauseBlocker(target) => self
                .widget_groups
                .idle_pause_groups
                .blocker_hover_card_for(target),
            HoverTarget::Reachability(target) => self
                .widget_groups
                .reachability_group
                .hover_card_for(target, self.clock),
            HoverTarget::Tailnet(target) => self
                .widget_groups
                .tailnet_group
                .hover_card_for(target, self.clock),
            HoverTarget::WireguardTunnel(target) => self
                .widget_groups
                .wireguard_tunnels_group
                .hover_card_for(target, self.clock),
            HoverTarget::NetworkManagerVpn(target) => self
                .widget_groups
                .network_manager_vpn_group
                .hover_card_for(target, self.clock),
            HoverTarget::Weather(target) => {
                self.widget_groups
                    .weather_group
                    .hover_card_for(*target, &self.config, self.clock)
            }
            HoverTarget::Wallpaper(target) => self
                .widget_groups
                .wallpaper_group
                .hover_card_for(*target, self.clock),
            HoverTarget::Caldav(target) => self.widget_groups.caldav_group.hover_card_for(
                *target,
                &self.config.widget_groups.caldav,
                self.clock,
            ),
            HoverTarget::Media(target) => self.widget_groups.media_group.hover_card_for(
                target,
                &self.desktop_entries,
                self.clock,
            ),
            HoverTarget::Gamepad(target) => self.widget_groups.gamepad_group.hover_card_for(
                *target,
                &self.config.widget_groups.gamepad,
                self.clock,
            ),
            HoverTarget::ClaudeUsage(target) => self
                .widget_groups
                .claude_usage_group
                .hover_card_for(*target, self.clock),
            HoverTarget::CopilotUsage(target) => self
                .widget_groups
                .copilot_usage_group
                .hover_card_for(*target, self.clock),
            HoverTarget::OpenRouterCredit(target) => self
                .widget_groups
                .openrouter_credit_group
                .hover_card_for(*target, self.clock),
            HoverTarget::LlamaServer(target) => self
                .widget_groups
                .llama_server_group
                .hover_card_for(target, self.clock),
            HoverTarget::PodmanEngine(target) => self
                .widget_groups
                .podman_engines_group
                .hover_card_for(target, self.clock),
            HoverTarget::VirtualMachine(target) => self
                .widget_groups
                .virtual_machines_group
                .hover_card_for(target, self.clock),
            HoverTarget::NspawnContainer(target) => self
                .widget_groups
                .nspawn_containers_group
                .hover_card_for(target, self.clock),
            HoverTarget::CiRuns(target) => self
                .widget_groups
                .ci_runs_group
                .hover_card_for(target, self.clock),
            HoverTarget::SessionFocus(target) => {
                Some(self.widget_groups.session_focus_group.hover_card_for(
                    *target,
                    &self.config,
                    self.clock,
                ))
            }
            HoverTarget::LoginSession(target) => self
                .widget_groups
                .login_session_group
                .hover_card_for(*target, self.clock),
            HoverTarget::Clock(target) => self
                .widget_groups
                .clock_group
                .hover_card_for(target, self.clock),
            HoverTarget::QuietSchedule(target) => self
                .widget_groups
                .quiet_schedule_group
                .hover_card_for(*target, self.config.calendar.locale, self.clock),
            HoverTarget::PanelSettings(_)
            | HoverTarget::SidebarSettings(_)
            | HoverTarget::AppearancePresets(_)
            | HoverTarget::OverlaySettings(_)
            | HoverTarget::Notification(_)
            | HoverTarget::AppLauncher(_)
            | HoverTarget::GroupMenu(_)
            | HoverTarget::GroupFace(_)
            | HoverTarget::GroupMenuSettings(_)
            | HoverTarget::Session(_)
            | HoverTarget::Tray(_)
            | HoverTarget::GlobalMenu(_)
            | HoverTarget::Window(_)
            | HoverTarget::Workspace(_)
            | HoverTarget::WorkspaceOrder
            | HoverTarget::WorkspaceFolding => None,
        }
    }

    pub(super) fn hovered_reading_target(&self) -> Option<&HoverTarget> {
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
