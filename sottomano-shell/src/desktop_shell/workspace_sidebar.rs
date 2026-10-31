use iced::{
    Element,
    widget::{Column, container},
    window::Id,
};
use sottomano_config::screen_edges::Position;
use sottomano_widgets::{
    widget_groups::{
        sidebar_settings_group::WorkspacesPlacement, workspaces_group::sidebar_column,
    },
    widgets::{
        panel_frame::{LayerFrameStyle, OutlinedSides, panel_frame},
        region_reporter::{RegionRoles, region_reporter},
    },
};

use crate::{desktop_shell::DesktopShell, shell_message::Message};

impl DesktopShell {
    pub(super) fn workspace_sidebar<'a>(
        &'a self,
        window: Id,
        output: &str,
    ) -> Element<'a, Message> {
        let config = &self.config;
        let sidebar = self
            .widget_groups
            .sidebar_settings_group
            .sidebar_appearance();
        let frame_style = LayerFrameStyle::new(&sidebar, config);
        let outlined_sides = OutlinedSides::inward_of(Position::Left)
            .all_when_floating(self.widget_groups.panel_settings_group.float_offset_px());
        let framed_workspaces = (self
            .widget_groups
            .sidebar_settings_group
            .workspaces_placement()
            == WorkspacesPlacement::Sidebar)
            .then_some(&self.niri_snapshot)
            .and_then(|snapshot| {
                sidebar_column::vertical_view(
                    snapshot,
                    output,
                    self.configured_workspaces.as_ref(),
                    self.widget_groups.sidebar_settings_group.empty_workspaces(),
                    frame_style.drawn_corners(outlined_sides),
                    self.bar_slot_context(Position::Left),
                )
            })
            .map(|workspaces| {
                panel_frame(container(workspaces.map(Message::from)), frame_style)
                    .outlined_sides(outlined_sides)
                    .blurred(sidebar.blurred)
                    .into()
            });
        region_reporter(
            Column::with_children(framed_workspaces),
            RegionRoles {
                takes_input: true,
                ..RegionRoles::default()
            },
        )
        .on_width_change(move |width| Message::SidebarWidthChanged { window, width })
        .into()
    }
}
