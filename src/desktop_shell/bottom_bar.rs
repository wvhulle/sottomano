use std::iter::once;

use iced::{
    Alignment, Element, Length,
    widget::{container, row},
};

use crate::{
    desktop_shell::{DesktopShell, Message, monitor_surfaces::Position, top_bar::lift},
    state::WorkspacesPlacement,
    widget_groups::workspaces_group,
    widgets::window_icon::WindowAssets,
};

impl DesktopShell {
    pub(super) fn bottom_bar<'a>(&'a self, output: &'a str) -> Element<'a, Message> {
        let start = self.group_cluster(
            self.bottom_start_groups()
                .into_iter()
                .chain(once(self.inline_workspaces(output))),
        );
        let middle = self.group_cluster([self.window_list(output)]);
        let end = self.group_cluster(self.bottom_end_groups());
        let spacing = &self.config.spacing;
        let clusters = row![start, container(middle).center_x(Length::Fill), end]
            .spacing(spacing.group_spacing_px)
            .align_y(Alignment::Center)
            .padding([0.0, spacing.group_padding_px]);
        self.bar_band(clusters)
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

    fn window_list<'a>(&'a self, output: &'a str) -> Option<Element<'a, Message>> {
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

    pub(super) fn bottom_start_groups(&self) -> [Option<Element<'_, Message>>; 5] {
        let config = &self.config;
        let shell_settings = &self.shell_settings;
        let bottom = Position::Bottom;
        [
            self.shell_layout_group
                .group_content(config, &shell_settings.layout)
                .map(Message::ShellLayout)
                .view(config, bottom)
                .map(lift),
            self.panel_appearance_group
                .group_content(config)
                .map(Message::PanelAppearance)
                .view(config, bottom)
                .map(lift),
            self.overlay_group
                .group_content(config, shell_settings.overlays)
                .map(Message::Overlay)
                .view(config, bottom)
                .map(lift),
            self.surface_effects_group
                .group_content(config, shell_settings.shadow, self.niri_rules.styling())
                .map(Message::SurfaceEffects)
                .view(config, bottom)
                .map(lift),
            self.niri_window_group
                .group_content(config)
                .map(Message::NiriWindow)
                .view(config, bottom)
                .map(lift),
        ]
    }

    pub(super) fn bottom_end_groups(&self) -> [Option<Element<'_, Message>>; 1] {
        let config = &self.config;
        let bottom = Position::Bottom;
        [self
            .tray_group
            .group_content(config)
            .map(Message::Tray)
            .view(config, bottom)
            .map(lift)]
    }
}
