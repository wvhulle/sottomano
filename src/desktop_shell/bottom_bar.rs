use iced::{
    Alignment, Element, Length,
    widget::{container, row},
};

use crate::{
    desktop_shell::{
        DesktopShell, Message, monitor_surfaces::Position, top_bar::rendered_widget_groups,
    },
    state::WorkspacesPlacement,
    widget_groups::workspaces_group,
    widgets::{widget_group::GroupViewWithAlerts, window_icon::WindowAssets},
};

impl DesktopShell {
    pub(super) fn bottom_bar<'a>(&'a self, output: &'a str) -> Element<'a, Message> {
        let start = self.group_cluster(
            rendered_widget_groups(self.bottom_start_groups())
                .chain(self.inline_workspaces(output))
                .collect(),
        );
        let middle = self.group_cluster(self.window_list(output).into_iter().collect());
        let end = self.widget_group_cluster(self.bottom_end_groups().into());
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

    pub(super) fn bottom_start_groups(&self) -> [GroupViewWithAlerts<'_, Message>; 5] {
        let config = &self.config;
        let shell_settings = &self.shell_settings;
        let bottom = Position::Bottom;
        [
            self.shell_layout_group
                .widget_group(config, &shell_settings.layout)
                .place_on_bar(Message::ShellLayout, config, bottom),
            self.panel_appearance_group
                .widget_group(config)
                .place_on_bar(Message::PanelAppearance, config, bottom),
            self.overlay_group
                .widget_group(config, shell_settings.overlays)
                .place_on_bar(Message::Overlay, config, bottom),
            self.surface_effects_group
                .widget_group(config, shell_settings.shadow, self.niri_rules.styling())
                .place_on_bar(Message::SurfaceEffects, config, bottom),
            self.niri_window_group.widget_group(config).place_on_bar(
                Message::NiriWindow,
                config,
                bottom,
            ),
        ]
    }

    pub(super) fn bottom_end_groups(&self) -> [GroupViewWithAlerts<'_, Message>; 1] {
        [self.tray_group.widget_group(&self.config).place_on_bar(
            Message::Tray,
            &self.config,
            Position::Bottom,
        )]
    }
}
