use iced::Element;
use sottomano_config::screen_edges::Position;
use sottomano_state::WorkspacesPlacement;
use sottomano_widgets::{
    widget_groups::workspaces_group::inline_strip, widgets::widget_group::GroupContent,
};

use crate::{desktop_shell::DesktopShell, shell_message::Message};

pub(super) struct BottomBarGroups<'a> {
    pub start: [GroupContent<'a, Message>; 9],
    pub end: [GroupContent<'a, Message>; 9],
}

impl DesktopShell {
    pub(super) fn bottom_bar<'a>(&'a self, output: &'a str) -> Element<'a, Message> {
        let config = &self.config;
        let BottomBarGroups { start, end } = self.bottom_bar_groups();
        let view =
            |group: GroupContent<'a, Message>| group.view(config, Position::Bottom, self.clock);
        self.panel_bar(
            Position::Bottom,
            start
                .map(view)
                .into_iter()
                .chain([self.inline_workspaces(output)]),
            [self.window_group_view(output)],
            end.map(view),
        )
    }

    pub(super) fn bottom_bar_groups(&self) -> BottomBarGroups<'_> {
        BottomBarGroups {
            start: self.bottom_bar_start_groups(),
            end: self.bottom_bar_end_groups(),
        }
    }

    fn bottom_bar_start_groups(&self) -> [GroupContent<'_, Message>; 9] {
        let config = &self.config;
        [
            self.panel_settings_group
                .group_content(config)
                .map(Message::PanelSettings),
            self.sidebar_settings_group
                .group_content(config)
                .map(Message::SidebarSettings),
            self.overlay_settings_group
                .group_content(config)
                .map(Message::OverlaySettings),
            self.niri_window_group
                .group_content(config)
                .map(Message::NiriWindow),
            self.monitor_group
                .group_content(config)
                .map(Message::Monitor),
            self.dark_mode_group
                .group_content(config)
                .map(Message::DarkMode),
            self.color_temperature_group
                .group_content(config)
                .map(Message::ColorTemperature),
            self.wallpaper_group.group_content().map(Message::Wallpaper),
            self.idle_timeouts_group
                .group_content(config)
                .map(Message::IdleTimeouts),
        ]
    }

    fn bottom_bar_end_groups(&self) -> [GroupContent<'_, Message>; 9] {
        let config = &self.config;
        [
            self.llama_server_group
                .group_content()
                .map(Message::LlamaServer),
            self.claude_usage_group
                .group_content()
                .map(Message::ClaudeUsage),
            self.copilot_usage_group
                .group_content()
                .map(Message::CopilotUsage),
            self.openrouter_credit_group
                .group_content()
                .map(Message::OpenRouterCredit),
            self.systemd_manager_group
                .group_content(config, self.clock)
                .map(Message::SystemdManager),
            self.journal_events_group
                .group_content(config)
                .map(Message::JournalEvents),
            self.pending_upgrades_group
                .group_content(config)
                .map(Message::PendingUpgrades),
            self.login_session_group
                .group_content()
                .map(Message::LoginSession),
            self.tray_group.group_content(config).map(Message::Tray),
        ]
    }

    fn inline_workspaces<'a>(&'a self, output: &'a str) -> Option<Element<'a, Message>> {
        match self.sidebar_settings_group.workspaces_placement() {
            WorkspacesPlacement::Sidebar => None,
            WorkspacesPlacement::BottomPanel => inline_strip::inline_view(
                self.niri_snapshot.get()?,
                output,
                &self.config,
                Position::Bottom,
                &self.drag_view(),
                self.configured_workspaces.as_ref(),
                self.clock,
            )
            .map(|workspaces| workspaces.map(Message::from)),
        }
    }

    fn window_group_view<'a>(&'a self, output: &'a str) -> Option<Element<'a, Message>> {
        self.window_group
            .view(
                self.niri_snapshot.get()?,
                self.window_assets(),
                self.audio_group.window_audio(),
                output,
                &self.config,
                Position::Bottom,
                &self.drag_view(),
                self.clock,
            )
            .map(|element| element.map(|event| Message::from(event.map(Message::WindowGroup))))
    }
}
