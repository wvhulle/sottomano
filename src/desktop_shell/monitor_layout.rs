use iced::{
    Element, Length, Vector,
    widget::{column, container, stack},
    window::Id,
};

use crate::desktop_shell::{
    DesktopShell, Message, hover_tooltips::on_surface, monitor_surfaces::Position,
};

impl DesktopShell {
    pub(super) fn monitor_layout<'a>(
        &'a self,
        window: Id,
        output: &'a str,
    ) -> Element<'a, Message> {
        let float = self.shell_settings.layout.float_offset_px;
        let origin = Vector::new(-float, -float);
        let on_edge = |position, content| on_surface(window, position, origin, content);
        let workspace_sidebar = container(on_edge(
            Position::Left,
            self.workspace_sidebar(window, output),
        ))
        .align_left(Length::Fill)
        .center_y(Length::Fill);
        let notification_toasts = Element::from(self.notifications.anchored_toasts(
            output,
            &self.config,
            &self.theme,
            &self.shell_settings,
            self.bar_clearance(output),
        ))
        .map(Message::Notification);
        stack![
            notification_toasts,
            column![
                on_edge(Position::Top, self.top_bar()),
                workspace_sidebar,
                on_edge(Position::Bottom, self.bottom_bar(output)),
            ]
            .padding(float)
        ]
        .into()
    }
}
