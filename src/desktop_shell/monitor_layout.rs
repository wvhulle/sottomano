use iced::{
    Element, Length, Vector,
    widget::{column, container, stack},
    window::Id,
};

use crate::{
    desktop_shell::{
        DesktopShell, Message,
        bottom_bar::{BottomBar, bottom_end_groups},
        hover_tooltips::on_surface,
        monitor_surfaces::Position,
        top_bar::{TopBar, top_center_groups, top_end_groups, top_start_groups},
        workspace_sidebar::WorkspaceSidebar,
    },
    widgets::widget_group::WidgetGroup,
};

pub struct MonitorLayout<'a> {
    shell: &'a DesktopShell,
    window: Id,
    output: &'a str,
}

impl<'a> MonitorLayout<'a> {
    pub fn new(shell: &'a DesktopShell, window: Id, output: &'a str) -> Self {
        Self {
            shell,
            window,
            output,
        }
    }
}

impl<'a> From<MonitorLayout<'a>> for Element<'a, Message> {
    fn from(layout: MonitorLayout<'a>) -> Self {
        let MonitorLayout {
            shell,
            window,
            output,
        } = layout;
        let float = shell.panel_settings.layout.float_offset_px;
        let origin = Vector::new(-float, -float);
        let on_edge =
            |position, content: Element<'a, Message>| on_surface(window, position, origin, content);
        let top_bar = on_edge(Position::Top, TopBar::new(shell, window).into());
        let bottom_bar = on_edge(
            Position::Bottom,
            BottomBar::new(shell, window, output).into(),
        );
        let workspace_sidebar = container(on_edge(
            Position::Left,
            WorkspaceSidebar::new(shell, window, output).into(),
        ))
        .align_left(Length::Fill)
        .center_y(Length::Fill);
        let notification_toasts = Element::from(shell.notifications.anchored_toasts(
            window,
            output,
            &shell.config,
            &shell.theme,
            &shell.panel_settings.layout,
            shell.bar_clearance(output),
        ))
        .map(Message::Notification);
        stack![
            notification_toasts,
            column![top_bar, workspace_sidebar, bottom_bar].padding(float)
        ]
        .into()
    }
}

pub fn all_widget_groups(shell: &DesktopShell) -> impl Iterator<Item = WidgetGroup<'_, Message>> {
    top_start_groups(shell)
        .into_iter()
        .chain(top_center_groups(shell))
        .chain(top_end_groups(shell))
        .chain(bottom_end_groups(shell))
}
