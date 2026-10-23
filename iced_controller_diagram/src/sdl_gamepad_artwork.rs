use std::io::Cursor;

use iced_widget::core::{Point, Size, image::Handle};
use png::{Decoder, Transformations};

use crate::{
    diagram_anchor::{DiagramAnchor, DiagramButton, DiagramStick},
    face_style::FaceStyle,
};

pub(crate) const BODY_SIZE: Size = Size::new(512.0, 317.0);
pub(crate) const FACE_OFFSET: Point = Point::new(363.0, 118.0);
pub(crate) const FACE_SIZE: Size = Size::new(103.0, 80.0);

const BODY_PNG: &[u8] = include_bytes!("../assets/gamepad_front.png");
const FACE_SONY_PNG: &[u8] = include_bytes!("../assets/gamepad_face_sony.png");
const FACE_ABXY_PNG: &[u8] = include_bytes!("../assets/gamepad_face_abxy.png");
const FACE_BAYX_PNG: &[u8] = include_bytes!("../assets/gamepad_face_bayx.png");
const FACE_AXBY_PNG: &[u8] = include_bytes!("../assets/gamepad_face_axby.png");

fn decoded_rgba(png_bytes: &[u8]) -> Handle {
    let mut decoder = Decoder::new(Cursor::new(png_bytes));
    decoder.set_transformations(
        Transformations::EXPAND | Transformations::STRIP_16 | Transformations::ALPHA,
    );
    let mut reader = decoder
        .read_info()
        .expect("bundled controller artwork is a valid PNG");
    let mut pixels = vec![
        0;
        reader
            .output_buffer_size()
            .expect("bundled controller artwork fits in memory")
    ];
    let frame = reader
        .next_frame(&mut pixels)
        .expect("bundled controller artwork decodes");
    pixels.truncate(frame.buffer_size());
    Handle::from_rgba(frame.width, frame.height, pixels)
}

#[derive(Debug, Clone)]
pub struct ControllerArtwork {
    body: Handle,
    sony: Handle,
    abxy: Handle,
    bayx: Handle,
    axby: Handle,
}

impl ControllerArtwork {
    #[must_use]
    pub fn decoded() -> Self {
        Self {
            body: decoded_rgba(BODY_PNG),
            sony: decoded_rgba(FACE_SONY_PNG),
            abxy: decoded_rgba(FACE_ABXY_PNG),
            bayx: decoded_rgba(FACE_BAYX_PNG),
            axby: decoded_rgba(FACE_AXBY_PNG),
        }
    }

    pub(crate) fn body(&self) -> &Handle {
        &self.body
    }

    pub(crate) fn face(&self, face: FaceStyle) -> Option<&Handle> {
        match face {
            FaceStyle::PlayStation => Some(&self.sony),
            FaceStyle::XboxLetters => Some(&self.abxy),
            FaceStyle::NintendoLetters => Some(&self.bayx),
            FaceStyle::SwappedLetters => Some(&self.axby),
            FaceStyle::Unlabeled => None,
        }
    }
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

    fn pixel_size(handle: &Handle) -> Size<u32> {
        let Handle::Rgba {
            width,
            height,
            pixels,
            ..
        } = handle
        else {
            panic!("artwork should be decoded to RGBA");
        };
        assert_eq!(pixels.len(), (width * height * 4) as usize);
        Size::new(*width, *height)
    }

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

    #[test]
    fn decoded_artwork_matches_the_layout_sizes() {
        let artwork = ControllerArtwork::decoded();
        assert_eq!(pixel_size(artwork.body()), Size::new(512, 317));
        for face in [
            FaceStyle::PlayStation,
            FaceStyle::XboxLetters,
            FaceStyle::NintendoLetters,
            FaceStyle::SwappedLetters,
        ] {
            let handle = artwork.face(face).expect("labelled faces have an overlay");
            assert_eq!(pixel_size(handle), Size::new(103, 80));
        }
        assert!(artwork.face(FaceStyle::Unlabeled).is_none());
    }
}
