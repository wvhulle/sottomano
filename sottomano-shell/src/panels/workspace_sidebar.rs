use iced::{
    Element,
    widget::{Column, container},
    window::Id,
};
use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::{
    desktop_panels::{
        panel_frame::{inward_outlined_sides, panel_frame},
        region_reporter::{PointerInput, RegionRoles, region_reporter},
    },
    panel_menus::appearance_menu::sidebar_settings_group::WorkspacesPlacement,
    workspace_switcher::sidebar_column,
};

use crate::{desktop_shell::DesktopShell, shell_message::ShellInteraction};

impl DesktopShell {
    pub(crate) fn workspace_sidebar<'view>(
        &'view self,
        window: Id,
        output: &str,
    ) -> Element<'view, ShellInteraction> {
        let sidebar = self
            .indicator_groups
            .sidebar_settings_group
            .layer
            .appearance();
        let frame_style = sidebar.frame;
        let outlined_sides = inward_outlined_sides(Position::Left)
            .all_when_floating(self.indicator_groups.panel_settings_group.float_offset_px());
        let framed_workspaces = (self
            .indicator_groups
            .sidebar_settings_group
            .workspaces_placement
            == WorkspacesPlacement::Sidebar)
            .then_some(&self.window_snapshot)
            .and_then(|snapshot| {
                sidebar_column::vertical_view(
                    snapshot,
                    output,
                    self.configured_workspaces.read_order(),
                    &self
                        .indicator_groups
                        .sidebar_settings_group
                        .empty_workspace_folding,
                    frame_style.drawn_corners(outlined_sides),
                    self.panel_slot_context(Position::Left),
                )
            })
            .map(|workspaces| {
                panel_frame(
                    container(workspaces.map(ShellInteraction::from)),
                    frame_style,
                )
                .outlined_sides(outlined_sides)
                .backdrop_blur(sidebar.backdrop_blur)
                .into()
            });
        region_reporter(
            Column::with_children(framed_workspaces),
            RegionRoles {
                pointer_input: PointerInput::Accepted,
                ..RegionRoles::default()
            },
        )
        .on_width_change(move |width| ShellInteraction::SidebarWidthChanged { window, width })
        .into()
    }
}
