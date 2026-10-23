use std::sync::LazyLock;

use iced_widget::core::{Point, Size, image::Handle};

use crate::{
    diagram_anchor::{DiagramAnchor, DiagramButton, DiagramStick},
    face_style::FaceStyle,
};

pub(crate) const BODY_SIZE: Size = Size::new(512.0, 317.0);
pub(crate) const FACE_OFFSET: Point = Point::new(363.0, 118.0);
pub(crate) const FACE_SIZE: Size = Size::new(103.0, 80.0);

static BODY: LazyLock<Handle> =
    LazyLock::new(|| Handle::from_bytes(include_bytes!("../assets/gamepad_front.png").as_slice()));
static FACE_SONY: LazyLock<Handle> = LazyLock::new(|| {
    Handle::from_bytes(include_bytes!("../assets/gamepad_face_sony.png").as_slice())
});
static FACE_ABXY: LazyLock<Handle> = LazyLock::new(|| {
    Handle::from_bytes(include_bytes!("../assets/gamepad_face_abxy.png").as_slice())
});
static FACE_BAYX: LazyLock<Handle> = LazyLock::new(|| {
    Handle::from_bytes(include_bytes!("../assets/gamepad_face_bayx.png").as_slice())
});
static FACE_AXBY: LazyLock<Handle> = LazyLock::new(|| {
    Handle::from_bytes(include_bytes!("../assets/gamepad_face_axby.png").as_slice())
});

pub(crate) fn body_image() -> Handle {
    BODY.clone()
}

pub(crate) fn face_image(face: FaceStyle) -> Option<Handle> {
    let face = match face {
        FaceStyle::PlayStation => &FACE_SONY,
        FaceStyle::XboxLetters => &FACE_ABXY,
        FaceStyle::NintendoLetters => &FACE_BAYX,
        FaceStyle::SwappedLetters => &FACE_AXBY,
        FaceStyle::Unlabeled => return None,
    };
    Some(LazyLock::force(face).clone())
}

pub(crate) const fn anchor_point(anchor: DiagramAnchor) -> Point {
    let (x, y) = match anchor {
        DiagramAnchor::Button(button) => match button {
            DiagramButton::South => (413.0, 190.0),
            DiagramButton::East => (456.0, 156.0),
            DiagramButton::West => (372.0, 159.0),
            DiagramButton::North => (415.0, 127.0),
            DiagramButton::Back => (199.0, 157.0),
            DiagramButton::Guide => (257.0, 153.0),
            DiagramButton::Start => (314.0, 157.0),
            DiagramButton::LeftStick => (98.0, 177.0),
            DiagramButton::RightStick => (331.0, 254.0),
            DiagramButton::LeftShoulder => (102.0, 65.0),
            DiagramButton::RightShoulder => (421.0, 61.0),
            DiagramButton::LeftTrigger => (116.0, 5.0),
            DiagramButton::RightTrigger => (400.0, 5.0),
            DiagramButton::DpadUp => (179.0, 213.0),
            DiagramButton::DpadDown => (179.0, 274.0),
            DiagramButton::DpadLeft => (141.0, 242.0),
            DiagramButton::DpadRight => (211.0, 242.0),
        },
        DiagramAnchor::Stick(DiagramStick::Left) => (99.0, 178.0),
        DiagramAnchor::Stick(DiagramStick::Right) => (331.0, 256.0),
    };
    Point::new(x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_anchor_lies_on_the_body_image() {
        let anchors = DiagramButton::ALL
            .into_iter()
            .map(DiagramAnchor::from)
            .chain([DiagramStick::Left, DiagramStick::Right].map(DiagramAnchor::from));
        for anchor in anchors {
            let point = anchor_point(anchor);
            assert!(
                (0.0..=BODY_SIZE.width).contains(&point.x)
                    && (0.0..=BODY_SIZE.height).contains(&point.y),
                "{anchor:?} at {point:?} is off the image"
            );
        }
    }
}
