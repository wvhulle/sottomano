use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(crate) fn services_group_menu(&self) -> GroupMenuView<'_> {
        let config = &self.config;
        self.group_menu(
            GroupMenuTopic::Services,
            [
                self.indicator_groups.llama_server_group.group_content(),
                self.indicator_groups
                    .ci_runs_group
                    .group_content(self.clock, self.config.calendar.locale),
                self.indicator_groups
                    .claude_usage_group
                    .group_content(self.clock),
                self.indicator_groups
                    .copilot_usage_group
                    .group_content(self.clock),
                self.indicator_groups
                    .openrouter_credit_group
                    .group_content(self.clock),
                self.indicator_groups
                    .systemd_manager_group
                    .group_content(config, self.clock),
                self.indicator_groups
                    .journal_events_group
                    .system_journal_content(config),
                self.indicator_groups
                    .journal_events_group
                    .user_journal_content(config),
                self.indicator_groups
                    .pending_upgrades_group
                    .group_content(config),
            ],
        )
    }
}
