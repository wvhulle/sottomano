pub mod agenda_menu;
pub mod gaming_menu;
pub mod hardware_menu;
pub mod media_menu;
pub mod network_menu;
pub mod services_menu;
pub mod session_menu;

use serde::{Deserialize, Serialize};

use self::{
    agenda_menu::AgendaMenuConfig, gaming_menu::GamingMenuConfig,
    hardware_menu::HardwareMenuConfig, media_menu::MediaMenuConfig,
    network_menu::NetworkMenuConfig, services_menu::ServicesMenuConfig,
    session_menu::SessionMenuConfig,
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PanelMenusConfig {
    pub hardware: HardwareMenuConfig,
    pub network: NetworkMenuConfig,
    pub agenda: AgendaMenuConfig,
    pub media: MediaMenuConfig,
    pub gaming: GamingMenuConfig,
    pub services: ServicesMenuConfig,
    pub session: SessionMenuConfig,
}
