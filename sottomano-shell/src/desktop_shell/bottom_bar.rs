use iced::Element;
use sottomano_config::screen_edges::Position;
use sottomano_widgets::{
    widget_groups::{
        group_menus::GroupMenuKind, sidebar_settings_group::WorkspacesPlacement,
        workspaces_group::inline_strip,
    },
    widgets::{
        group_menu::GroupMenu,
        phosphor::{Phosphor, Weight},
        widget_group::GroupContent,
    },
};

use crate::{desktop_shell::DesktopShell, shell_message::Message};

pub(super) struct BottomBarGroups<'a> {
    pub settings_menus: [GroupMenu<'a, Message>; 2],
    pub start: [GroupContent<'a, Message>; 6],
    pub services_menu: GroupMenu<'a, Message>,
    pub end: [GroupContent<'a, Message>; 2],
}

impl DesktopShell {
    pub(super) fn bottom_bar<'a>(&'a self, output: &'a str) -> Element<'a, Message> {
        let config = &self.config;
        let BottomBarGroups {
            settings_menus,
            start,
            services_menu,
            end,
        } = self.bottom_bar_groups();
        let view =
            |group: GroupContent<'a, Message>| group.view(config, Position::Bottom, self.clock);
        self.panel_bar(
            Position::Bottom,
            settings_menus
                .into_iter()
                .flat_map(|group_menu| group_menu.bar_views(config, Position::Bottom, self.clock))
                .chain(start.map(view))
                .chain([self.inline_workspaces(output)]),
            [self.window_group_view(output)],
            services_menu
                .bar_views(config, Position::Bottom, self.clock)
                .chain(end.map(view)),
        )
    }

    pub(super) fn bottom_bar_groups(&self) -> BottomBarGroups<'_> {
        BottomBarGroups {
            settings_menus: self.settings_menus(),
            start: self.bottom_bar_start_groups(),
            services_menu: self.services_menu(),
            end: self.bottom_bar_end_groups(),
        }
    }

    pub(super) fn settings_menus(&self) -> [GroupMenu<'_, Message>; 2] {
        [
            self.group_menu(
                GroupMenuKind::Appearance,
                "Appearance",
                Phosphor::SlidersHorizontal(Weight::Regular),
                self.settings_groups().map(GroupContent::only_aligned_items),
            ),
            self.group_menu(
                GroupMenuKind::Behaviour,
                "Behaviour",
                Phosphor::ToggleRight(Weight::Regular),
                self.settings_groups()
                    .map(GroupContent::only_unaligned_items),
            ),
        ]
    }

    fn settings_groups(&self) -> [GroupContent<'_, Message>; 4] {
        let config = &self.config;
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
        ]
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

    pub(super) fn services_menu(&self) -> GroupMenu<'_, Message> {
        let config = &self.config;
        self.group_menu(
            GroupMenuKind::Services,
            "Services",
            Phosphor::Stack(Weight::Regular),
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
            ],
        )
    }

    fn bottom_bar_end_groups(&self) -> [GroupContent<'_, Message>; 2] {
        let config = &self.config;
        [
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
                self.widget_groups.sidebar_settings_group.empty_workspaces(),
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
