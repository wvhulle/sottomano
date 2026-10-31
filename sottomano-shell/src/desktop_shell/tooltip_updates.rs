use std::time::Instant;

use iced::Task;
use sottomano_widgets::{
    hover_tooltips::{HoverTarget, TooltipMessage, tooltip_transition::Departed},
    layer_surfaces::PanelEdge,
    widget_groups::{
        bluetooth_adapters_group::BluetoothTarget,
        window_group::{WindowGroup, WindowTarget, quick_launch::QuickLaunch},
        workspaces_group::workspace_tooltip,
    },
    widgets::tooltip_body::{
        TooltipBody,
        tooltip_source_delivery::{DeliveryTiming, Tooltip},
    },
};
use tokio::time::sleep_until;

use crate::{
    desktop_shell::{DesktopShell, focused_app_id},
    shell_message::Message,
};

fn hide_tooltip_at(due: Instant) -> Task<Message> {
    Task::future(async move {
        sleep_until(due.into()).await;
        Message::TooltipHideElapsed
    })
}

impl DesktopShell {
    pub fn update_tooltip(&mut self, msg: TooltipMessage) -> Task<Message> {
        match msg {
            TooltipMessage::Enter {
                surface,
                position,
                target,
                anchor,
                announcement,
            } => {
                if self.is_dragging() {
                    return Task::none();
                }
                let Some(host) = self.surfaces.get(&surface).filter(|_| {
                    self.hover_content(&target)
                        .is_some_and(|content| content.shows_anything(announcement))
                }) else {
                    return Task::none();
                };
                let parent = PanelEdge {
                    output: host.output.clone(),
                    position,
                };
                if let Some(intent_elapsed) =
                    self.tooltip
                        .defer_switch(&parent, &target, anchor, announcement)
                {
                    return intent_elapsed.map(Message::Tooltip);
                }
                self.tooltip.open(&parent, target, anchor, announcement);
                Task::none()
            }
            TooltipMessage::SwitchIntentElapsed => {
                if let Some(pending) = self.tooltip.take_pending_switch() {
                    self.tooltip.open(
                        &pending.parent,
                        pending.target,
                        pending.anchor,
                        pending.announcement,
                    );
                }
                Task::none()
            }
            other => self
                .tooltip
                .update(other, self.clock.instant)
                .map_or_else(Task::none, hide_tooltip_at),
        }
    }

    pub fn leave_tooltip(&mut self) -> Task<Message> {
        hide_tooltip_at(self.tooltip.leave_after_grace(self.clock.instant))
    }

    fn tooltip_departure(&self) -> Option<Departed> {
        let content = self
            .tooltip
            .target()
            .and_then(|target| self.hover_content(target))?;
        self.tooltip.departure(content)
    }

    pub fn dismiss_tooltip(&mut self) -> Task<Message> {
        let departing = self.tooltip_departure();
        self.tooltip.dismiss(departing, self.clock.instant);
        Task::none()
    }

    pub fn leave_tooltip_after_grace(&mut self) -> Task<Message> {
        let departing = self.tooltip_departure();
        self.tooltip.leave_elapsed(self.clock.instant, departing);
        Task::none()
    }

