use iced::Subscription;
use sottomano_config::Config;
use sottomano_sources::{
    host_machine_identity::udev_hardware_identity::HostHardware,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_widgets::{
    background_work::snapshot_subscription,
    widget_groups::{
        audio_group::AudioGroup, bluetooth_adapters_group::BluetoothAdaptersGroup,
        clock_group::ClockGroup, compute_group::ComputeGroup,
        copilot_usage_group::CopilotUsageGroup, dark_mode_group::DarkModeGroup,
        gamepad_group::GamepadGroup, journal_events_group::JournalEventsGroup,
        login_session_group::LoginSessionGroup, memory_board_group::MemoryBoardGroup,
        mobile_modems_group::MobileModemsGroup, monitor_group::MonitorGroup,
        pending_upgrades_group::PendingUpgradesGroup, power_group::PowerGroup,
        quiet_schedule_group::QuietScheduleGroup, session_focus_group::SessionFocusGroup,
        systemd_manager_group::SystemdManagerGroup, tray_group::TrayGroup,
        window_group::WindowGroup,
    },
};

use crate::{desktop_shell::shell_widget_groups::ShellWidgetGroups, shell_message::Message};

impl ShellWidgetGroups {
    pub fn subscription(
        &self,
        config: &Config,
        focused_app_id: Option<&str>,
        buses: Option<&DesktopBuses>,
    ) -> Subscription<Message> {
        let working_days = self.quiet_schedule_group.working_days();
        Subscription::batch([
            self.weather_group.subscription(buses).map(Message::Weather),
            self.wallpaper_group
                .subscription(buses)
                .map(Message::Wallpaper),
            self.caldav_group
                .subscription(&config.widget_groups.caldav, buses)
                .map(Message::Caldav),
            ComputeGroup::subscription(&config.widget_groups.temperature_sensors.cpu)
                .map(Message::Compute),
            MemoryBoardGroup::subscription().map(Message::MemoryBoard),
            self.gpu_group.subscription().map(Message::Gpu),
            self.storage_space_group
                .subscription()
                .map(Message::Storage),
            self.physical_drive_group
                .subscription(&config.widget_groups.temperature_sensors.drive)
                .map(Message::Drive),
            ClockGroup::subscription(config).map(Message::Clock),
            JournalEventsGroup::subscription().map(Message::JournalEvents),
            DarkModeGroup::subscription().map(Message::DarkMode),
            TrayGroup::subscription().map(Message::Tray),
            AudioGroup::subscription().map(Message::Audio),
            self.gamepad_group
                .subscription(&config.widget_groups.gamepad, focused_app_id)
                .map(Message::Gamepad),
            self.claude_usage_group
                .subscription(config, working_days, buses)
                .map(Message::ClaudeUsage),
            CopilotUsageGroup::subscription(config, working_days).map(Message::CopilotUsage),
            self.openrouter_credit_group
                .subscription(config, buses)
                .map(Message::OpenRouterCredit),
            WindowGroup::subscription(&config.widget_groups.window_list.agents)
                .map(Message::WindowGroup),
            SessionFocusGroup::subscription(&config.widget_groups.focus_time)
                .map(Message::SessionFocus),
        ])
    }

    pub fn desktop_bus_subscription(
        &self,
        config: &Config,
        buses: &DesktopBuses,
    ) -> Subscription<Message> {
        Subscription::batch([
            PowerGroup::subscription(buses).map(Message::Power),
            LoginSessionGroup::subscription(buses).map(Message::LoginSession),
            SystemdManagerGroup::subscription(&config.widget_groups.systemd_manager, buses)
                .map(Message::SystemdManager),
            PendingUpgradesGroup::subscription(buses).map(Message::PendingUpgrades),
            self.reachability_group
                .subscription(&config.widget_groups.reachability, buses)
                .map(Message::Reachability),
            self.vpn_group.subscription(buses).map(Message::Vpn),
            self.wifi_adapters_group
                .subscription(buses)
                .map(Message::WifiAdapters),
            MobileModemsGroup::subscription(buses).map(Message::MobileModems),
            self.ethernet_ports_group
                .subscription(buses)
                .map(Message::EthernetPorts),
            BluetoothAdaptersGroup::subscription(buses).map(Message::BluetoothAdapters),
            GamepadGroup::network_subscription(buses).map(Message::Gamepad),
            MonitorGroup::subscription(buses).map(Message::Monitor),
            self.media_group
                .subscription(buses, &config.widget_groups.media_lyrics)
                .map(Message::Media),
            DarkModeGroup::resume_subscription(buses).map(Message::DarkMode),
            self.voice_dictation_group
                .subscription(buses)
                .map(Message::VoiceDictation),
            QuietScheduleGroup::resume_subscription(buses).map(Message::QuietSchedule),
            SessionFocusGroup::sleep_subscription(buses).map(Message::SessionFocus),
            WindowGroup::launcher_entry_subscription(buses).map(Message::WindowGroup),
            snapshot_subscription::<HostHardware>(buses.clone())
                .map(|delivery| Message::HostHardware(Box::new(delivery))),
            self.copilot_usage_group
                .account_subscription(buses)
                .map(Message::CopilotUsage),
            self.color_temperature_group
                .subscription(buses)
                .map(Message::ColorTemperature),
            self.llama_server_group
                .subscription(buses)
                .map(Message::LlamaServer),
        ])
    }
}
