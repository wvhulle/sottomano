use iced::{
    Element,
    widget::{Column, container},
    window::Id,
};

use crate::{
    desktop_shell::{DesktopShell, Message, monitor_surfaces::Position},
    state::WorkspacesPlacement,
    widget_groups::workspaces_group,
    widgets::{
        panel_frame::panel_frame,
        region_reporter::{RegionRoles, region_reporter},
    },
};

impl DesktopShell {
    pub(super) fn workspace_sidebar<'a>(
        &'a self,
        window: Id,
        output: &str,
    ) -> Element<'a, Message> {
        let config = &self.config;
        let sidebar = &self.shell_settings.sidebar;
        let frame_style = sidebar.frame_style(config);
        let outlined_sides = self
            .shell_settings
            .outlined_sides(Position::Left.inward_sides());
        let framed_workspaces = (self.shell_settings.layout.workspaces_placement
            == WorkspacesPlacement::Sidebar)
            .then(|| self.niri_snapshot.get())
            .flatten()
            .and_then(|snapshot| {
                workspaces_group::vertical_view(
                    snapshot,
                    output,
                    config,
                    &self.drag_view(),
                    self.configured_workspaces.as_ref(),
                )
            })
            .map(|workspaces| {
                panel_frame(
                    container(workspaces.map(Message::from))
                        .padding([sidebar.corner_radius.top_left, 0.0]),
                    &frame_style,
                )
                .outlined_sides(outlined_sides)
                .into()
            });
        region_reporter(
            Column::with_children(framed_workspaces),
            RegionRoles {
                takes_input: true,
                blurred: sidebar.blurred,
                corner_radius: sidebar.corner_radius.top_left,
            },
        )
        .on_width_change(move |width| Message::SidebarWidthChanged { window, width })
        .into()
    }
}
