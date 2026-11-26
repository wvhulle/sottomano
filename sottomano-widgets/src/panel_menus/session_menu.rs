pub mod idle_pause_groups;
pub mod login_session_group;
pub mod tray_group;

use sottomano_desktop_sources::desktop_windows_apps::window_edge_placement::WindowOwner;
use sottomano_toml_config::Config;

use self::{
    idle_pause_groups::IdlePauseGroups, login_session_group::LoginSessionGroup,
    tray_group::TrayGroup,
};
use crate::{
    clock::Clock, group_indicators::indicator_group_content::IndicatorGroupContent,
    icon_catalog::drawable_icon_catalog::IconCatalog,
};

#[derive(Default)]
pub struct SessionMenu {
    pub idle_pause_groups: IdlePauseGroups,
    pub login_session_group: LoginSessionGroup,
    pub tray_group: TrayGroup,
}

impl SessionMenu {
    pub fn collapse(&mut self) {
        self.idle_pause_groups.collapse();
        self.login_session_group.collapse();
        self.tray_group.collapse();
    }

    #[must_use]
    pub fn group_contents<'view>(
        &'view self,
        config: &'view Config,
        icon_catalog: &'view IconCatalog,
        window_matches: &dyn Fn(&WindowOwner) -> bool,
        clock: Clock,
    ) -> Vec<IndicatorGroupContent<'view>> {
        vec![
            self.idle_pause_groups.idle_chain_group(config, clock),
            self.login_session_group.group_content(),
            self.tray_group
                .group_content(config, icon_catalog, window_matches),
        ]
    }
}
