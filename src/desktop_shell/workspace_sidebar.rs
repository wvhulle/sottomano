use iced::{
    Element,
    widget::{Column, container},
    window::Id,
};

use crate::{
    desktop_shell::{DesktopShell, Message, monitor_surfaces::ShellElement},
    state::WorkspacesPlacement,
    widget_groups::workspaces_group,
    widgets::region_reporter::{RegionRoles, region_reporter},
};

const SIDEBAR_REGION_ROLES: RegionRoles = RegionRoles {
    takes_input: true,
    blurred: true,
};

impl DesktopShell {
    pub(super) fn workspace_sidebar<'a>(
        &'a self,
        window: Id,
        output: &str,
    ) -> Element<'a, Message> {
        let config = &self.config;
        let layout = &self.panel_settings.layout;
        let frame_style = layout.frame_style(&config.palette);
        let framed_workspaces = (layout.workspaces_placement == WorkspacesPlacement::Sidebar)
            .then(|| workspaces_group::vertical_view(&self.niri_snapshot, output, config))
            .flatten()
            .map(|workspaces| {
                container(workspaces.map(Message::from))
                    .style(move |_| frame_style)
                    .padding([
                        layout.frame_corner_inset() + config.spacing.group_padding_px,
                        0.0,
                    ])
                    .into()
            });
        region_reporter(
            Column::with_children(framed_workspaces),
            SIDEBAR_REGION_ROLES,
            move |region| Message::ElementRegions {
                window,
                element: ShellElement::WorkspaceSidebar,
                region,
            },
        )
        .into()
    }
}
