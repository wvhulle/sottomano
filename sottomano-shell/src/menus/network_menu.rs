use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(crate) fn network_group_menu(&self) -> GroupMenuView<'_> {
        let groups = self
            .widget_groups
            .wifi_adapters_group
            .group_contents()
            .into_iter()
            .chain(self.widget_groups.mobile_modems_group.group_contents())
            .chain(self.widget_groups.ethernet_ports_group.group_contents())
            .chain(self.widget_groups.bluetooth_adapters_group.group_contents(
                &|hardware_address| {
                    self.widget_groups
                        .gamepad_group
                        .report_timing_for_address(hardware_address)
                },
                self.clock,
            ))
            .chain([
                self.widget_groups
                    .reachability_group
                    .group_content(self.clock),
                self.widget_groups.tailnet_group.group_content(self.clock),
                self.widget_groups.wireguard_tunnels_group.group_content(),
                self.widget_groups.network_manager_vpn_group.group_content(),
            ]);
        self.group_menu(GroupMenuTopic::Network, groups)
    }
}
