use iced::{
    Element, Length,
    widget::{button, column, container, row, text},
};
use iced_controller_diagram::{DiagramButton, DiagramStick, FaceStyle, controller_diagram};

const FACES: [(&str, FaceStyle); 5] = [
    ("PlayStation", FaceStyle::PlayStation),
    ("Xbox", FaceStyle::XboxLetters),
    ("Nintendo", FaceStyle::NintendoLetters),
    ("Swapped letters", FaceStyle::SwappedLetters),
    ("Unlabeled", FaceStyle::Unlabeled),
];

#[derive(Debug, Clone, Copy)]
enum GalleryMessage {
    FaceChosen(FaceStyle),
}

#[derive(Default)]
struct DiagramGallery {
    face: FaceStyle,
}

impl DiagramGallery {
    fn update(&mut self, message: GalleryMessage) {
        let GalleryMessage::FaceChosen(face) = message;
        self.face = face;
    }

    fn view(&self) -> Element<'_, GalleryMessage> {
        let face_choices = row(FACES.map(|(name, face)| {
            button(text(name))
                .on_press(GalleryMessage::FaceChosen(face))
                .into()
        }))
        .spacing(8);
        let diagram = controller_diagram(self.face)
            .scale(0.6)
            .callout(DiagramButton::LeftTrigger, text("dictate"))
            .callout(DiagramButton::RightTrigger, text("click"))
            .callout(DiagramButton::LeftShoulder, text("column ←"))
            .callout(DiagramButton::RightShoulder, text("column →"))
            .callout(DiagramStick::Left, text("navigate"))
            .callout(DiagramStick::Right, text("pointer"))
            .callout(DiagramButton::DpadUp, text("Up"))
            .callout(DiagramButton::DpadDown, text("Down"))
            .callout(DiagramButton::DpadLeft, text("Left"))
            .callout(DiagramButton::DpadRight, text("Right"))
            .callout(DiagramButton::South, text("Return"))
            .callout(DiagramButton::East, text("Escape"))
            .callout(DiagramButton::North, text("maximize"))
            .callout(DiagramButton::Guide, text("overview"))
            .callout(DiagramButton::Back, text("hand over"))
            .callout(DiagramButton::Start, text("hand over"));
        container(column![face_choices, diagram].spacing(24))
            .padding(24)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

fn main() -> iced::Result {
    iced::application(
        DiagramGallery::default,
        DiagramGallery::update,
        DiagramGallery::view,
    )
    .run()
}
