use iced::{Rectangle, Task};

use crate::{
    data::source::process::{self, Watched},
    desktop_shell::{
        DesktopShell, Message,
        hover_tooltips::{HoverTarget, TooltipMessage, tooltip_transition::Departed},
        monitor_surfaces::PanelEdge,
    },
    widget_groups::{
        window_group::{WindowGroupMessage, window_list::WindowListMessage},
        workspaces_group::workspace_tooltip,
    },
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
            } => {
                let content = self.tooltip_for(&target);
                let Some(host) = self.surfaces.get(&surface).filter(|_| !content.is_empty()) else {
                    return Task::none();
                };
                let parent = PanelEdge {
                    output: host.output.clone(),
                    position,
                };
                if let Some(intent) = self.tooltip.defer_switch(&parent, &target, anchor) {
                    return intent;
                }
                self.open_tooltip(&parent, target, anchor)
            }
            TooltipMessage::SwitchIntentElapsed { seq } => {
                let Some(pending) = self.tooltip.take_pending_switch(seq) else {
                    return Task::none();
                };
                self.open_tooltip(&pending.parent, pending.target, pending.anchor)
            }
            other => self.tooltip.update(other),
        }
    }

    fn open_tooltip(
        &mut self,
        parent: &PanelEdge,
        target: HoverTarget,
        anchor: Rectangle,
    ) -> Task<Message> {
        self.tooltip.open(parent, target, anchor);
        let Some(watched) = self.watched_window() else {
            return Task::none();
        };
        Task::future(process::inspected(watched))
            .and_then(Task::done)
            .map(|inspection| {
                Message::WindowGroup(WindowGroupMessage::WindowList(
                    WindowListMessage::ProcessInspected(inspection),
                ))
            })
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
        if let HoverTarget::Tray(address) = target {
            return self.tray_group.tooltip_for(address);
        }
        if let HoverTarget::Window(window) = target {
            return self
                .window_group
                .window_tooltip(&self.niri, *window)
                .unwrap_or_else(Tooltip::empty);
        }
        if let HoverTarget::Workspace(workspace) = target {
            return workspace_tooltip::tooltip(
                &self.niri_snapshot,
                &self.desktop_entries,
                &self.config.font,
                &self.font_fallbacks,
                *workspace,
            )
            .unwrap_or_else(Tooltip::empty);
        }
        self.compute_group
            .tooltip_for(target)
            .or_else(|| self.disk_group.tooltip_for(target))
            .or_else(|| self.battery_group.tooltip_for(target))
            .or_else(|| self.clock_group.tooltip_for(target))
            .or_else(|| self.systemd_manager_group.tooltip_for(target))
            .or_else(|| self.journal_events_group.tooltip_for(target))
            .or_else(|| self.pending_upgrades_group.tooltip_for(target))
            .or_else(|| self.net_interfaces_group.tooltip_for(target))
            .or_else(|| self.audio_group.tooltip_for(target))
            .or_else(|| self.monitor_group.tooltip_for(target, &self.outputs))
            .or_else(|| self.color_temperature_group.tooltip_for(target))
            .or_else(|| self.dark_mode_group.tooltip_for(target))
            .or_else(|| self.idle_timeouts_group.tooltip_for(target))
            .or_else(|| self.internet_group.tooltip_for(target))
            .or_else(|| self.weather_group.tooltip_for(target))
            .or_else(|| self.wallpaper_group.tooltip_for(target))
            .or_else(|| self.media_group.tooltip_for(target))
            .or_else(|| self.ai_group.tooltip_for(target))
            .or_else(|| {
                self.shell_layout_group
                    .tooltip_for(target, &self.shell_settings.layout)
            })
            .or_else(|| self.panel_appearance_group.tooltip_for(target))
            .or_else(|| {
                self.surface_effects_group.tooltip_for(
                    target,
                    self.shell_settings.shadow,
                    self.niri_rules.styling(),
                )
            })
            .or_else(|| {
                self.overlay_group
                    .tooltip_for(target, self.shell_settings.overlays)
            })
            .or_else(|| self.niri_window_group.tooltip_for(target))
            .unwrap_or_else(Tooltip::empty)
    }

    pub fn watched_window(&self) -> Option<Watched> {
        let Some(HoverTarget::Window(window)) = self.tooltip.target() else {
            return None;
        };
        let window = self.niri.windows.windows.get(window)?;
        Some(Watched {
            pid: window.pid?,
            app_id: window.app_id.clone(),
        })
    }

    pub fn tooltip_has_relative_time(&self) -> bool {
        self.tooltip
            .target()
            .is_some_and(|target| self.tooltip_for(target).footer.is_some())
    }
}
