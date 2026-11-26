use std::time::Duration;

use iced::{Point, Size, Vector, mouse::Button};
use iced_test::instruction::{Mouse, Target};
use sottomano_widgets::{
    group_indicators::indicator_group_identity::IndicatorGroupIdentity,
    group_menus::group_menu_state::{GroupMenuPin, PanelPin, PanelPinDuration},
    hover_cards::HoverTarget,
};

use crate::{
    hovered_widget::{HoveredWidget, MenuPinWidget},
    scenes::GroupShown,
    script_steps::Step,
    scripted_session::{FRAME_INTERVAL, FrameWatcher, ScriptedSession},
    showcase_program::HOVER_FETCH_LATENCY,
};

const REST_HOLD: Duration = Duration::from_millis(250);
const HOVER_HOLD: Duration = Duration::from_millis(1600);
const PRESS_LEAD: Duration = Duration::from_millis(150);
const PRESS_HOLD: Duration = Duration::from_millis(900);
const END_STATE_HOLD: Duration = Duration::from_millis(2000);
const DRAG_MOVES: u16 = 16;
const DRAG_MOVE_HOLD: Duration = Duration::from_millis(40);

pub(super) fn rest_pointer(canvas: Size) -> Point {
    Point::new(canvas.width - 1.0, canvas.height / 2.0)
}

impl<Watcher: FrameWatcher> ScriptedSession<Watcher> {
    pub fn hold(&mut self, duration: Duration) {
        let frame_count = duration.as_millis() / FRAME_INTERVAL.as_millis();
        (0..frame_count).for_each(|_| self.advance_frame());
    }

    pub fn rest(&mut self) {
        self.point_at(rest_pointer(self.canvas));
        self.hold(REST_HOLD);
    }

    pub fn hold_end_state(&mut self) {
        self.hold(END_STATE_HOLD);
    }

    pub fn press_away(&mut self) {
        self.point_at(rest_pointer(self.canvas));
        self.click();
        self.hold(REST_HOLD);
    }

    fn press_at(&mut self, pointer: Point) {
        self.point_at(pointer);
        self.hold(PRESS_LEAD);
        self.click();
        self.hold(PRESS_HOLD);
    }

    pub fn perform(&mut self, step: &Step) {
        match step {
            Step::Hover(target) => {
                let pointer = self.locate(HoveredWidget {
                    hover_target: target.clone(),
                });
                self.point_at(pointer);
                self.hold(self.hover_hold_for(target));
            }
            Step::Press(target) => {
                let pointer = self.locate(HoveredWidget {
                    hover_target: target.clone(),
                });
                self.press_at(pointer);
            }
            Step::HoverFace(group) => self.perform(&Step::Hover(self.face_target(group))),
            Step::PressFace(group) => self.perform(&Step::Press(self.face_target(group))),
            Step::PressPin(entry) => {
                let pointer = self.locate(MenuPinWidget {
                    menu_pin: entry.clone(),
                });
                self.press_at(pointer);
            }
            Step::HoverText(shown) => {
                self.interact(Mouse::Move(Target::Text((*shown).to_owned())));
                self.hold(HOVER_HOLD);
            }
            Step::PressText(shown) => {
                self.interact(Mouse::Move(Target::Text((*shown).to_owned())));
                self.hold(PRESS_LEAD);
                self.interact(Mouse::Click {
                    button: Button::Left,
                    target: Some(Target::Text((*shown).to_owned())),
                });
                self.hold(PRESS_HOLD);
            }
            Step::RestFor(duration) => {
                self.point_at(rest_pointer(self.canvas));
                self.hold(*duration);
            }
            Step::DragFace { group, by } => {
                let grabbed = self.locate(HoveredWidget {
                    hover_target: self.face_target(group),
                });
                self.point_at(grabbed);
                self.hold(PRESS_LEAD);
                self.interact(Mouse::Press {
                    button: Button::Left,
                    target: Some(Target::Point(grabbed)),
                });
                (1..=DRAG_MOVES).for_each(|moved| {
                    let progress = f32::from(moved) / f32::from(DRAG_MOVES);
                    self.point_at(grabbed + Vector::new(by * progress, 0.0));
                    self.hold(DRAG_MOVE_HOLD);
                });
                self.interact(Mouse::Release {
                    button: Button::Left,
                    target: Some(Target::Point(self.pointer)),
                });
                self.hold(HOVER_HOLD);
            }
        }
    }

    fn face_target(&self, group: &IndicatorGroupIdentity) -> HoverTarget {
        match &self.scene.shown {
            GroupShown::PinnedFrom {
                menu,
                group: pinned_group,
            } if pinned_group == group => HoverTarget::PinnedOnPanel(Box::new(PanelPin {
                menu: *menu,
                entry: GroupMenuPin::Group(group.clone()),
                duration: PanelPinDuration::Permanent,
            })),
            _ => HoverTarget::GroupHeadline(Box::new(group.clone())),
        }
    }

    fn hover_hold_for(&self, target: &HoverTarget) -> Duration {
        let fetch_replies =
            (self.scene.group)(self.clock).start_hovered_fetch(target, &self.config, self.clock);
        if fetch_replies.is_empty() {
            HOVER_HOLD
        } else {
            HOVER_FETCH_LATENCY + HOVER_HOLD
        }
    }
}
