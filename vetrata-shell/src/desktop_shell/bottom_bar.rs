use iced::Element;
use vetrata_config::screen_edges::Position;
use vetrata_state::WorkspacesPlacement;

use crate::{
    desktop_shell::{DesktopShell, Message},
    widget_groups::workspaces_group,
    widgets::widget_group::GroupContent,
};

pub(super) struct BottomBarGroups<'a> {
    pub start: [GroupContent<'a, Message>; 4],
    pub end: [GroupContent<'a, Message>; 1],
}

impl DesktopShell {
    pub(super) fn bottom_bar<'a>(&'a self, output: &'a str) -> Element<'a, Message> {
        let config = &self.config;
        let BottomBarGroups { start, end } = self.bottom_bar_groups();
        let view = |group: GroupContent<'a, Message>| group.view(config, Position::Bottom);
        self.panel_bar(
            Position::Bottom,
            start
                .map(view)
                .into_iter()
                .chain([self.inline_workspaces(output)]),
            [self.window_group_view(output)],
            end.map(view),
        )
    }

    pub(super) fn bottom_bar_groups(&self) -> BottomBarGroups<'_> {
        let config = &self.config;
        let shell_settings = &self.shell_settings;
        BottomBarGroups {
            start: [
                self.panel_settings_group
                    .group_content(config, &shell_settings.layout, &shell_settings.panels)
                    .map(Message::PanelSettings),
                self.sidebar_settings_group
                    .group_content(config, &shell_settings.layout, &shell_settings.sidebar)
                    .map(Message::SidebarSettings),
                self.overlay_settings_group
                    .group_content(
                        config,
                        shell_settings.notifications,
                        &shell_settings.overlays,
                    )
                    .map(Message::OverlaySettings),
                self.niri_window_group
                    .group_content(config, self.niri_rules.styling())
                    .map(Message::NiriWindow),
            ],
            end: [self.tray_group.group_content(config).map(Message::Tray)],
        }
    }

    fn inline_workspaces<'a>(&'a self, output: &'a str) -> Option<Element<'a, Message>> {
        match self.shell_settings.layout.workspaces_placement {
            WorkspacesPlacement::Sidebar => None,
            WorkspacesPlacement::BottomPanel => workspaces_group::inline_view(
                self.niri_snapshot.get()?,
                output,
                &self.config,
                Position::Bottom,
                &self.drag_view(),
                self.configured_workspaces.as_ref(),
            )
            .map(|workspaces| workspaces.map(Message::from)),
        }
    }

    fn window_group_view<'a>(&'a self, output: &'a str) -> Option<Element<'a, Message>> {
        self.window_group
            .view(
                self.niri_snapshot.get()?,
                self.window_assets(),
                self.audio_group.window_audio(),
                output,
                &self.config,
                Position::Bottom,
                &self.drag_view(),
            )
            .map(|element| element.map(|event| Message::from(event.map(Message::WindowGroup))))
    }
}
