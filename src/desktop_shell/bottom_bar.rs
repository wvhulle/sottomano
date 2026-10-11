use iced::Element;

use crate::{
    desktop_shell::{DesktopShell, Message, monitor_surfaces::Position},
    state::WorkspacesPlacement,
    widget_groups::workspaces_group,
    widgets::{widget_group::GroupContent, window_icon::WindowAssets},
};

pub(super) struct BottomBarGroups<'a> {
    pub start: [GroupContent<'a, Message>; 5],
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
                self.shell_layout_group
                    .group_content(config, &shell_settings.layout)
                    .map(Message::ShellLayout),
                self.panel_appearance_group
                    .group_content(config)
                    .map(Message::PanelAppearance),
                self.overlay_group
                    .group_content(config, shell_settings.overlays)
                    .map(Message::Overlay),
                self.surface_effects_group
                    .group_content(
                        config,
                        shell_settings.shadow,
                        shell_settings.outline,
                        self.niri_rules.styling(),
                    )
                    .map(Message::SurfaceEffects),
                self.niri_window_group
                    .group_content(config)
                    .map(Message::NiriWindow),
            ],
            end: [self.tray_group.group_content(config).map(Message::Tray)],
        }
    }

    fn inline_workspaces<'a>(&'a self, output: &'a str) -> Option<Element<'a, Message>> {
        match self.shell_settings.layout.workspaces_placement {
            WorkspacesPlacement::Sidebar => None,
            WorkspacesPlacement::BottomPanel => workspaces_group::inline_view(
                &self.niri_snapshot,
                output,
                &self.config,
                Position::Bottom,
            )
            .map(|workspaces| workspaces.map(Message::from)),
        }
    }

    fn window_group_view<'a>(&'a self, output: &'a str) -> Option<Element<'a, Message>> {
        self.window_group
            .view(
                &self.niri_snapshot,
                WindowAssets {
                    entries: &self.desktop_entries,
                    favicons: &self.favicons,
                    fallbacks: &self.font_fallbacks,
                },
                self.audio_group.window_audio(),
                output,
                &self.config,
                Position::Bottom,
            )
            .map(|element| element.map(|event| Message::from(event.map(Message::WindowGroup))))
    }
}
