use std::time::Instant;

use iced::{Animation, Rectangle, animation::Easing, time::Duration};

use crate::{
    data::source::Tooltip,
    wayland_layers::{surface::Position, tooltip::HoverTarget},
};

const MORPH: Duration = Duration::from_millis(160);

#[derive(Debug, Clone)]
pub struct Placement {
    pub anchor: Rectangle,
    pub position: Position,
    pub output: String,
}

pub struct Occupant {
    pub target: HoverTarget,
    pub placement: Placement,
    pub output_width: Option<i32>,
}

pub struct Departed {
    pub content: Tooltip,
    pub placement: Placement,
}

impl Departed {
    pub fn new(content: Tooltip, placement: Placement) -> Self {
        Self { content, placement }
    }
}

enum Stage {
    Idle,
    Showing {
        current: Occupant,
        previous: Option<Departed>,
        leaving: bool,
    },
    Hiding {
        previous: Departed,
    },
}

pub struct TooltipTransition {
    stage: Stage,
    morph: Animation<bool>,
    seq: u64,
}

impl Default for TooltipTransition {
    fn default() -> Self {
        Self {
            stage: Stage::Idle,
            morph: settled_morph(),
            seq: 0,
        }
    }
}

fn settled_morph() -> Animation<bool> {
    Animation::new(false)
        .easing(Easing::EaseOutCubic)
        .duration(MORPH)
}

impl TooltipTransition {
    pub fn show(&mut self, current: Occupant, previous: Option<Departed>) {
        self.stage = Stage::Showing {
            current,
            previous,
            leaving: false,
        };
        self.restart();
    }

    pub fn hide(&mut self, previous: Option<Departed>) {
        self.stage = match previous {
            Some(previous) => Stage::Hiding { previous },
            None => Stage::Idle,
        };
        self.restart();
    }

    pub fn reset(&mut self) {
        self.stage = Stage::Idle;
        self.restart();
    }

    fn restart(&mut self) {
        self.morph = settled_morph().go(true, Instant::now());
        self.seq += 1;
    }

    pub fn request_leave(&mut self) {
        if let Stage::Showing { leaving, .. } = &mut self.stage {
            *leaving = true;
        }
    }

    pub fn cancel_leave(&mut self) {
        if let Stage::Showing { leaving, .. } = &mut self.stage {
            *leaving = false;
        }
    }

    pub fn is_leaving(&self) -> bool {
        matches!(self.stage, Stage::Showing { leaving: true, .. })
    }

    pub fn current(&self) -> Option<&Occupant> {
        match &self.stage {
            Stage::Showing { current, .. } => Some(current),
            Stage::Idle | Stage::Hiding { .. } => None,
        }
    }

    pub fn previous(&self) -> Option<&Departed> {
        match &self.stage {
            Stage::Showing { previous, .. } => previous.as_ref(),
            Stage::Hiding { previous } => Some(previous),
            Stage::Idle => None,
        }
    }

    pub fn placement(&self) -> Option<&Placement> {
        match &self.stage {
            Stage::Showing { current, .. } => Some(&current.placement),
            Stage::Hiding { previous } => Some(&previous.placement),
            Stage::Idle => None,
        }
    }

    pub fn seq(&self) -> u64 {
        self.seq
    }

    pub fn progress(&self) -> f32 {
        self.morph.interpolate(0.0, 1.0, Instant::now())
    }

    pub fn is_morphing(&self) -> bool {
        self.morph.is_animating(Instant::now())
    }
}
