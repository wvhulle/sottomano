use std::{collections::BTreeMap, mem::take};

use corredo::transitions::ExpansionState;
use iced::{
    Rectangle, Task, task,
    time::{Duration, Instant},
    window::Id,
};
use iced_exwlshell::reexport::KeyboardInteractivity;
use sottomano_toml_config::screen_edges::Position;
use tokio::time::sleep;

use crate::{
    hover_cards::{
        HoverCardMessage, HoverTarget, MenuHandleIndicator, PointerPresence, WidgetAnchor,
        hover_card_transition::{
            CardRetention, Departed, HoverCardTransition, LeaveIntent, Occupant,
        },
        laid_out_anchors::{LaidOutAnchors, WidgetLiveness},
    },
    layer_surfaces::{PanelEdge, SurfaceRequest},
};

mod hover_card_rendering;
#[cfg(test)]
mod hover_card_surfaces_tests;

pub use hover_card_rendering::{HoverCardSurfaceStyle, HoverContent};

pub const HIDE_GRACE: Duration = Duration::from_millis(250);
pub(crate) const SWITCH_INTENT: Duration = Duration::from_millis(180);

pub struct PendingSwitch {
    pub parent: PanelEdge,
    pub target: HoverTarget,
    _intent_timer: task::Handle,
}

