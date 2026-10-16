use std::time::Duration;

use iced::{Point, Size};

use crate::readme_showcase::{compute_fixture, showcased_group::ShowcasedGroup};

pub struct Scene {
    pub name: &'static str,
    pub canvas: Size,
    pub boot_group: fn() -> ShowcasedGroup,
    pub steps: &'static [Step],
}

pub enum Step {
    PointAt { at: Point, hold: Duration },
    Press { hold: Duration },
}

impl Step {
    pub fn hold(&self) -> Duration {
        match self {
            Step::PointAt { hold, .. } | Step::Press { hold } => *hold,
        }
    }
}

pub const SCENES: &[Scene] = &[Scene {
    name: "compute-group",
    canvas: Size::new(440.0, 330.0),
    boot_group: compute_fixture::compute_group,
    steps: &[
        Step::PointAt {
            at: Point::new(300.0, 280.0),
            hold: Duration::from_millis(800),
        },
        Step::PointAt {
            at: Point::new(22.0, 16.0),
            hold: Duration::from_millis(1500),
        },
        Step::Press {
            hold: Duration::from_millis(1500),
        },
        Step::PointAt {
            at: Point::new(50.0, 16.0),
            hold: Duration::from_millis(2500),
        },
        Step::PointAt {
            at: Point::new(100.0, 16.0),
            hold: Duration::from_millis(2000),
        },
        Step::PointAt {
            at: Point::new(300.0, 280.0),
            hold: Duration::from_millis(1000),
        },
    ],
}];
