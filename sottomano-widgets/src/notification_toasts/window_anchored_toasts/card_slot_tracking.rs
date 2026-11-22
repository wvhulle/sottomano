use std::{collections::HashMap, convert::identity};

use corredo::transitions::MotionState;
use iced::{
    Animation, Point,
    advanced::widget::Tree,
    animation::Easing,
    time::{Duration, Instant},
};

struct Drift {
    x: Animation<f32>,
    y: Animation<f32>,
    target: Point,
    at: Point,
}

impl Drift {
    fn resting_at(point: Point, duration: Duration) -> Self {
        Self {
            x: motion(point.x, duration),
            y: motion(point.y, duration),
            target: point,
            at: point,
        }
    }

    fn advance(&mut self, frame: Instant) {
        self.x.go_mut(self.target.x, frame);
        self.y.go_mut(self.target.y, frame);
        self.at = Point::new(
            self.x.interpolate_with(identity, frame),
            self.y.interpolate_with(identity, frame),
        );
    }
}

fn motion(value: f32, duration: Duration) -> Animation<f32> {
    Animation::new(value)
        .easing(Easing::EaseOutCubic)
        .duration(duration)
}

#[derive(Default)]
pub(super) struct ChildOrder {
    keys: Vec<u32>,
}

impl ChildOrder {
    pub(super) fn rekey(&mut self, ids: &[u32], children: &mut Vec<Tree>) {
        if self.keys == ids {
            return;
        }
        let mut retired: Vec<Option<Tree>> = children.drain(..).map(Some).collect();
        *children = ids
            .iter()
            .map(|id| {
                self.keys
                    .iter()
                    .position(|key| key == id)
                    .and_then(|index| retired[index].take())
                    .unwrap_or_else(Tree::empty)
            })
            .collect();
        self.keys.clear();
        self.keys.extend_from_slice(ids);
    }
}

#[derive(Default)]
pub(super) struct Drifts {
    by_id: HashMap<u32, Drift>,
}

impl Drifts {
    pub(super) fn aim(&mut self, ids: &[u32], targets: &[Point], motion: Duration) -> Vec<Point> {
        self.by_id.retain(|id, _| ids.contains(id));
        ids.iter()
            .zip(targets)
            .map(|(id, target)| {
                let drift = self
                    .by_id
                    .entry(*id)
                    .or_insert_with(|| Drift::resting_at(*target, motion));
                drift.target = *target;
                drift.at
            })
            .collect()
    }

    pub(super) fn advance(&mut self, frame: Instant) -> MotionState {
        self.by_id
            .values_mut()
            .for_each(|drift| drift.advance(frame));
        if self
            .by_id
            .values()
            .any(|drift| drift.x.is_animating(frame) || drift.y.is_animating(frame))
        {
            MotionState::Moving
        } else {
            MotionState::Settled
        }
    }
}