#[derive(Default)]
enum DeferredSwitch {
    #[default]
    Absent,
    Pending(PendingSwitch),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HideSchedule {
    #[default]
    Unscheduled,
    DueAt(Instant),
}

impl HideSchedule {
    #[must_use]
    pub fn elapsed_by(self, now: Instant) -> bool {
        match self {
            HideSchedule::DueAt(due) => due <= now,
            HideSchedule::Unscheduled => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum KeyboardGrab {
    #[default]
    Released,
    Held(Id),
}

#[derive(Default)]
pub struct HoverCardHost {
    pub transition: HoverCardTransition,
    deferred_switch: DeferredSwitch,
    pub hide_schedule: HideSchedule,
    pointer_over_hover_card: bool,
    keyboard_grab: KeyboardGrab,
    laid_out_anchors: LaidOutAnchors,
    toast_cards_by_output: BTreeMap<String, Vec<Rectangle>>,
}

impl HoverCardHost {
    pub fn update(&mut self, message: HoverCardMessage, now: Instant) -> Option<Instant> {
        match message {
            HoverCardMessage::Exit { target } => self.exit(&target, now),
            HoverCardMessage::EnterHoverCard => {
                self.pointer_over_hover_card = true;
                self.deferred_switch = DeferredSwitch::Absent;
                self.hide_schedule = HideSchedule::Unscheduled;
                self.transition.cancel_leave();
                None
            }
            HoverCardMessage::ExitHoverCard => {
                self.pointer_over_hover_card = false;
                Some(self.leave_after_grace(now))
            }
            HoverCardMessage::ToastCardsLaidOut {
                output,
                toast_cards,
            } => {
                self.toast_cards_by_output.insert(output, toast_cards);
                None
            }
            HoverCardMessage::Anchor(_)
            | HoverCardMessage::OnSurface { .. }
            | HoverCardMessage::DismissHeldOpen
            | HoverCardMessage::ShownWidgetDropped
            | HoverCardMessage::ConfirmShownWidgetDropped
            | HoverCardMessage::SwitchIntentElapsed => None,
        }
    }

    pub fn defer_switch(
        &mut self,
        parent: &PanelEdge,
        target: &HoverTarget,
    ) -> Option<Task<HoverCardMessage>> {
        let current = self.transition.current()?;
        if parent.position != Position::Left || &current.target == target {
            return None;
        }
        self.transition.cancel_leave();
        let (intent_elapsed, handle) = Task::future(sleep(SWITCH_INTENT))
            .map(|()| HoverCardMessage::SwitchIntentElapsed)
            .abortable();
        self.deferred_switch = DeferredSwitch::Pending(PendingSwitch {
            parent: parent.clone(),
            target: target.clone(),
            _intent_timer: handle.abort_on_drop(),
        });
        Some(intent_elapsed)
    }

    pub fn take_pending_switch(&mut self) -> Option<PendingSwitch> {
        let DeferredSwitch::Pending(pending) = take(&mut self.deferred_switch) else {
            return None;
        };
        (self
            .laid_out_anchors
            .anchored_widget_liveness(&pending.parent.output, &pending.target)
            == WidgetLiveness::Alive)
            .then_some(pending)
    }

    pub fn open(&mut self, parent: &PanelEdge, target: HoverTarget) {
        self.occupy(parent, target, CardRetention::UntilPointerLeaves);
    }

    pub fn open_until_dismissed(&mut self, parent: &PanelEdge, target: HoverTarget) {
        self.occupy(parent, target, CardRetention::UntilDismissed);
    }

    pub fn remember_anchor(
        &mut self,
        output: &str,
        widget_anchor: WidgetAnchor,
        now: Instant,
    ) -> Option<Instant> {
        let hid_the_shown_card = !widget_anchor.visible
            && self
                .transition
                .shown_target_on_output()
                .is_some_and(|(target, shown_output)| {
                    shown_output == output && *target == widget_anchor.target
                });
        self.laid_out_anchors.remember(output, widget_anchor);
        hid_the_shown_card.then(|| self.leave_after_grace(now))
    }

    pub fn set_handle_pointing(
        &mut self,
        output: &str,
        target: &HoverTarget,
        pointing: PointerPresence,
    ) {
        self.laid_out_anchors.set_pointing(output, target, pointing);
    }

    #[must_use]
    pub fn presence_on(&self, output: &str) -> HoverCardPresence {
        if self
            .transition
            .placement()
            .is_some_and(|placement| placement.output == output)
        {
            HoverCardPresence::OnThisOutput
        } else {
            HoverCardPresence::Elsewhere
        }
    }

    #[must_use]
    pub fn hover_effect(&self, target: &HoverTarget, output: &str) -> HoverEffect {
        if self.transition.shown_retention() == CardRetention::UntilDismissed
            && self.presence_on(output) == HoverCardPresence::OnThisOutput
            && self.target() != Some(target)
        {
            HoverEffect::SwitchesMenu
        } else {
            HoverEffect::KeepsMenu
        }
    }

    #[must_use]
    pub fn handle_indicator(&self, target: &HoverTarget, output: &str) -> MenuHandleIndicator {
        MenuHandleIndicator {
            pointing: self.laid_out_anchors.handle_pointing(output, target),
            menu_expansion: if self.target() == Some(target)
                && self.presence_on(output) == HoverCardPresence::OnThisOutput
            {
                ExpansionState::Expanded
            } else {
                ExpansionState::Collapsed
            },
        }
    }

    #[must_use]
    pub fn shown_anchor(&self) -> Option<Rectangle> {
        let (target, output) = self.transition.shown_target_on_output()?;
        self.laid_out_anchors.anchor_of(output, target)
    }

    #[must_use]
    pub fn shown_widget_liveness(&self) -> WidgetLiveness {
        match self.transition.current() {
            Some(current) => self
                .laid_out_anchors
                .anchored_widget_liveness(&current.placement.output, &current.target),
            None => WidgetLiveness::Alive,
        }
    }

    fn occupy(&mut self, parent: &PanelEdge, target: HoverTarget, retention: CardRetention) {
        self.deferred_switch = DeferredSwitch::Absent;
        self.pointer_over_hover_card = false;
        self.transition.cancel_leave();
        if self
            .transition
            .current()
            .is_some_and(|current| current.target == target)
        {
            return;
        }
        self.transition.show(Occupant {
            target,
            placement: parent.clone(),
            retention,
        });
    }

    fn exit(&mut self, target: &HoverTarget, now: Instant) -> Option<Instant> {
        if self.transition.shown_retention() == CardRetention::UntilDismissed {
            return None;
        }
        if let DeferredSwitch::Pending(pending) = &self.deferred_switch
            && &pending.target == target
        {
            self.deferred_switch = DeferredSwitch::Absent;
            return Some(self.leave_after_grace(now));
        }
        if self.pointer_over_hover_card
            || !self
                .transition
                .current()
                .is_some_and(|current| &current.target == target)
        {
            return None;
        }
        Some(self.leave_after_grace(now))
    }

    pub fn leave_after_grace(&mut self, now: Instant) -> Instant {
        self.transition.request_leave();
        let due = now + HIDE_GRACE;
        if self.transition.shown_retention() != CardRetention::UntilDismissed {
            self.hide_schedule = HideSchedule::DueAt(due);
        }
        due
    }

    pub fn leave_elapsed(&mut self, now: Instant, departing: Option<Departed>) {
        if self.transition.leave_intent() == LeaveIntent::Leaving
            && self.hide_schedule.elapsed_by(now)
        {
            self.hide_schedule = HideSchedule::Unscheduled;
            self.pointer_over_hover_card = false;
            self.transition.hide(departing, now);
        }
    }

    pub fn dismiss(&mut self, departing: Option<Departed>, now: Instant) {
        self.pointer_over_hover_card = false;
        if self.transition.current().is_some() {
            self.transition.hide(departing, now);
        }
    }

    pub fn reconcile_with_panels(
        &mut self,
        panel_surfaces: &BTreeMap<String, Id>,
        now: Instant,
        fade: Duration,
    ) -> Task<SurfaceRequest> {
        let finished_hiding = self
            .transition
            .hiding_since()
            .is_some_and(|since| now >= since + fade);
        let lost_its_panel = self
            .transition
            .placement()
            .is_some_and(|placement| !panel_surfaces.contains_key(&placement.output));
        if finished_hiding || lost_its_panel {
            self.transition.reset();
        }
        self.reconcile_keyboard_grab(panel_surfaces)
    }

    fn reconcile_keyboard_grab(
        &mut self,
        panel_surfaces: &BTreeMap<String, Id>,
    ) -> Task<SurfaceRequest> {
        let wanted_grab = self
            .transition
            .current()
            .filter(|current| current.retention == CardRetention::UntilDismissed)
            .and_then(|current| panel_surfaces.get(&current.placement.output).copied())
            .map_or(KeyboardGrab::Released, KeyboardGrab::Held);
        if wanted_grab == self.keyboard_grab {
            return Task::none();
        }
        let released = match self.keyboard_grab {
            KeyboardGrab::Held(surface)
                if panel_surfaces.values().any(|panel| *panel == surface) =>
            {
                Some(Task::done(SurfaceRequest::KeyboardInteractivity {
                    surface,
                    interactivity: KeyboardInteractivity::None,
                }))
            }
            KeyboardGrab::Held(_) | KeyboardGrab::Released => None,
        };
        let grabbed = match wanted_grab {
            KeyboardGrab::Held(surface) => {
                Some(Task::done(SurfaceRequest::KeyboardInteractivity {
                    surface,
                    interactivity: KeyboardInteractivity::Exclusive,
                }))
            }
            KeyboardGrab::Released => None,
        };
        self.keyboard_grab = wanted_grab;
        Task::batch(released.into_iter().chain(grabbed))
    }

    pub fn forget_toast_cards(&mut self, output: &str) {
        self.toast_cards_by_output.remove(output);
    }

    pub fn forget_output(&mut self, output: &str) {
        self.laid_out_anchors.forget_output(output);
        self.forget_toast_cards(output);
        if self
            .transition
            .placement()
            .is_some_and(|placement| placement.output == output)
        {
            self.transition.reset();
        }
    }

    #[must_use]
    pub fn target(&self) -> Option<&HoverTarget> {
        self.transition.current().map(|current| &current.target)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverCardPresence {
    OnThisOutput,
    Elsewhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverEffect {
    SwitchesMenu,
    KeepsMenu,
}
