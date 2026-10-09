use iced::Rectangle;

use crate::{
    desktop_shell::{
        hover_tooltips::{HoverTarget, PressHints},
        monitor_surfaces::Position,
    },
    widgets::tooltip_body::Tooltip,
};

#[derive(Debug, Clone)]
pub struct Placement {
    pub anchor: Rectangle,
    pub position: Position,
    pub output: String,
}

pub struct Occupant {
    pub target: HoverTarget,
    pub press_hints: PressHints,
    pub placement: Placement,
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
    Showing { current: Occupant, leaving: bool },
    Hiding { previous: Departed },
}

pub struct TooltipTransition {
    stage: Stage,
    seq: u64,
}

impl Default for TooltipTransition {
    fn default() -> Self {
        Self {
            stage: Stage::Idle,
            seq: 0,
        }
    }
}

impl TooltipTransition {
    pub fn show(&mut self, current: Occupant) {
        self.stage = Stage::Showing {
            current,
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
            Stage::Hiding { previous } => Some(previous),
            Stage::Idle | Stage::Showing { .. } => None,
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
}
