use iced::Element;
use sottomano_widgets::shell_message::Message;

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(super) fn notification_toast_layer<'a>(&'a self, output: &'a str) -> Element<'a, Message> {
        Element::from(self.notifications.anchored_toasts(
            output,
            &self.config,
            &self.theme,
            &self.shell_settings,
            self.bar_clearance(output),
            self.clock,
        ))
        .map(Message::Notification)
    }
}
