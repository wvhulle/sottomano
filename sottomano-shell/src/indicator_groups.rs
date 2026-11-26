mod group_actions;
mod group_appearance_settings;
mod group_bus_requests;
mod group_delivery_routing;
mod group_hover_cards;
mod group_routing;
mod group_subscriptions;

use iced::Task;
use sottomano_compositor_model::compositor_window_state::CompositorWindowState;
use sottomano_desktop_sinks::compositor_rule_files::CompositorRulesFile;
use sottomano_toml_config::{Config, SecretPaths};
use sottomano_widgets::{
    cascading_menus::{app_launcher_menu::AppLauncherMenu, window_menu_bar::WindowMenuBar},
    clock::Clock,
    group_menus::group_menu_state::GroupMenuState,
    output_windows::OutputWindows,
    panel_menus::{
        PanelMenus,
        agenda_menu::{
            notification_group::NotificationGroup, quiet_schedule_group::QuietScheduleGroup,
            session_focus_group::SessionFocusGroup,
        },
        appearance_menu::{
            overlay_settings_group::OverlaySettingsGroup,
            panel_settings_group::{GroupCollapsing, PanelSettingsGroup},
            sidebar_settings_group::SidebarSettingsGroup,
        },
        display_menu::{
            color_temperature_group::ColorTemperatureGroup, dark_mode_group::DarkModeGroup,
        },
        gaming_menu::gamepad_group::GamepadGroup,
        session_menu::idle_pause_groups::IdlePauseGroups,
        session_power_menu::session_power_group::SessionPowerGroup,
    },
};

use crate::shell_message::Message;

pub struct ShellIndicatorGroups {
    pub panel_menus: PanelMenus,
    pub group_menus: GroupMenuState,
    pub window_menu_bar: WindowMenuBar,
    pub output_windows: OutputWindows,
    pub app_launcher_menu: AppLauncherMenu,
}

impl ShellIndicatorGroups {
    pub fn new(
        config: &Config,
        secrets: SecretPaths,
        compositor_rules: CompositorRulesFile,
        clock: Clock,
    ) -> Self {
        Self {
            panel_menus: PanelMenus::new(config, secrets, compositor_rules, clock),
            group_menus: GroupMenuState::default(),
            window_menu_bar: WindowMenuBar::default(),
            output_windows: OutputWindows::default(),
            app_launcher_menu: AppLauncherMenu::default(),
        }
    }

    pub fn load(&self, clock: Clock) -> Task<Message> {
        Task::batch([
            SessionFocusGroup::load(clock).map(Message::SessionFocus),
            SessionPowerGroup::load().map(Message::Session),
            IdlePauseGroups::load().map(Message::IdleTimeouts),
            GroupMenuState::load().map(Message::GroupMenuState),
            AppLauncherMenu::load().map(Message::AppLauncher),
            PanelSettingsGroup::load().map(Message::PanelSettings),
            SidebarSettingsGroup::load().map(Message::SidebarSettings),
            OverlaySettingsGroup::load().map(Message::OverlaySettings),
            WindowMenuBar::load().map(Message::WindowMenuBar),
            NotificationGroup::load().map(Message::NotificationGroup),
            QuietScheduleGroup::load().map(Message::QuietSchedule),
            DarkModeGroup::load().map(Message::DarkMode),
            GamepadGroup::load().map(Message::Gamepad),
            self.panel_menus
                .appearance
                .compositor_styling_group
                .load()
                .map(Message::CompositorStyling),
        ])
    }

    pub fn reheat_switch_heat(
        &mut self,
        config: &Config,
        compositor: &CompositorWindowState,
        clock: Clock,
    ) -> Task<Message> {
        let session_focus_group = &self.panel_menus.agenda.session_focus_group;
        self.panel_menus
            .appearance
            .compositor_styling_group
            .reheat(config, || {
                session_focus_group.heated_windows(compositor, config, clock)
            })
            .map(Message::CompositorStyling)
    }

    pub fn adopt_config(&mut self, previous: &Config, config: &Config) {
        let service = &config.launched_programs.color_temperature_service;
        if *service != previous.launched_programs.color_temperature_service {
            self.panel_menus.display.color_temperature_group =
                ColorTemperatureGroup::new(service.clone());
        }
    }

    pub fn collapse_when_auto_collapsing(&mut self) {
        if self
            .panel_menus
            .appearance
            .panel_settings_group
            .group_collapsing
            == GroupCollapsing::StayExpanded
        {
            return;
        }
        self.panel_menus.collapse_groups();
        self.window_menu_bar.collapse();
        self.group_menus.collapse_settings();
    }
}
