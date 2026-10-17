use iced::Task;

use crate::{
    desktop_shell::{
        DesktopShell, Message,
        hover_tooltips::{HoverTarget, TooltipMessage, tooltip_transition::Departed},
        monitor_surfaces::PanelEdge,
    },
    widget_groups::{
        notification_group::NotificationGroup,
        window_group::{WindowGroup, WindowTarget, quick_launch::QuickLaunch},
        workspaces_group::workspace_tooltip,
    },
    widgets::tooltip_body::{Freshness, Tooltip, TooltipBody},
};

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
                    self.tooltip_for(&target).is_some_and(|tooltip| {
                        !tooltip.is_empty() || !press_hints.gestures().is_empty()
                    })
                }) else {
                    return Task::none();
                };
                let parent = PanelEdge {
                    output: host.output.clone(),
                    position,
                };
                if let Some(intent) =
                    self.tooltip
                        .defer_switch(&parent, &target, anchor, press_hints)
                {
                    return intent;
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
            other => self.tooltip.update(other),
        }
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
        self.tooltip.leave_elapsed(departing);
        Task::none()
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::Compute(target) => self.compute_group.tooltip_for(*target),
            HoverTarget::Gpu(target) => self.gpu_group.tooltip_for(target),
            HoverTarget::Disk(target) => self.disk_group.tooltip_for(*target),
            HoverTarget::Power(target) => self.power_group.tooltip_for(*target),
            HoverTarget::PanelSettings(target) => Some(self.panel_settings_group.tooltip_for(
                *target,
                &self.shell_settings.layout,
                &self.shell_settings.panels,
                self.niri_rules.styling(),
            )),
            HoverTarget::SidebarSettings(target) => Some(self.sidebar_settings_group.tooltip_for(
                *target,
                &self.shell_settings.layout,
                &self.shell_settings.sidebar,
            )),
            HoverTarget::OverlaySettings(target) => Some(self.overlay_settings_group.tooltip_for(
                *target,
                self.shell_settings.notifications,
                &self.shell_settings.overlays,
            )),
            HoverTarget::NiriWindow(target) => Some(self.niri_window_group.tooltip_for(*target)),
            HoverTarget::Systemd(target) => self.systemd_manager_group.tooltip_for(target),
            HoverTarget::Journal(target) => self.journal_events_group.tooltip_for(*target),
            HoverTarget::PendingUpgrades(target) => {
                self.pending_upgrades_group.tooltip_for(*target)
            }
            HoverTarget::NetInterface(target) => self.net_interfaces_group.tooltip_for(*target),
            HoverTarget::Audio(kind) => self.audio_group.tooltip_for(*kind),
            HoverTarget::Monitor(target) => self.monitor_group.tooltip_for(target, &self.outputs),
            HoverTarget::ColorTemperature(target) => {
                self.color_temperature_group.tooltip_for(*target)
            }
            HoverTarget::DarkMode(target) => self.dark_mode_group.tooltip_for(*target),
            HoverTarget::IdleTimeout(kind) => Some(self.idle_timeouts_group.tooltip_for(*kind)),
            HoverTarget::Internet(target) => self.internet_group.tooltip_for(target),
            HoverTarget::Weather(target) => self.weather_group.tooltip_for(*target),
            HoverTarget::Wallpaper(target) => self.wallpaper_group.tooltip_for(*target),
            HoverTarget::Media(target) => {
                self.media_group.tooltip_for(target, &self.desktop_entries)
            }
            HoverTarget::Ai(target) => self.ai_group.tooltip_for(*target),
            HoverTarget::SessionFocus(target) => Some(self.session_focus_group.tooltip_for(
                *target,
                &self.config,
                self.quiet_schedule_group.waking_window(),
            )),
            HoverTarget::LoginSession(target) => self.login_session_group.tooltip_for(*target),
            HoverTarget::Clock(target) => self.clock_group.tooltip_for(target),
            HoverTarget::QuickLaunch(app_id) => Some(QuickLaunch::tooltip_for(app_id)),
            HoverTarget::GroupHandle(_) => {
                Some(Tooltip::sourced(TooltipBody::sections([]), Freshness::Live))
            }
            HoverTarget::Notification(target) => Some(NotificationGroup::tooltip_for(
                *target,
                self.shell_settings.notifications,
                self.notification_status(),
            )),
            HoverTarget::QuietSchedule(target) => self.quiet_schedule_group.tooltip_for(*target),
            HoverTarget::Session(target) => Some(
                self.session_menu
                    .tooltip_for(*target, self.idle_timeouts_group.timeouts()),
            ),
            HoverTarget::Tray(address) => self.tray_group.tooltip_for(address),
            HoverTarget::Window(WindowTarget::Pick(window)) => {
                WindowGroup::pick_tooltip(&self.niri, &self.niri_snapshot, *window)
            }
            HoverTarget::Window(WindowTarget::Inspect(_)) => {
                Some(self.window_group.inspect_tooltip())
            }
            HoverTarget::Window(WindowTarget::Usage(window)) => {
                Some(self.session_focus_group.window_usage_tooltip(
                    *window,
                    &self.config,
                    self.quiet_schedule_group.waking_window(),
                ))
            }
            HoverTarget::Workspace(workspace) => workspace_tooltip::tooltip(
                &self.niri_snapshot,
                &self.desktop_entries,
                &self.config.theme.font,
                &self.font_fallbacks,
                *workspace,
            ),
            HoverTarget::WorkspaceOrder => Some(Tooltip::empty()),
        }
    }

    pub fn retarget_hover_readings(&mut self) -> Task<Message> {
        let hovered = self.tooltip.target();
        Task::batch([
            self.window_group
                .hover(&self.niri, hovered, &self.config)
                .map(Message::WindowGroup),
            self.gpu_group
                .hover(hovered, &self.config)
                .map(Message::Gpu),
            self.net_interfaces_group
                .hover(hovered, &self.config)
                .map(Message::NetInterfaces),
            self.internet_group
                .hover(hovered, &self.config)
                .map(Message::Internet),
            self.compute_group
                .hover(hovered, &self.config)
                .map(Message::Compute),
            self.power_group
                .hover(hovered, &self.config)
                .map(Message::Power),
            self.ai_group.hover(hovered, &self.config).map(Message::Ai),
            self.login_session_group
                .hover(hovered, &self.config)
                .map(Message::LoginSession),
        ])
    }

    pub fn tooltip_has_polled_data(&self) -> bool {
        self.tooltip
            .target()
            .and_then(|target| self.tooltip_for(target))
            .is_some_and(|tooltip| tooltip.has_polled_data())
    }
}
