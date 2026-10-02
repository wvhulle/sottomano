use iced::{
    Alignment, Element, Length,
    widget::{Row, container, row},
    window::Id,
};

use crate::{
    desktop_shell::{
        DesktopShell, Message,
        monitor_surfaces::{Position, ShellElement},
    },
    state::WorkspacesPlacement,
    widget_groups::{
        output_group::{self, OutputMessage},
        workspaces_group,
    },
    widgets::{widget_group::WidgetGroup, window_icon::WindowAssets},
};

impl DesktopShell {
    pub(super) fn bottom_bar<'a>(&'a self, window: Id, output: &'a str) -> Element<'a, Message> {
        let start = self.group_cluster(
            window,
            ShellElement::BottomBarStart,
            vec![self.places(output)],
        );
        let middle = self.group_cluster(
            window,
            ShellElement::BottomBarMiddle,
            self.window_list(output).into_iter().collect(),
        );
        let end = self.widget_group_cluster(
            window,
            ShellElement::BottomBarEnd,
            self.bottom_end_groups().into(),
            Position::Bottom,
        );
        let spacing = &self.config.spacing;
        let clusters = row![start, container(middle).center_x(Length::Fill), end]
            .spacing(spacing.group_spacing_px)
            .align_y(Alignment::Center)
            .padding([0.0, spacing.group_padding_px]);
        self.bar_band(window, ShellElement::BottomBar, clusters)
    }

    fn places<'a>(&'a self, output: &'a str) -> Element<'a, Message> {
        let config = &self.config;
        let display = output_group::view(output, config, Position::Bottom).map(|m| match m {
            OutputMessage::Clicked => Message::OverviewToggled,
            OutputMessage::Tooltip(t) => Message::Tooltip(t),
        });
        let workspaces = match self.panel_settings.layout.workspaces_placement {
            WorkspacesPlacement::Sidebar => None,
            WorkspacesPlacement::BottomPanel => workspaces_group::inline_view(
                &self.niri_snapshot,
                output,
                config,
                Position::Bottom,
            )
            .map(|workspaces| workspaces.map(Message::from)),
        };
        Row::with_children([Some(display), workspaces].into_iter().flatten())
            .spacing(config.spacing.widget_spacing_px)
            .align_y(Alignment::Center)
            .into()
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

    pub(super) fn bottom_end_groups(&self) -> [WidgetGroup<'_, Message>; 1] {
        [self
            .tray_group
            .widget_group(&self.config)
            .map_messages(Message::Tray)]
    }
}
