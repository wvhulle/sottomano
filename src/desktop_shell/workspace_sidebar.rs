use iced::{
    Element,
    widget::{Column, container},
    window::Id,
};

use crate::{
    desktop_shell::{DesktopShell, Message},
    state::WorkspacesPlacement,
    widget_groups::workspaces_group,
    widgets::region_reporter::{RegionRoles, region_reporter},
};

impl DesktopShell {
    pub(super) fn workspace_sidebar<'a>(
        &'a self,
        window: Id,
        output: &str,
    ) -> Element<'a, Message> {
        let config = &self.config;
        let panels = &self.shell_settings.panels;
        let frame_style = self.shell_settings.frame_style(&config.palette);
        let framed_workspaces = (self.shell_settings.layout.workspaces_placement
            == WorkspacesPlacement::Sidebar)
            .then(|| workspaces_group::vertical_view(&self.niri_snapshot, output, config))
            .flatten()
            .map(|workspaces| {
                container(workspaces.map(Message::from))
                    .style(move |_| frame_style)
                    .padding([panels.corner_radius.top_left, 0.0])
                    .into()
            });
        region_reporter(
            Column::with_children(framed_workspaces),
            RegionRoles {
                takes_input: true,
                blurred: true,
                corner_radius: panels.corner_radius.top_left,
            },
        )
        .on_width_change(move |width| Message::SidebarWidthChanged { window, width })
        .into()
    }
}
