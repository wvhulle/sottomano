use iced::time::Instant;
use sottomano_toml_config::screen_edges::Position;

use crate::hover_cards::{
    HoverAnnouncement, HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard,
};

#[derive(Debug, Clone)]
pub struct Placement {
    pub position: Position,
    pub output: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardRetention {
    UntilPointerLeaves,
    UntilDismissed,
}

pub struct Occupant {
    pub target: HoverTarget,
    pub announcement: HoverAnnouncement,
    pub placement: Placement,
    pub retention: CardRetention,
}

pub enum DepartedContent {
    Described(HoverCard),
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

#[derive(Debug, Clone, PartialEq)]
pub enum ShownTarget {
    Nothing,
    Target(HoverTarget),
}

enum Stage {
    Idle,
    Showing { current: Occupant, leaving: bool },
    Hiding { previous: Departed, since: Instant },
}

pub struct HoverCardTransition {
    stage: Stage,
}

impl Default for HoverCardTransition {
    fn default() -> Self {
        Self { stage: Stage::Idle }
    }
}

impl HoverCardTransition {
    pub fn show(&mut self, current: Occupant) {
        self.stage = Stage::Showing {
            current,
            leaving: false,
        };
    }

    pub fn hide(&mut self, previous: Option<Departed>, now: Instant) {
        self.stage = match previous {
            Some(previous) => Stage::Hiding {
                previous,
                since: now,
            },
            None => Stage::Idle,
        };
    }

    pub fn reset(&mut self) {
        self.stage = Stage::Idle;
    }

    #[must_use]
    pub fn shown_target(&self) -> ShownTarget {
        match &self.stage {
            Stage::Showing { current, .. } => ShownTarget::Target(current.target.clone()),
            Stage::Idle | Stage::Hiding { .. } => ShownTarget::Nothing,
        }
    }

    pub fn request_leave(&mut self) {
        if let Stage::Showing {
            current, leaving, ..
        } = &mut self.stage
            && current.retention == CardRetention::UntilPointerLeaves
        {
            *leaving = true;
        }
    }

    #[must_use]
    pub fn is_held_open(&self) -> bool {
        self.current()
            .is_some_and(|current| current.retention == CardRetention::UntilDismissed)
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
    pub fn shown_target_on_output(&self) -> Option<(&HoverTarget, &str)> {
        match &self.stage {
            Stage::Showing { current, .. } => {
                Some((&current.target, current.placement.output.as_str()))
            }
            Stage::Hiding { previous, .. } => {
                Some((&previous.target, previous.placement.output.as_str()))
            }
            Stage::Idle => None,
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
}
