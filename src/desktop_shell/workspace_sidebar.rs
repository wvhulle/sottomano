use iced::{Element, widget::Column, window::Id};

use crate::{
    desktop_shell::{DesktopShell, Message, monitor_surfaces::ShellElement},
    state::WorkspacesPlacement,
    widget_groups::workspaces_group,
    widgets::{
        region_reporter::{RegionRoles, region_reporter},
        widget_group_frame::widget_group_frame,
    },
};

const SIDEBAR_REGION_ROLES: RegionRoles = RegionRoles {
    takes_input: true,
    blurred: true,
};

pub struct WorkspaceSidebar<'a> {
    shell: &'a DesktopShell,
    window: Id,
    output: &'a str,
}

impl<'a> WorkspaceSidebar<'a> {
    pub fn new(shell: &'a DesktopShell, window: Id, output: &'a str) -> Self {
        Self {
            shell,
            window,
            output,
        }
    }
}

impl<'a> From<WorkspaceSidebar<'a>> for Element<'a, Message> {
    fn from(sidebar: WorkspaceSidebar<'a>) -> Self {
        let WorkspaceSidebar {
            shell,
            window,
            output,
        } = sidebar;
        let config = &shell.config;
        let layout = &shell.panel_settings.layout;
        let framed_workspaces = (layout.workspaces_placement == WorkspacesPlacement::Sidebar)
            .then(|| workspaces_group::vertical_view(&shell.niri_snapshot, output, config))
            .flatten()
            .map(|workspaces| {
                widget_group_frame(workspaces.map(Message::from), layout, &config.palette)
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
