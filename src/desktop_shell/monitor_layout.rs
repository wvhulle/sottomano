use iced::{
    Element, Length, Vector,
    widget::{column, container, stack},
    window::Id,
};

use crate::{
    desktop_shell::{
        DesktopShell, Message, hover_tooltips::on_surface, monitor_surfaces::Position,
    },
    widgets::widget_group::WidgetGroup,
};

impl DesktopShell {
    pub(super) fn monitor_layout<'a>(&'a self, window: Id, output: &'a str) -> Element<'a, Message> {
        let float = self.panel_settings.layout.float_offset_px;
        let origin = Vector::new(-float, -float);
        let on_edge = |position, content| on_surface(window, position, origin, content);
        let workspace_sidebar = container(on_edge(
            Position::Left,
            self.workspace_sidebar(window, output),
        ))
        .align_left(Length::Fill)
        .center_y(Length::Fill);
        let notification_toasts = Element::from(self.notifications.anchored_toasts(
            window,
            output,
            &self.config,
            &self.theme,
            &self.panel_settings.layout,
            self.bar_clearance(output),
        ))
        .map(Message::Notification);
        stack![
            notification_toasts,
            column![
                on_edge(Position::Top, self.top_bar(window)),
                workspace_sidebar,
                on_edge(Position::Bottom, self.bottom_bar(window, output)),
            ]
            .padding(float)
        ]
        .into()
    }

    pub(super) fn all_widget_groups(&self) -> impl Iterator<Item = WidgetGroup<'_, Message>> {
        self.top_start_groups()
            .into_iter()
            .chain(self.top_center_groups())
            .chain(self.top_end_groups())
            .chain(self.bottom_end_groups())
    }
}
