use iced::{Rectangle, time::Instant};
use sottomano_config::screen_edges::Position;

use crate::{
    hover_tooltips::{HoverAnnouncement, HoverTarget},
    widgets::tooltip_body::tooltip_source_delivery::Tooltip,
};

#[derive(Debug, Clone)]
pub struct Placement {
    pub anchor: Rectangle,
    pub position: Position,
    pub output: String,
}

pub struct Occupant {
    pub target: HoverTarget,
    pub announcement: HoverAnnouncement,
    pub placement: Placement,
}

pub enum DepartedContent {
    Described(Tooltip),
    Interactive,
}

pub struct Departed {
    pub target: HoverTarget,
    pub content: DepartedContent,
    pub placement: Placement,
}

impl Departed {
    #[must_use]
    pub fn new(target: HoverTarget, content: DepartedContent, placement: Placement) -> Self {
        Self {
            target,
            content,
            placement,
        }
    }
}

enum Stage {
    Idle,
    Showing { current: Occupant, leaving: bool },
    Hiding { previous: Departed, since: Instant },
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

    pub fn hide(&mut self, previous: Option<Departed>, now: Instant) {
        self.stage = match previous {
            Some(previous) => Stage::Hiding {
                previous,
                since: now,
            },
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

    #[must_use]
    pub fn hiding_since(&self) -> Option<Instant> {
        match self.stage {
            Stage::Hiding { since, .. } => Some(since),
            Stage::Idle | Stage::Showing { .. } => None,
        }
    }

    #[must_use]
    pub fn is_leaving(&self) -> bool {
        matches!(self.stage, Stage::Showing { leaving: true, .. })
    }

    pub fn revise_announcement(&mut self, target: &HoverTarget, announcement: HoverAnnouncement) {
        if let Stage::Showing { current, .. } = &mut self.stage
            && current.target == *target
        {
            current.announcement = announcement;
        }
    }

    #[must_use]
    pub fn current(&self) -> Option<&Occupant> {
        match &self.stage {
            Stage::Showing { current, .. } => Some(current),
            Stage::Idle | Stage::Hiding { .. } => None,
        }
    }

    #[must_use]
    pub fn previous(&self) -> Option<&Departed> {
        match &self.stage {
            Stage::Hiding { previous, .. } => Some(previous),
            Stage::Idle | Stage::Showing { .. } => None,
        }
    }

    #[must_use]
    pub fn placement(&self) -> Option<&Placement> {
        match &self.stage {
            Stage::Showing { current, .. } => Some(&current.placement),
            Stage::Hiding { previous, .. } => Some(&previous.placement),
            Stage::Idle => None,
        }
    }

    #[must_use]
    pub fn seq(&self) -> u64 {
        self.seq
    }
}
