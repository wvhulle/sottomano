use sottomano_widgets::{
    group_items::group_item::RowHoverCard,
    hover_cards::{HoverTarget, hover_card_body::Section},
    widget_groups::{
        window_group::{WindowGroup, WindowTarget},
        workspaces_group::workspace_hover_card,
    },
};

use crate::{
    desktop_shell::DesktopShell,
    panels::{PanelZoneEntry, all_panel_entries},
};

impl DesktopShell {
    #[must_use]
    pub fn row_hover_card(&self, target: &HoverTarget) -> Option<RowHoverCard> {
        let output = self.hovered_output();
        all_panel_entries().find_map(|entry| match entry {
            PanelZoneEntry::Menu(topic) => self
                .group_menu_of(topic, output)
                .row_hover_card(target, self.clock),
            PanelZoneEntry::Standalone(group) => self
                .standalone_group(group)
                .row_hover_card(target, self.clock),
            PanelZoneEntry::Fixed(fixed_entry) => self
                .fixed_panel_group(fixed_entry)
                .row_hover_card(target, self.clock),
            PanelZoneEntry::InlineWorkspaces | PanelZoneEntry::OutputWindows => None,
        })
    }

    #[must_use]
    pub fn hover_card_sections(&self, target: &HoverTarget) -> Vec<Section> {
        self.row_hover_card(target).map_or_else(
            || self.standalone_hover_card(target),
            |row_card| row_card.sections,
        )
    }

    fn standalone_hover_card(&self, target: &HoverTarget) -> Vec<Section> {
        match target {
            HoverTarget::Window(WindowTarget::Inspect(_)) => {
                self.widget_groups.window_group.inspect_hover_card()
            }
            HoverTarget::Window(WindowTarget::Usage(window)) => {
                let mut window_sections =
                    WindowGroup::window_identity_hover_card(&self.compositor, *window, self.clock);
                window_sections.extend(
                    self.widget_groups
                        .session_focus_group
                        .window_usage_hover_card(*window, &self.config, self.clock),
                );
                window_sections
            }
            HoverTarget::Workspace(workspace) => workspace_hover_card::hover_card(
                &self.window_snapshot,
                &self.desktop_entries,
                self.widget_groups
                    .compositor_styling_group
                    .app_window_layout(),
                *workspace,
            ),
            _ => Vec::new(),
        }
    }
}
