use iced_widget::core::{Point, Size, svg::Handle};

use crate::{
    controller_model::ControllerModel,
    diagram_anchor::{DiagramAnchor, DiagramButton, DiagramStick},
};

const DUALSENSE_SVG: &[u8] = include_bytes!("../assets/dualsense_front.svg");
const XBOX_SERIES_SVG: &[u8] = include_bytes!("../assets/xbox_series_front.svg");
const SWITCH_PRO_SVG: &[u8] = include_bytes!("../assets/switch_pro_front.svg");

#[derive(Debug, Clone)]
pub struct ControllerArtwork {
    dualsense: Handle,
    xbox_series: Handle,
    switch_pro: Handle,
}

impl ControllerArtwork {
    #[must_use]
    pub fn decoded() -> Self {
        Self {
            dualsense: Handle::from_memory(DUALSENSE_SVG),
            xbox_series: Handle::from_memory(XBOX_SERIES_SVG),
            switch_pro: Handle::from_memory(SWITCH_PRO_SVG),
        }
    }

    pub(crate) fn front_view(&self, model: ControllerModel) -> &Handle {
        match model {
            ControllerModel::DualSense => &self.dualsense,
            ControllerModel::XboxSeries => &self.xbox_series,
            ControllerModel::SwitchPro => &self.switch_pro,
        }
    }
}

pub(crate) const fn body_size(model: ControllerModel) -> Size {
    match model {
        ControllerModel::DualSense => Size::new(2058.7, 1144.9),
        ControllerModel::XboxSeries => Size::new(1534.7, 954.0),
        ControllerModel::SwitchPro => Size::new(1584.1, 1151.0),
    }
}

pub(crate) const fn anchor_point(model: ControllerModel, anchor: DiagramAnchor) -> Point {
    let (x, y) = match model {
        ControllerModel::DualSense => dualsense_anchor(anchor),
        ControllerModel::XboxSeries => xbox_series_anchor(anchor),
        ControllerModel::SwitchPro => switch_pro_anchor(anchor),
    };
    Point::new(x, y)
}

const fn dualsense_anchor(anchor: DiagramAnchor) -> (f32, f32) {
    match anchor {
        DiagramAnchor::Button(button) => match button {
            DiagramButton::South => (1688.0, 729.0),
            DiagramButton::East => (1848.0, 598.0),
            DiagramButton::West => (1537.0, 604.0),
            DiagramButton::North => (1697.0, 472.0),
            DiagramButton::Back => (529.0, 402.0),
            DiagramButton::Guide => (1034.0, 804.0),
            DiagramButton::Start => (1532.0, 402.0),
            DiagramButton::LeftStick => (692.0, 890.0),
            DiagramButton::RightStick => (1368.0, 891.0),
            DiagramButton::LeftShoulder => (367.0, 206.0),
            DiagramButton::RightShoulder => (1692.0, 206.0),
            DiagramButton::LeftTrigger => (380.0, 60.0),
            DiagramButton::RightTrigger => (1679.0, 60.0),
            DiagramButton::DpadUp => (366.0, 519.0),
            DiagramButton::DpadDown => (367.0, 686.0),
            DiagramButton::DpadLeft => (263.0, 603.0),
            DiagramButton::DpadRight => (470.0, 603.0),
        },
        DiagramAnchor::Stick(DiagramStick::Left) => (692.0, 890.0),
        DiagramAnchor::Stick(DiagramStick::Right) => (1368.0, 891.0),
    }
}

