use iced::{Rectangle, Task};

use crate::{
    desktop_shell::{
        DesktopShell, Message,
        hover_tooltips::{HoverTarget, PressHints, TooltipMessage, tooltip_transition::Departed},
        monitor_surfaces::PanelEdge,
    },
    widget_groups::workspaces_group::workspace_tooltip,
    widgets::tooltip_body::Tooltip,
};

impl DesktopShell {
    pub(crate) fn update_tooltip(&mut self, msg: TooltipMessage) -> Task<Message> {
        match msg {
            TooltipMessage::Enter {
                surface,
                position,
                target,
                anchor,
                press_hints,
            } => {
                let content = self.tooltip_for(&target);
                let Some(host) = self.surfaces.get(&surface).filter(|_| !content.is_empty()) else {
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
                self.open_tooltip(&parent, target, anchor, press_hints)
            }
            TooltipMessage::SwitchIntentElapsed { seq } => {
                let Some(pending) = self.tooltip.take_pending_switch(seq) else {
                    return Task::none();
                };
                self.open_tooltip(
                    &pending.parent,
                    pending.target,
                    pending.anchor,
                    pending.press_hints,
                )
            }
            other => self.tooltip.update(other),
        }
    }

    fn open_tooltip(
        &mut self,
        parent: &PanelEdge,
        target: HoverTarget,
        anchor: Rectangle,
        press_hints: PressHints,
    ) -> Task<Message> {
        self.tooltip.open(parent, target, anchor, press_hints);
        Task::none()
    }

    fn tooltip_departure(&self) -> Option<Departed> {
        let content = self
            .tooltip
            .target()
            .map(|target| self.tooltip_for(target))?;
        self.tooltip.departure(content)
    }

    pub(crate) fn dismiss_tooltip(&mut self) -> Task<Message> {
        let departing = self.tooltip_departure();
        self.tooltip.dismiss(departing)
    }

    pub(crate) fn leave_tooltip_after_grace(&mut self) -> Task<Message> {
        let departing = self.tooltip_departure();
        self.tooltip.leave_elapsed(departing)
    }

    pub(crate) fn tooltip_for(&self, target: &HoverTarget) -> Tooltip {
        let tooltip = match target {
            HoverTarget::Compute(target) => self.compute_group.tooltip_for(*target),
            HoverTarget::Gpu(target) => self.gpu_group.tooltip_for(target),
            HoverTarget::Disk(target) => self.disk_group.tooltip_for(*target),
            HoverTarget::Power(target) => self.power_group.tooltip_for(*target),
            HoverTarget::PanelAppearance(target) => {
                Some(self.panel_appearance_group.tooltip_for(*target))
            }
            HoverTarget::SurfaceEffects(target) => Some(self.surface_effects_group.tooltip_for(
                *target,
                self.shell_settings.shadow,
                self.niri_rules.styling(),
            )),
            HoverTarget::NiriWindow(target) => Some(self.niri_window_group.tooltip_for(*target)),
            HoverTarget::ShellLayout(target) => Some(
                self.shell_layout_group
                    .tooltip_for(*target, &self.shell_settings.layout),
            ),
            HoverTarget::Overlay(target) => Some(
                self.overlay_group
                    .tooltip_for(*target, self.shell_settings.overlays),
            ),
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
            HoverTarget::Media => self.media_group.tooltip(),
            HoverTarget::Ai(target) => self.ai_group.tooltip_for(*target),
            HoverTarget::Clock(target) => self.clock_group.tooltip_for(target),
            HoverTarget::Notification(_) => None,
            HoverTarget::Tray(address) => Some(self.tray_group.tooltip_for(address)),
            HoverTarget::Window(window) => self.window_group.window_tooltip(&self.niri, *window),
            HoverTarget::Workspace(workspace) => workspace_tooltip::tooltip(
                &self.niri_snapshot,
                &self.desktop_entries,
                &self.config.theme.font,
                &self.font_fallbacks,
                *workspace,
            ),
        };
        tooltip.unwrap_or_else(Tooltip::empty)
    }

    pub(crate) fn retarget_hover_readings(&mut self) -> Task<Message> {
        let window = match self.tooltip.target() {
            Some(HoverTarget::Window(window)) => Some(*window),
            _ => None,
        };
        let gpu = match self.tooltip.target() {
            Some(HoverTarget::Gpu(target)) => Some(target),
            _ => None,
        };
        Task::batch([
            self.window_group
                .hover_window(&self.niri, window, &self.config)
                .map(Message::WindowGroup),
            self.gpu_group.hover(gpu, &self.config).map(Message::Gpu),
        ])
    }

    pub fn tooltip_has_relative_time(&self) -> bool {
        self.tooltip
            .target()
            .is_some_and(|target| self.tooltip_for(target).footer.is_some())
    }
}
