use corredo::{
    orientation::Edge,
    popovers::{PopupVisibility, popup_trigger},
};
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
    popup_visibility: PopupVisibility,
}

impl PopupDemo {
    fn update(&mut self, message: PopupMessage) {
        self.popup_visibility = match (message, self.popup_visibility) {
            (PopupMessage::Toggled, PopupVisibility::Closed) => PopupVisibility::Open,
            (PopupMessage::Toggled, PopupVisibility::Open) | (PopupMessage::Dismissed, _) => {
                PopupVisibility::Closed
            }
        };
    }

    fn view(&self) -> Element<'_, PopupMessage> {
        let trigger = button(text("Toggle popup")).on_press(PopupMessage::Toggled);
        let content = column![
            text("Anchored to the button"),
            text("Press outside to close")
        ];
        container(
            popup_trigger(trigger, content, self.popup_visibility)
                .side(Edge::Bottom)
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
