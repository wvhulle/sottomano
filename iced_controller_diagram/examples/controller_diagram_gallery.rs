use iced::{
    Element, Length,
    widget::{button, column, container, row, text},
};
use iced_controller_diagram::{
    ControllerArtwork, ControllerModel, DiagramButton, DiagramStick, controller_diagram,
};

const MODELS: [(&str, ControllerModel); 3] = [
    ("DualSense", ControllerModel::DualSense),
    ("Xbox Series", ControllerModel::XboxSeries),
    ("Switch Pro", ControllerModel::SwitchPro),
];

#[derive(Debug, Clone, Copy)]
enum GalleryMessage {
    ModelChosen(ControllerModel),
}

struct DiagramGallery {
    artwork: ControllerArtwork,
    model: ControllerModel,
}

impl DiagramGallery {
    fn new() -> Self {
        Self {
            artwork: ControllerArtwork::decoded(),
            model: ControllerModel::default(),
        }
    }

    fn update(&mut self, message: GalleryMessage) {
        let GalleryMessage::ModelChosen(model) = message;
        self.model = model;
    }

    fn view(&self) -> Element<'_, GalleryMessage> {
        let model_choices = row(MODELS.map(|(name, model)| {
            button(text(name))
                .on_press(GalleryMessage::ModelChosen(model))
                .into()
        }))
        .spacing(8);
        let diagram = controller_diagram(self.artwork.clone(), self.model)
            .body_width(420.0)
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
            .callout(DiagramButton::West, text("close"))
            .callout(DiagramButton::Guide, text("overview"))
            .callout(DiagramButton::Back, text("hand over"))
            .callout(DiagramButton::Start, text("hand over"))
            .callout(DiagramButton::LeftStick, text("left click"))
            .callout(DiagramButton::RightStick, text("right click"));
        container(column![model_choices, diagram].spacing(24))
            .padding(24)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

fn main() -> iced::Result {
    iced::application(
        DiagramGallery::new,
        DiagramGallery::update,
        DiagramGallery::view,
    )
    .run()
}
