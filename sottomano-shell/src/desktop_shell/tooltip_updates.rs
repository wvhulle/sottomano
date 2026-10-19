use std::time::Instant;

use iced::Task;
use sottomano_sources::system_protocol_clients::dbus_desktop_buses::DesktopBuses;
use sottomano_widgets::{
    hover_tooltips::{
        HoverTarget, TooltipMessage, tooltip_surfaces::DeferredSwitch, tooltip_transition::Departed,
    },
    layer_surfaces::PanelEdge,
    shell_message::Message,
    widget_groups::{
        clock_group::ClockGroup,
        color_temperature_group::ColorTemperatureGroup,
        compute_group::ComputeGroup,
        dark_mode_group::DarkModeGroup,
        disk_group::DiskGroup,
        internet_group::InternetGroup,
        login_session_group::LoginSessionGroup,
        memory_board_group::MemoryBoardGroup,
        monitor_group::MonitorGroup,
        notification_group::NotificationGroup,
        power_group::PowerGroup,
        window_group::{WindowGroup, WindowTarget, quick_launch::QuickLaunch},
        workspaces_group::workspace_tooltip,
    },
    widgets::tooltip_body::{Freshness, Tooltip, TooltipBody},
};
use tokio::time::sleep_until;

use crate::desktop_shell::DesktopShell;

fn deliver_at(due: Instant, message: Message) -> Task<Message> {
    Task::future(async move {
        sleep_until(due.into()).await;
        message
    })
}

fn hide_tooltip_at(due: Instant) -> Task<Message> {
    deliver_at(due, Message::TooltipHideElapsed)
}

