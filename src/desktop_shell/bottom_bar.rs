use iced::{
    Alignment, Element, Length,
    widget::{Row, container, row},
    window::Id,
};

use crate::{
    desktop_shell::{
        DesktopShell, Message,
        monitor_surfaces::{Position, ShellElement},
        top_bar::{BarBand, GroupCluster},
    },
    state::WorkspacesPlacement,
    widget_groups::{
        output_group::{self, OutputMessage},
        workspaces_group,
    },
    widgets::{widget_group::WidgetGroup, window_icon::WindowAssets},
};

pub struct BottomBar<'a> {
    shell: &'a DesktopShell,
    window: Id,
    output: &'a str,
}

impl<'a> BottomBar<'a> {
    pub fn new(shell: &'a DesktopShell, window: Id, output: &'a str) -> Self {
        Self {
            shell,
            window,
            output,
        }
    }

    fn places(&self) -> Element<'a, Message> {
        let shell = self.shell;
        let config = &shell.config;
        let position = Position::Bottom;
        let display = output_group::view(self.output, config, position).map(|m| match m {
            OutputMessage::Clicked => Message::OverviewToggled,
            OutputMessage::Tooltip(t) => Message::Tooltip(t),
        });
        let workspaces = match shell.panel_settings.layout.workspaces_placement {
            WorkspacesPlacement::Sidebar => None,
            WorkspacesPlacement::BottomPanel => {
                workspaces_group::inline_view(&shell.niri_snapshot, self.output, config, position)
                    .map(|workspaces| workspaces.map(Message::from))
            }
        };
        Row::with_children([Some(display), workspaces].into_iter().flatten())
            .spacing(config.spacing.widget_spacing_px)
            .align_y(Alignment::Center)
            .into()
    }

    fn window_list(&self) -> Option<Element<'a, Message>> {
        let shell = self.shell;
        shell
            .window_group
            .view(
                &shell.niri_snapshot,
                WindowAssets {
                    entries: &shell.desktop_entries,
                    favicons: &shell.favicons,
                    fallbacks: &shell.font_fallbacks,
                },
                shell.audio_group.window_audio(),
                self.output,
                &shell.config,
                Position::Bottom,
            )
            .map(|element| element.map(|event| Message::from(event.map(Message::WindowGroup))))
    }
}

impl<'a> From<BottomBar<'a>> for Element<'a, Message> {
    fn from(bar: BottomBar<'a>) -> Self {
        let BottomBar { shell, window, .. } = bar;
        let start = GroupCluster::new(
            shell,
            window,
            ShellElement::BottomBarStart,
            vec![bar.places()],
        );
        let middle = GroupCluster::new(
            shell,
            window,
            ShellElement::BottomBarMiddle,
            bar.window_list().into_iter().collect(),
        );
        let end = GroupCluster::of_widget_groups(
            shell,
            window,
            ShellElement::BottomBarEnd,
            bottom_end_groups(shell).into(),
            Position::Bottom,
        );
        let spacing = &shell.config.spacing;
        let clusters = row![start, container(middle).center_x(Length::Fill), end]
            .spacing(spacing.group_spacing_px)
            .align_y(Alignment::Center)
            .padding([0.0, spacing.group_padding_px]);
        BarBand::new(shell, window, ShellElement::BottomBar, clusters).into()
    }
}

pub(super) fn bottom_end_groups(shell: &DesktopShell) -> [WidgetGroup<'_, Message>; 1] {
    [shell
        .tray_group
        .widget_group(&shell.config)
        .map_messages(Message::Tray)]
}
