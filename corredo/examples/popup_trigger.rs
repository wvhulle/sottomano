use corredo::{anchored_popup::PopupSide, popup_trigger::popup_trigger};
use iced::{
    Element, Length,
    alignment::{Horizontal, Vertical},
    widget::{button, column, container, text},
};

#[derive(Debug, Clone, Copy)]
enum PopupMessage {
    Toggled,
    Dismissed,
}

#[derive(Debug, Default)]
struct PopupDemo {
    popup_open: bool,
}

impl PopupDemo {
    fn update(&mut self, message: PopupMessage) {
        self.popup_open = match message {
            PopupMessage::Toggled => !self.popup_open,
            PopupMessage::Dismissed => false,
        };
    }

    fn view(&self) -> Element<'_, PopupMessage> {
        let trigger = button(text("Toggle popup")).on_press(PopupMessage::Toggled);
        let content = column![
            text("Anchored to the button"),
            text("Press outside to close")
        ];
        container(
            popup_trigger(trigger, content, self.popup_open)
                .side(PopupSide::Below)
                .on_dismiss(PopupMessage::Dismissed),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .into()
    }
}

fn main() -> iced::Result {
    iced::run(PopupDemo::update, PopupDemo::view)
}
