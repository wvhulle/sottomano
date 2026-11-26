use iced::Task;
use sottomano_desktop_sources::system_protocol_clients::dbus_desktop_buses::DesktopBuses;
use sottomano_toml_config::Config;
use sottomano_widgets::{
    clock::Clock,
    group_indicators::indicator_group_interaction::IndicatorGroupBusRequest,
    panel_menus::{
        display_menu::{
            color_temperature_group::ColorTemperatureGroup, monitor_group::MonitorGroup,
        },
        media_menu::voice_dictation_group::VoiceDictationGroup,
        network_menu::{
            network_manager_vpn_group::NetworkManagerVpnGroup,
            reachability_group::ReachabilityGroup,
        },
    },
};

use crate::{indicator_groups::ShellIndicatorGroups, shell_message::Message};

impl ShellIndicatorGroups {
    pub fn request_on_buses(
        &mut self,
        bus_request: IndicatorGroupBusRequest,
        config: &Config,
        buses: &DesktopBuses,
        clock: Clock,
    ) -> Task<Message> {
        match bus_request {
            IndicatorGroupBusRequest::Weather(request) => self
                .panel_menus
                .agenda
                .weather_group
                .on_buses(request, buses)
                .map(Message::Weather),
            IndicatorGroupBusRequest::Power(request) => self
                .panel_menus
                .hardware
                .power_group
                .on_buses(request, config, buses, clock)
                .map(Message::Power),
            IndicatorGroupBusRequest::ClaudeUsage(request) => {
                let working_days = self.panel_menus.agenda.quiet_schedule_group.working_days;
                self.panel_menus
                    .services
                    .claude_usage_group
                    .on_buses(request, config, working_days, buses, clock)
                    .map(Message::ClaudeUsage)
            }
            IndicatorGroupBusRequest::CopilotUsage(request) => self
                .panel_menus
                .services
                .copilot_usage_group
                .on_buses(request, buses)
                .map(Message::CopilotUsage),
            IndicatorGroupBusRequest::OpenRouterCredit(request) => self
                .panel_menus
                .services
                .openrouter_credit_group
                .on_buses(request, config, buses)
                .map(Message::OpenRouterCredit),
            IndicatorGroupBusRequest::LoginSession(request) => self
                .panel_menus
                .session
                .login_session_group
                .on_buses(request, config, buses, clock)
                .map(Message::LoginSession),
            IndicatorGroupBusRequest::VoiceDictation(request) => {
                VoiceDictationGroup::on_buses(request, buses).map(Message::VoiceDictation)
            }
            IndicatorGroupBusRequest::Reachability(request) => {
                ReachabilityGroup::on_buses(request, buses).map(Message::Reachability)
            }
            IndicatorGroupBusRequest::Tailnet(request) => self
                .panel_menus
                .network
                .tailnet_group
                .on_buses(request, config, buses, clock)
                .map(Message::Tailnet),
            IndicatorGroupBusRequest::WireguardTunnels(request) => self
                .panel_menus
                .network
                .wireguard_tunnels_group
                .on_buses(request, config, buses, clock)
                .map(Message::WireguardTunnels),
            IndicatorGroupBusRequest::NetworkManagerVpns(request) => {
                NetworkManagerVpnGroup::on_buses(request, buses).map(Message::NetworkManagerVpns)
            }
            IndicatorGroupBusRequest::WifiAdapters(request) => self
                .panel_menus
                .network
                .wifi_adapters_group
                .on_buses(request, config, buses, clock)
                .map(Message::WifiAdapters),
            IndicatorGroupBusRequest::EthernetPorts(request) => self
                .panel_menus
                .network
                .ethernet_ports_group
                .on_buses(request, config, buses, clock)
                .map(Message::EthernetPorts),
            IndicatorGroupBusRequest::Monitor(request) => {
                MonitorGroup::on_buses(request, buses).map(Message::Monitor)
            }
            IndicatorGroupBusRequest::ColorTemperature(request) => {
                ColorTemperatureGroup::on_buses(request, buses).map(Message::ColorTemperature)
            }
        }
    }
}
