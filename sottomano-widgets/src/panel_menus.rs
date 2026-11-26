pub mod agenda_menu;
pub mod appearance_menu;
pub mod behaviour_menu;
pub mod display_menu;
pub mod gaming_menu;
pub mod hardware_menu;
pub mod media_menu;
pub mod network_menu;
pub mod services_menu;
pub mod session_menu;
pub mod session_power_menu;
pub mod virtualization_menu;

use sottomano_desktop_sinks::compositor_rule_files::CompositorRulesFile;
use sottomano_toml_config::{Config, SecretPaths};

use self::{
    agenda_menu::AgendaMenu, appearance_menu::AppearanceMenu, behaviour_menu::BehaviourMenu,
    display_menu::DisplayMenu, gaming_menu::GamingMenu, hardware_menu::HardwareMenu,
    media_menu::MediaMenu, network_menu::NetworkMenu, services_menu::ServicesMenu,
    session_menu::SessionMenu, session_power_menu::SessionPowerMenu,
    virtualization_menu::VirtualizationMenu,
};
use crate::clock::Clock;

pub struct PanelMenus {
    pub hardware: HardwareMenu,
    pub network: NetworkMenu,
    pub agenda: AgendaMenu,
    pub media: MediaMenu,
    pub gaming: GamingMenu,
    pub display: DisplayMenu,
    pub virtualization: VirtualizationMenu,
    pub services: ServicesMenu,
    pub session: SessionMenu,
    pub appearance: AppearanceMenu,
    pub behaviour: BehaviourMenu,
    pub session_power: SessionPowerMenu,
}

impl PanelMenus {
    #[must_use]
    pub fn new(
        config: &Config,
        secrets: SecretPaths,
        compositor_rules: CompositorRulesFile,
        clock: Clock,
    ) -> Self {
        let agenda = AgendaMenu::new(config, &secrets, clock);
        let display = DisplayMenu::new(config, &secrets, clock);
        Self {
            hardware: HardwareMenu::default(),
            network: NetworkMenu::new(secrets.tailscale_api_token),
            agenda,
            media: MediaMenu::default(),
            gaming: GamingMenu::default(),
            display,
            virtualization: VirtualizationMenu::default(),
            services: ServicesMenu::new(
                secrets.claude_code_oauth,
                secrets.gh_auth_token,
                secrets.openrouter_management,
            ),
            session: SessionMenu::default(),
            appearance: AppearanceMenu::new(compositor_rules),
            behaviour: BehaviourMenu::default(),
            session_power: SessionPowerMenu::default(),
        }
    }

    pub fn collapse_groups(&mut self) {
        self.hardware.collapse();
        self.network.collapse();
        self.agenda.collapse();
        self.media.collapse();
        self.gaming.collapse();
        self.display.collapse();
        self.virtualization.collapse();
        self.services.collapse();
        self.session.collapse();
        self.appearance.collapse();
        self.behaviour.collapse();
    }
}