const fn xbox_series_anchor(anchor: DiagramAnchor) -> (f32, f32) {
    match anchor {
        DiagramAnchor::Button(button) => match button {
            DiagramButton::South => (1178.0, 514.0),
            DiagramButton::East => (1287.0, 407.0),
            DiagramButton::West => (1071.0, 418.0),
            DiagramButton::North => (1182.0, 311.0),
            DiagramButton::Back => (650.0, 420.0),
            DiagramButton::Guide => (767.0, 266.0),
            DiagramButton::Start => (884.0, 420.0),
            DiagramButton::LeftStick => (352.0, 443.0),
            DiagramButton::RightStick => (976.0, 671.0),
            DiagramButton::LeftShoulder => (300.0, 150.0),
            DiagramButton::RightShoulder => (1235.0, 150.0),
            DiagramButton::LeftTrigger => (306.0, 50.0),
            DiagramButton::RightTrigger => (1216.0, 50.0),
            DiagramButton::DpadUp => (558.0, 595.0),
            DiagramButton::DpadDown => (558.0, 725.0),
            DiagramButton::DpadLeft => (475.0, 660.0),
            DiagramButton::DpadRight => (640.0, 660.0),
        },
        DiagramAnchor::Stick(DiagramStick::Left) => (352.0, 443.0),
        DiagramAnchor::Stick(DiagramStick::Right) => (976.0, 671.0),
    }
}

const fn switch_pro_anchor(anchor: DiagramAnchor) -> (f32, f32) {
    match anchor {
        DiagramAnchor::Button(button) => match button {
            DiagramButton::South => (1208.0, 476.0),
            DiagramButton::East => (1332.0, 368.0),
            DiagramButton::West => (1085.0, 368.0),
            DiagramButton::North => (1208.0, 258.0),
            DiagramButton::Back => (598.0, 248.0),
            DiagramButton::Guide => (903.0, 367.0),
            DiagramButton::Start => (986.0, 248.0),
            DiagramButton::LeftStick => (356.0, 367.0),
            DiagramButton::RightStick => (998.0, 587.0),
            DiagramButton::LeftShoulder => (372.0, 120.0),
            DiagramButton::RightShoulder => (1211.0, 120.0),
            DiagramButton::LeftTrigger => (362.0, 40.0),
            DiagramButton::RightTrigger => (1221.0, 40.0),
            DiagramButton::DpadUp => (550.0, 510.0),
            DiagramButton::DpadDown => (550.0, 665.0),
            DiagramButton::DpadLeft => (470.0, 587.0),
            DiagramButton::DpadRight => (630.0, 587.0),
        },
        DiagramAnchor::Stick(DiagramStick::Left) => (356.0, 367.0),
        DiagramAnchor::Stick(DiagramStick::Right) => (998.0, 587.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_anchor() -> impl Iterator<Item = DiagramAnchor> {
        DiagramButton::ALL
            .into_iter()
            .map(DiagramAnchor::from)
            .chain([DiagramStick::Left, DiagramStick::Right].map(DiagramAnchor::from))
    }

    #[test]
    fn every_anchor_lies_on_its_controller_drawing() {
        for model in ControllerModel::ALL {
            let size = body_size(model);
            for anchor in every_anchor() {
                let point = anchor_point(model, anchor);
                assert!(
                    (0.0..=size.width).contains(&point.x) && (0.0..=size.height).contains(&point.y),
                    "{model:?} {anchor:?} at {point:?} is off the drawing"
                );
            }
        }
    }

    #[test]
    fn bundled_drawings_are_svg_documents() {
        for drawing in [DUALSENSE_SVG, XBOX_SERIES_SVG, SWITCH_PRO_SVG] {
            let head = String::from_utf8_lossy(&drawing[..drawing.len().min(512)]);
            assert!(head.contains("<svg"), "bundled drawing lacks an svg root");
        }
    }

    #[test]
    fn body_sizes_match_the_drawing_aspect_ratios() {
        let view_boxes = [
            (ControllerModel::DualSense, Size::new(544.7, 302.9)),
            (ControllerModel::XboxSeries, Size::new(1534.7, 954.0)),
            (ControllerModel::SwitchPro, Size::new(419.1, 304.5)),
        ];
        for (model, view_box) in view_boxes {
            let body = body_size(model);
            let ratio_gap = body.width / body.height - view_box.width / view_box.height;
            assert!(
                ratio_gap.abs() < 0.005,
                "{model:?} aspect differs by {ratio_gap}"
            );
        }
    }
}