impl DesktopShell {
    pub fn update_tooltip(&mut self, msg: TooltipMessage) -> Task<Message> {
        match msg {
            TooltipMessage::Enter {
                surface,
                position,
                target,
                anchor,
                press_hints,
            } => {
                if self.is_dragging() {
                    return Task::none();
                }
                let Some(host) = self.surfaces.get(&surface).filter(|_| {
                    self.tooltip_for(&target)
                        .is_some_and(|tooltip| !tooltip.is_empty() || !press_hints.is_empty())
                }) else {
                    return Task::none();
                };
                let parent = PanelEdge {
                    output: host.output.clone(),
                    position,
                };
                if let Some(DeferredSwitch { seq, after }) =
                    self.tooltip
                        .defer_switch(&parent, &target, anchor, press_hints)
                {
                    return deliver_at(
                        self.clock.instant + after,
                        Message::Tooltip(TooltipMessage::SwitchIntentElapsed { seq }),
                    );
                }
                self.tooltip.open(&parent, target, anchor, press_hints);
                Task::none()
            }
            TooltipMessage::SwitchIntentElapsed { seq } => {
                if let Some(pending) = self.tooltip.take_pending_switch(seq) {
                    self.tooltip.open(
                        &pending.parent,
                        pending.target,
                        pending.anchor,
                        pending.press_hints,
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
            .and_then(|target| self.tooltip_for(target))?;
        self.tooltip.departure(content)
    }

    pub fn dismiss_tooltip(&mut self) -> Task<Message> {
        let departing = self.tooltip_departure();
        self.tooltip.dismiss(departing);
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
            HoverTarget::PanelSettings(target) => Some(self.panel_settings_group.tooltip_for(
                *target,
                &self.shell_settings.layout,
                &self.shell_settings.panels,
                self.clock,
            )),
            HoverTarget::SidebarSettings(target) => Some(self.sidebar_settings_group.tooltip_for(
                *target,
                &self.shell_settings.layout,
                &self.shell_settings.sidebar,
                self.clock,
            )),
            HoverTarget::OverlaySettings(target) => Some(self.overlay_settings_group.tooltip_for(
                *target,
                self.shell_settings.notifications,
                &self.shell_settings.overlays,
                self.clock,
            )),
            HoverTarget::Notification(target) => Some(NotificationGroup::tooltip_for(
                *target,
                self.shell_settings.notifications,
                self.notification_status(),
                self.clock,
            )),
            HoverTarget::QuickLaunch(app_id) => Some(QuickLaunch::tooltip_for(app_id)),
            HoverTarget::GroupHandle(_) => {
                Some(Tooltip::sourced(TooltipBody::sections([]), Freshness::Live))
            }
            HoverTarget::Session(target) => Some(
                self.session_menu
                    .tooltip_for(*target, self.idle_timeouts_group.timeouts()),
            ),
            HoverTarget::Tray(address) => self.tray_group.tooltip_for(address),
            HoverTarget::Window(WindowTarget::Pick(window)) => {
                WindowGroup::pick_tooltip(&self.niri, &self.niri_snapshot, *window, self.clock)
            }
            HoverTarget::Window(WindowTarget::Inspect(_)) => {
                Some(self.window_group.inspect_tooltip(self.clock))
            }
            HoverTarget::Window(WindowTarget::Usage(window)) => {
                Some(self.session_focus_group.window_usage_tooltip(
                    *window,
                    &self.config,
                    self.quiet_schedule_group.waking_window(),
                    self.clock,
                ))
            }
            HoverTarget::Workspace(workspace) => {
                workspace_tooltip::tooltip(&self.niri_snapshot, &self.desktop_entries, *workspace)
            }
            HoverTarget::WorkspaceOrder => Some(Tooltip::empty()),
            source_target => self.source_group_tooltip_for(source_target),
        }
    }

    fn source_group_tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::Compute(target) => self.compute_group.tooltip_for(*target, self.clock),
            HoverTarget::MemoryBoard(target) => {
                self.memory_board_group.tooltip_for(*target, self.clock)
            }
            HoverTarget::Gpu(target) => self.gpu_group.tooltip_for(target, self.clock),
            HoverTarget::Disk(target) => self.disk_group.tooltip_for(*target, self.clock),
            HoverTarget::Power(target) => self.power_group.tooltip_for(*target, self.clock),
            HoverTarget::NiriWindow(target) => Some(self.niri_window_group.tooltip_for(
                *target,
                &self.niri_rules,
                self.clock,
            )),
            HoverTarget::Systemd(target) => {
                self.systemd_manager_group.tooltip_for(target, self.clock)
            }
            HoverTarget::Journal(target) => {
                self.journal_events_group.tooltip_for(*target, self.clock)
            }
            HoverTarget::PendingUpgrades(target) => {
                self.pending_upgrades_group.tooltip_for(*target, self.clock)
            }
            HoverTarget::NetInterface(target) => {
                self.net_interfaces_group.tooltip_for(*target, self.clock)
            }
            HoverTarget::Audio(kind) => self.audio_group.tooltip_for(*kind, self.clock),
            HoverTarget::Monitor(target) => {
                self.monitor_group
                    .tooltip_for(target, &self.outputs, self.clock)
            }
            HoverTarget::ColorTemperature(target) => self
                .color_temperature_group
                .tooltip_for(*target, self.clock),
            HoverTarget::DarkMode(target) => self.dark_mode_group.tooltip_for(*target, self.clock),
            HoverTarget::IdleTimeout(kind) => {
                Some(self.idle_timeouts_group.tooltip_for(*kind, self.clock))
            }
            HoverTarget::Internet(target) => self.internet_group.tooltip_for(target, self.clock),
            HoverTarget::Weather(target) => self.weather_group.tooltip_for(*target, self.clock),
            HoverTarget::Wallpaper(target) => self.wallpaper_group.tooltip_for(*target, self.clock),
            HoverTarget::Media(target) => {
                self.media_group
                    .tooltip_for(target, &self.desktop_entries, self.clock)
            }
            HoverTarget::ClaudeUsage(target) => {
                self.claude_usage_group.tooltip_for(*target, self.clock)
            }
            HoverTarget::CopilotUsage(target) => {
                self.copilot_usage_group.tooltip_for(*target, self.clock)
            }
            HoverTarget::OpenRouterCredit(target) => self
                .openrouter_credit_group
                .tooltip_for(*target, self.clock),
            HoverTarget::LlamaServer(target) => {
                self.llama_server_group.tooltip_for(target, self.clock)
            }
            HoverTarget::SessionFocus(target) => Some(self.session_focus_group.tooltip_for(
                *target,
                &self.config,
                self.quiet_schedule_group.waking_window(),
                self.clock,
            )),
            HoverTarget::LoginSession(target) => {
                self.login_session_group.tooltip_for(*target, self.clock)
            }
            HoverTarget::Clock(target) => self.clock_group.tooltip_for(target, self.clock),
            HoverTarget::QuietSchedule(target) => {
                self.quiet_schedule_group.tooltip_for(*target, self.clock)
            }
            HoverTarget::PanelSettings(_)
            | HoverTarget::SidebarSettings(_)
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
        let buses = self.desktop_buses.as_ref();
        let on_buses = |refresh: &dyn Fn(&DesktopBuses) -> Task<Message>| {
            buses.map_or_else(Task::none, refresh)
        };
        let sensors = &self.config.widget_groups.temperature_sensors;
        match target {
            HoverTarget::Weather(target) => {
                self.weather_group.refresh(*target).map(Message::Weather)
            }
            HoverTarget::Compute(target) => {
                ComputeGroup::refresh(*target, &sensors.cpu).map(Message::Compute)
            }
            HoverTarget::MemoryBoard(target) => {
                MemoryBoardGroup::refresh(*target, buses).map(Message::MemoryBoard)
            }
            HoverTarget::Gpu(target) => self.gpu_group.refresh(target).map(Message::Gpu),
            HoverTarget::Disk(target) => {
                DiskGroup::refresh(*target, &sensors.drive).map(Message::Disk)
            }
            HoverTarget::Clock(target) => ClockGroup::refresh(target).map(Message::Clock),
            HoverTarget::DarkMode(target) => DarkModeGroup::refresh(*target).map(Message::DarkMode),
            HoverTarget::ClaudeUsage(target) => self
                .claude_usage_group
                .refresh(*target, &self.config)
                .map(Message::ClaudeUsage),
            HoverTarget::CopilotUsage(target) => self
                .copilot_usage_group
                .refresh(*target, &self.config, buses)
                .map(Message::CopilotUsage),
            HoverTarget::OpenRouterCredit(target) => self
                .openrouter_credit_group
                .refresh(*target, &self.config)
                .map(Message::OpenRouterCredit),
            HoverTarget::Power(target) => {
                on_buses(&|buses| PowerGroup::refresh(*target, buses).map(Message::Power))
            }
            HoverTarget::LoginSession(target) => on_buses(&|buses| {
                LoginSessionGroup::refresh(*target, buses).map(Message::LoginSession)
            }),
            HoverTarget::Internet(target) => {
                on_buses(&|buses| InternetGroup::refresh(target, buses).map(Message::Internet))
            }
            HoverTarget::Monitor(target) => {
                on_buses(&|buses| MonitorGroup::refresh(target, buses).map(Message::Monitor))
            }
            HoverTarget::ColorTemperature(target) => on_buses(&|buses| {
                ColorTemperatureGroup::refresh(*target, buses).map(Message::ColorTemperature)
            }),
            HoverTarget::PanelSettings(_)
            | HoverTarget::SidebarSettings(_)
            | HoverTarget::OverlaySettings(_)
            | HoverTarget::NiriWindow(_)
            | HoverTarget::Systemd(_)
            | HoverTarget::Journal(_)
            | HoverTarget::PendingUpgrades(_)
            | HoverTarget::NetInterface(_)
            | HoverTarget::Audio(_)
            | HoverTarget::IdleTimeout(_)
            | HoverTarget::Wallpaper(_)
            | HoverTarget::Media(_)
            | HoverTarget::LlamaServer(_)
            | HoverTarget::SessionFocus(_)
            | HoverTarget::QuickLaunch(_)
            | HoverTarget::GroupHandle(_)
            | HoverTarget::Notification(_)
            | HoverTarget::QuietSchedule(_)
            | HoverTarget::Session(_)
            | HoverTarget::Tray(_)
            | HoverTarget::Window(_)
            | HoverTarget::Workspace(_)
            | HoverTarget::WorkspaceOrder => Task::none(),
        }
    }

    pub fn retarget_hover_readings(&mut self) -> Task<Message> {
        let hovered = self.tooltip.target();
        let buses = self.desktop_buses.as_ref();
        Task::batch([
            self.window_group
                .hover(&self.niri, hovered, &self.config, self.clock)
                .map(Message::WindowGroup),
            self.gpu_group
                .hover(hovered, &self.config, self.clock)
                .map(Message::Gpu),
            self.desktop_buses
                .as_ref()
                .map_or_else(Task::none, |buses| {
                    self.net_interfaces_group
                        .hover(hovered, &self.config, buses, self.clock)
                        .map(Message::NetInterfaces)
                }),
            self.internet_group
                .hover(hovered, &self.config, buses, self.clock)
                .map(Message::Internet),
            self.compute_group
                .hover(hovered, &self.config, self.clock)
                .map(Message::Compute),
            self.memory_board_group
                .hover(hovered, &self.config, self.clock)
                .map(Message::MemoryBoard),
            self.power_group
                .hover(hovered, &self.config, buses, self.clock)
                .map(Message::Power),
            self.claude_usage_group
                .hover(hovered, &self.config, self.clock)
                .map(Message::ClaudeUsage),
            self.desktop_buses
                .as_ref()
                .map_or_else(Task::none, |buses| {
                    self.login_session_group
                        .hover(hovered, &self.config, buses, self.clock)
                        .map(Message::LoginSession)
                }),
            self.llama_server_group
                .hover(hovered, &self.config, self.clock)
                .map(Message::LlamaServer),
        ])
    }
}
