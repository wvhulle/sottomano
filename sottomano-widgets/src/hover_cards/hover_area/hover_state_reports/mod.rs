use std::{mem::replace, sync::Arc};

use corredo::popovers::NestedMenuOpenTrigger;
use iced::Rectangle;

use super::HoverArea;
use crate::hover_cards::{
    AnchorEvent, HoverCardMessage, HoverTarget, PointerPresence, WidgetAnchor,
    laid_out_anchors::AnchorVisibility,
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum AnchorReport {
    Unreported,
    Reported {
        anchor: Rectangle,
        visibility: AnchorVisibility,
    },
}

pub(super) struct HoverState {
    target: HoverTarget,
    pub(super) pointer_presence: PointerPresence,
    anchor_report: AnchorReport,
    presence: Arc<()>,
}

impl From<HoverTarget> for HoverState {
    fn from(target: HoverTarget) -> Self {
        Self {
            target,
            pointer_presence: PointerPresence::NotPointedAt,
            anchor_report: AnchorReport::Unreported,
            presence: Arc::new(()),
        }
    }
}

impl<M> HoverArea<'_, M> {
    pub(super) fn exit_of_replaced_target(&self, state: &mut HoverState) -> Option<M> {
        if state.target == self.target {
            return None;
        }
        let replaced = replace(state, HoverState::from(self.target.clone()));
        (replaced.pointer_presence == PointerPresence::PointedAt).then(|| {
            (self.wrap)(match self.opening {
                NestedMenuOpenTrigger::Hover => HoverCardMessage::Exit {
                    target: replaced.target,
                },
                NestedMenuOpenTrigger::Press => {
                    HoverCardMessage::Anchor(AnchorEvent::HandlePointed {
                        target: replaced.target,
                        pointing: PointerPresence::NotPointedAt,
                    })
                }
            })
        })
    }

    pub(super) fn laid_out_anchor(
        &self,
        state: &mut HoverState,
        pointer_presence: PointerPresence,
        anchor: Rectangle,
        viewport: &Rectangle,
    ) -> Option<M> {
        let pointed_at = pointer_presence == PointerPresence::PointedAt;
        let reports = self.opening == NestedMenuOpenTrigger::Press
            || pointed_at
            || state.anchor_report != AnchorReport::Unreported;
        let visibility = if pointed_at || anchor.intersection(viewport).is_some() {
            AnchorVisibility::Visible
        } else {
            AnchorVisibility::Hidden
        };
        let anchor_report = AnchorReport::Reported { anchor, visibility };
        if !reports || state.anchor_report == anchor_report {
            return None;
        }
        state.anchor_report = anchor_report;
        Some((self.wrap)(HoverCardMessage::Anchor(AnchorEvent::LaidOut(
            WidgetAnchor {
                target: self.target.clone(),
                opening: self.opening,
                anchor,
                visibility,
                presence: Arc::downgrade(&state.presence),
            },
        ))))
    }

    fn enter(&self) -> M {
        (self.wrap)(HoverCardMessage::Anchor(AnchorEvent::Entered {
            target: self.target.clone(),
        }))
    }

    fn menu_handle_pointed(&self, pointing: PointerPresence) -> M {
        (self.wrap)(HoverCardMessage::Anchor(AnchorEvent::HandlePointed {
            target: self.target.clone(),
            pointing,
        }))
    }

    pub(super) fn menu_handle_pressed(&self) -> M {
        (self.wrap)(HoverCardMessage::Anchor(AnchorEvent::HandlePressed {
            target: self.target.clone(),
        }))
    }

    fn exit(&self) -> M {
        (self.wrap)(HoverCardMessage::Exit {
            target: self.target.clone(),
        })
    }

    pub(super) fn on_hover_change(
        &self,
        state: &mut HoverState,
        pointer_presence: PointerPresence,
    ) -> Option<M> {
        if pointer_presence == state.pointer_presence {
            return None;
        }
        state.pointer_presence = pointer_presence;
        match (pointer_presence, self.opening) {
            (PointerPresence::PointedAt, NestedMenuOpenTrigger::Hover) => Some(self.enter()),
            (PointerPresence::NotPointedAt, NestedMenuOpenTrigger::Hover) => Some(self.exit()),
            (pointer_presence, NestedMenuOpenTrigger::Press) => {
                Some(self.menu_handle_pointed(pointer_presence))
            }
        }
    }
}

#[cfg(test)]
mod hover_state_report_tests;
