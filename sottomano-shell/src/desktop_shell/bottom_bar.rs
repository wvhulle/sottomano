use iced::Element;
use sottomano_config::screen_edges::Position;
use sottomano_widgets::{
    hover_tooltips::GroupHandle,
    widget_groups::{sidebar_settings_group::WorkspacesPlacement, workspaces_group::inline_strip},
    widgets::{
        group_drawer::GroupDrawer,
        phosphor::{Phosphor, Weight},
        widget_group::GroupContent,
    },
};

use crate::{desktop_shell::DesktopShell, shell_message::Message};

pub(super) struct BottomBarGroups<'a> {
    pub settings_drawer: GroupDrawer<'a, Message>,
    pub start: [GroupContent<'a, Message>; 6],
    pub end: [GroupContent<'a, Message>; 9],
}

impl DesktopShell {
    pub(super) fn bottom_bar<'a>(&'a self, output: &'a str) -> Element<'a, Message> {
        let config = &self.config;
        let BottomBarGroups {
            settings_drawer,
            start,
            end,
        } = self.bottom_bar_groups();
        let view =
            |group: GroupContent<'a, Message>| group.view(config, Position::Bottom, self.clock);
        self.panel_bar(
            Position::Bottom,
            [Some(settings_drawer.handle_view(
                config,
                Position::Bottom,
                self.clock,
            ))]
            .into_iter()
            .chain(start.map(view))
            .chain([self.inline_workspaces(output)]),
            [self.window_group_view(output)],
            end.map(view),
        )
    }

    pub(super) fn bottom_bar_groups(&self) -> BottomBarGroups<'_> {
        BottomBarGroups {
            settings_drawer: self.settings_drawer(),
            start: self.bottom_bar_start_groups(),
            end: self.bottom_bar_end_groups(),
        }
    }

    pub(super) fn bar_drawers(&self) -> [GroupDrawer<'_, Message>; 1] {
        [self.settings_drawer()]
    }

    fn settings_drawer(&self) -> GroupDrawer<'_, Message> {
        let config = &self.config;
        GroupDrawer::new(
            GroupHandle::SettingsDrawer,
            "Settings",
            Phosphor::SlidersHorizontal(Weight::Regular),
            [
                self.widget_groups
                    .panel_settings_group
                    .group_content(config)
                    .map(Message::PanelSettings),
                self.widget_groups
                    .sidebar_settings_group
                    .group_content(config)
                    .map(Message::SidebarSettings),
                self.widget_groups
                    .overlay_settings_group
                    .group_content(config)
                    .map(Message::OverlaySettings),
                self.widget_groups
                    .niri_window_group
                    .group_content(config)
                    .map(Message::NiriWindow),
            ],
        )
    }

    fn bottom_bar_start_groups(&self) -> [GroupContent<'_, Message>; 6] {
        let config = &self.config;
        [
            self.widget_groups
                .monitor_group
                .group_content(config)
                .map(Message::Monitor),
            self.widget_groups
                .dark_mode_group
                .group_content(config)
                .map(Message::DarkMode),
            self.widget_groups
                .appearance_presets_menu
                .group_content(config)
                .map(Message::AppearancePresets),
            self.widget_groups
                .color_temperature_group
                .group_content(config)
                .map(Message::ColorTemperature),
            self.widget_groups
                .wallpaper_group
                .group_content()
                .map(Message::Wallpaper),
            self.widget_groups
                .idle_timeouts_group
                .group_content(config)
                .map(Message::IdleTimeouts),
        ]
    }

    fn bottom_bar_end_groups(&self) -> [GroupContent<'_, Message>; 9] {
        let config = &self.config;
        [
            self.widget_groups
                .llama_server_group
                .group_content()
                .map(Message::LlamaServer),
            self.widget_groups
                .claude_usage_group
                .group_content()
                .map(Message::ClaudeUsage),
            self.widget_groups
                .copilot_usage_group
                .group_content()
                .map(Message::CopilotUsage),
            self.widget_groups
                .openrouter_credit_group
                .group_content()
                .map(Message::OpenRouterCredit),
            self.widget_groups
                .systemd_manager_group
                .group_content(config, self.clock)
                .map(Message::SystemdManager),
            self.widget_groups
                .journal_events_group
                .group_content(config)
                .map(Message::JournalEvents),
            self.widget_groups
                .pending_upgrades_group
                .group_content(config)
                .map(Message::PendingUpgrades),
            self.widget_groups
                .login_session_group
                .group_content()
                .map(Message::LoginSession),
            self.widget_groups
                .tray_group
                .group_content(config)
                .map(Message::Tray),
        ]
    }

    fn inline_workspaces<'a>(&'a self, output: &'a str) -> Option<Element<'a, Message>> {
        match self
            .widget_groups
            .sidebar_settings_group
            .workspaces_placement()
        {
            WorkspacesPlacement::Sidebar => None,
            WorkspacesPlacement::BottomPanel => inline_strip::inline_view(
                &self.niri_snapshot,
                output,
                self.configured_workspaces.as_ref(),
                self.bar_slot_context(Position::Bottom),
            )
            .map(|workspaces| workspaces.map(Message::from)),
        }
    }

    fn window_group_view<'a>(&'a self, output: &'a str) -> Option<Element<'a, Message>> {
        self.widget_groups
            .window_group
            .view(
                &self.niri_snapshot,
                self.window_assets(),
                self.widget_groups.audio_group.window_audio(),
                output,
                self.bar_slot_context(Position::Bottom),
            )
            .map(|element| element.map(|event| Message::from(event.map(Message::WindowGroup))))
    }
}