    #[must_use]
    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::PanelSettings(target) => Some(
                self.widget_groups
                    .panel_settings_group
                    .tooltip_for(*target, self.clock),
            ),
            HoverTarget::AppearancePresets(target) => Some(
                self.widget_groups
                    .appearance_presets_menu
                    .tooltip_for(*target),
            ),
            HoverTarget::SidebarSettings(target) => Some(
                self.widget_groups
                    .sidebar_settings_group
                    .tooltip_for(*target, self.clock),
            ),
            HoverTarget::OverlaySettings(target) => Some(
                self.widget_groups
                    .overlay_settings_group
                    .tooltip_for(*target, self.clock),
            ),
            HoverTarget::Notification(target) => {
                Some(self.widget_groups.notification_group.tooltip_for(
                    *target,
                    self.notification_status(),
                    self.clock,
                ))
            }
            HoverTarget::QuickLaunch(app_id) => Some(QuickLaunch::tooltip_for(app_id)),
            HoverTarget::GroupHandle(_) => Some(Tooltip::sourced(
                TooltipBody::sections([]),
                DeliveryTiming::LivePush,
            )),
            HoverTarget::Session(target) => {
                Some(self.widget_groups.session_menu.tooltip_for(*target))
            }
            HoverTarget::Tray(address) => self.widget_groups.tray_group.tooltip_for(address),
            HoverTarget::Window(WindowTarget::Pick(window)) => {
                WindowGroup::pick_tooltip(&self.niri, *window, self.clock)
            }
            HoverTarget::Window(WindowTarget::Inspect(_)) => {
                Some(self.widget_groups.window_group.inspect_tooltip(self.clock))
            }
            HoverTarget::Window(WindowTarget::Usage(window)) => {
                Some(self.widget_groups.session_focus_group.window_usage_tooltip(
                    *window,
                    &self.config,
                    self.clock,
                ))
            }
            HoverTarget::Workspace(workspace) => workspace_tooltip::tooltip(
                &self.niri_snapshot,
                &self.desktop_entries,
                self.widget_groups.niri_window_group.niri_layout(),
                *workspace,
            ),
            HoverTarget::WorkspaceOrder => Some(Tooltip::empty()),
            source_target => self.source_group_tooltip_for(source_target),
        }
    }

    fn bluetooth_tooltip_for(&self, target: &BluetoothTarget) -> Option<Tooltip> {
        self.widget_groups.bluetooth_adapters_group.tooltip_for(
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
    fn source_group_tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::Compute(target) => self
                .widget_groups
                .compute_group
                .tooltip_for(*target, self.clock),
            HoverTarget::MemoryBoard(target) => self
                .widget_groups
                .memory_board_group
                .tooltip_for(*target, self.clock),
            HoverTarget::Gpu(target) => {
                self.widget_groups.gpu_group.tooltip_for(target, self.clock)
            }
            HoverTarget::Storage(target) => self
                .widget_groups
                .storage_space_group
                .tooltip_for(target, self.clock),
            HoverTarget::Drive(target) => self
                .widget_groups
                .physical_drive_group
                .tooltip_for(target, self.clock),
            HoverTarget::Power(target) => self
                .widget_groups
                .power_group
                .tooltip_for(*target, self.clock),
            HoverTarget::NiriWindow(target) => Some(
                self.widget_groups
                    .niri_window_group
                    .tooltip_for(*target, self.clock),
            ),
            HoverTarget::Systemd(target) => self
                .widget_groups
                .systemd_manager_group
                .tooltip_for(target, self.clock),
            HoverTarget::Journal(target) => self
                .widget_groups
                .journal_events_group
                .tooltip_for(*target, self.clock),
            HoverTarget::PendingUpgrades(target) => self
                .widget_groups
                .pending_upgrades_group
                .tooltip_for(*target, self.clock),
            HoverTarget::NetInterface(target) => self
                .widget_groups
                .net_interfaces_group
                .tooltip_for(*target, self.clock),
            HoverTarget::EthernetPort(target) => self
                .widget_groups
                .ethernet_ports_group
                .tooltip_for(target, self.clock),
            HoverTarget::Bluetooth(target) => self.bluetooth_tooltip_for(target),
            HoverTarget::Audio(kind) => self
                .widget_groups
                .audio_group
                .tooltip_for(*kind, self.clock),
            HoverTarget::Monitor(target) => {
                self.widget_groups
                    .monitor_group
                    .tooltip_for(target, &self.outputs, self.clock)
            }
            HoverTarget::ColorTemperature(target) => self
                .widget_groups
                .color_temperature_group
                .tooltip_for(*target, self.clock),
            HoverTarget::DarkMode(target) => self
                .widget_groups
                .dark_mode_group
                .tooltip_for(*target, self.clock),
            HoverTarget::VoiceDictation(target) => self
                .widget_groups
                .voice_dictation_group
                .tooltip_for(*target, self.clock),
            HoverTarget::IdleTimeout(kind) => Some(
                self.widget_groups
                    .idle_timeouts_group
                    .tooltip_for(*kind, self.clock),
            ),
            HoverTarget::Reachability(target) => self
                .widget_groups
                .reachability_group
                .tooltip_for(target, self.clock),
            HoverTarget::Vpn(target) => {
                self.widget_groups.vpn_group.tooltip_for(target, self.clock)
            }
            HoverTarget::Weather(target) => {
                self.widget_groups
                    .weather_group
                    .tooltip_for(*target, &self.config, self.clock)
            }
            HoverTarget::Wallpaper(target) => self
                .widget_groups
                .wallpaper_group
                .tooltip_for(*target, self.clock),
            HoverTarget::Caldav(target) => self
                .widget_groups
                .caldav_group
                .tooltip_for(*target, self.clock),
            HoverTarget::Media(target) => self.widget_groups.media_group.tooltip_for(
                target,
                &self.desktop_entries,
                self.clock,
            ),
            HoverTarget::Gamepad(target) => self.widget_groups.gamepad_group.tooltip_for(
                *target,
                &self.config.widget_groups.gamepad,
                focused_app_id(&self.niri),
                self.clock,
            ),
            HoverTarget::ClaudeUsage(target) => self
                .widget_groups
                .claude_usage_group
                .tooltip_for(*target, self.clock),
            HoverTarget::CopilotUsage(target) => self
                .widget_groups
                .copilot_usage_group
                .tooltip_for(*target, self.clock),
            HoverTarget::OpenRouterCredit(target) => self
                .widget_groups
                .openrouter_credit_group
                .tooltip_for(*target, self.clock),
            HoverTarget::LlamaServer(target) => self
                .widget_groups
                .llama_server_group
                .tooltip_for(target, self.clock),
            HoverTarget::SessionFocus(target) => {
                Some(self.widget_groups.session_focus_group.tooltip_for(
                    *target,
                    &self.config,
                    self.clock,
                ))
            }
            HoverTarget::LoginSession(target) => self
                .widget_groups
                .login_session_group
                .tooltip_for(*target, self.clock),
            HoverTarget::Clock(target) => self
                .widget_groups
                .clock_group
                .tooltip_for(target, self.clock),
            HoverTarget::QuietSchedule(target) => self
                .widget_groups
                .quiet_schedule_group
                .tooltip_for(*target, self.config.calendar.locale, self.clock),
            HoverTarget::PanelSettings(_)
            | HoverTarget::SidebarSettings(_)
            | HoverTarget::AppearancePresets(_)
            | HoverTarget::OverlaySettings(_)
            | HoverTarget::Notification(_)
            | HoverTarget::QuickLaunch(_)
            | HoverTarget::GroupHandle(_)
            | HoverTarget::Session(_)
            | HoverTarget::Tray(_)
            | HoverTarget::Window(_)
            | HoverTarget::Workspace(_)
            | HoverTarget::WorkspaceOrder => None,
        }
    }

    pub fn refresh_hovered_reading(&self) -> Task<Message> {
        let Some(target) = self.tooltip.target() else {
            return Task::none();
        };
        self.widget_groups
            .refresh_reading(target, &self.config, self.desktop_buses.as_ref())
    }

    pub fn retarget_hover_readings(&mut self) -> Task<Message> {
        let hovered = self.tooltip.target();
        let buses = self.desktop_buses.as_ref();
        Task::batch([
            self.widget_groups
                .window_group
                .hover(&self.niri, hovered, &self.config, self.clock)
                .map(Message::WindowGroup),
            self.widget_groups
                .gpu_group
                .hover(hovered, &self.config, self.clock)
                .map(Message::Gpu),
            self.desktop_buses
                .as_ref()
                .map_or_else(Task::none, |buses| {
                    self.widget_groups
                        .net_interfaces_group
                        .hover(hovered, &self.config, buses, self.clock)
                        .map(Message::NetInterfaces)
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
                .vpn_group
                .hover(hovered, &self.config, buses, self.clock)
                .map(Message::Vpn),
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
