use iced::Task;
use sottomano_config::Config;
use sottomano_sources::system_protocol_clients::dbus_desktop_buses::DesktopBuses;
use sottomano_widgets::{
    hover_cards::HoverTarget,
    widget_groups::{
        clock_group::ClockGroup, color_temperature_group::ColorTemperatureGroup,
        compute_group::ComputeGroup, dark_mode_group::DarkModeGroup,
        login_session_group::LoginSessionGroup, memory_board_group::MemoryBoardGroup,
        monitor_group::MonitorGroup, power_group::PowerGroup,
        reachability_group::ReachabilityGroup, voice_dictation_group::VoiceDictationGroup,
        vpn_group::VpnGroup,
    },
};

use crate::{desktop_shell::shell_widget_groups::ShellWidgetGroups, shell_message::Message};

impl ShellWidgetGroups {
    pub fn refresh_reading(
        &self,
        target: &HoverTarget,
        config: &Config,
        buses: Option<&DesktopBuses>,
    ) -> Task<Message> {
        let on_buses = |refresh: &dyn Fn(&DesktopBuses) -> Task<Message>| {
            buses.map_or_else(Task::none, refresh)
        };
        let widget_group_settings = &config.widget_groups;
        let working_days = self.quiet_schedule_group.working_days();
        match target {
            HoverTarget::Weather(target) => self
                .weather_group
                .refresh(*target, buses)
                .map(Message::Weather),
            HoverTarget::Compute(target) => {
                ComputeGroup::refresh(*target, &widget_group_settings.compute.temperature_sensors)
                    .map(Message::Compute)
            }
            HoverTarget::MemoryBoard(target) => {
                MemoryBoardGroup::refresh(*target, buses).map(Message::MemoryBoard)
            }
            HoverTarget::Gpu(target) => self.gpu_group.refresh(target).map(Message::Gpu),
            HoverTarget::Storage(target) => self
                .storage_space_group
                .refresh(target)
                .map(Message::Storage),
            HoverTarget::Drive(target) => self
                .physical_drive_group
                .refresh(
                    target,
                    &widget_group_settings.physical_drive.temperature_sensors,
                )
                .map(Message::Drive),
            HoverTarget::Clock(target) => ClockGroup::refresh(target).map(Message::Clock),
            HoverTarget::DarkMode(target) => DarkModeGroup::refresh(*target).map(Message::DarkMode),
            HoverTarget::VoiceDictation(target) => on_buses(&|buses| {
                VoiceDictationGroup::refresh(*target, buses).map(Message::VoiceDictation)
            }),
            HoverTarget::ClaudeUsage(target) => self
                .claude_usage_group
                .refresh(*target, config, working_days, buses)
                .map(Message::ClaudeUsage),
            HoverTarget::CopilotUsage(target) => self
                .copilot_usage_group
                .refresh(*target, config, working_days, buses)
                .map(Message::CopilotUsage),
            HoverTarget::OpenRouterCredit(target) => self
                .openrouter_credit_group
                .refresh(*target, config, buses)
                .map(Message::OpenRouterCredit),
            HoverTarget::Power(target) => {
                on_buses(&|buses| PowerGroup::refresh(*target, buses).map(Message::Power))
            }
            HoverTarget::LoginSession(target) => on_buses(&|buses| {
                LoginSessionGroup::refresh(*target, buses).map(Message::LoginSession)
            }),
            HoverTarget::Reachability(_) => {
                on_buses(&|buses| ReachabilityGroup::refresh(buses).map(Message::Reachability))
            }
            HoverTarget::Vpn(target) => {
                on_buses(&|buses| VpnGroup::refresh(target, buses).map(Message::Vpn))
            }
            HoverTarget::Monitor(target) => {
                on_buses(&|buses| MonitorGroup::refresh(target, buses).map(Message::Monitor))
            }
            HoverTarget::ColorTemperature(target) => on_buses(&|buses| {
                ColorTemperatureGroup::refresh(*target, buses).map(Message::ColorTemperature)
            }),
            HoverTarget::PanelSettings(_)
            | HoverTarget::SidebarSettings(_)
            | HoverTarget::AppearancePresets(_)
            | HoverTarget::OverlaySettings(_)
            | HoverTarget::NiriWindow(_)
            | HoverTarget::Systemd(_)
            | HoverTarget::Journal(_)
            | HoverTarget::PendingUpgrades(_)
            | HoverTarget::WifiAdapter(_)
            | HoverTarget::MobileModem(_)
            | HoverTarget::EthernetPort(_)
            | HoverTarget::Bluetooth(_)
            | HoverTarget::Audio(_)
            | HoverTarget::IdleTimeout(_)
            | HoverTarget::PauseBlocker(_)
            | HoverTarget::Wallpaper(_)
            | HoverTarget::Caldav(_)
            | HoverTarget::Media(_)
            | HoverTarget::Gamepad(_)
            | HoverTarget::LlamaServer(_)
            | HoverTarget::SessionFocus(_)
            | HoverTarget::AppLauncher(_)
            | HoverTarget::GroupMenu(_)
            | HoverTarget::GroupFace(_)
            | HoverTarget::GroupMenuSettings(_)
            | HoverTarget::Notification(_)
            | HoverTarget::QuietSchedule(_)
            | HoverTarget::Session(_)
            | HoverTarget::Tray(_)
            | HoverTarget::Window(_)
            | HoverTarget::Workspace(_)
            | HoverTarget::WorkspaceOrder
            | HoverTarget::WorkspaceFolding => Task::none(),
        }
    }
}
