use std::time::{Duration, Instant};

use sottomano_toml_config::indicator_groups::gamepad_bindings::StickDirection;

const FLICK_ENGAGE: f32 = 0.7;
const FLICK_RELEASE: f32 = 0.35;
const FLICK_REPEAT_DELAY: Duration = Duration::from_millis(450);
const FLICK_REPEAT_INTERVAL: Duration = Duration::from_millis(160);

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StickPosition {
    pub x: f32,
    pub y: f32,
}

impl StickPosition {
    fn reach(self) -> f32 {
        self.x.abs().max(self.y.abs())
    }

    fn engaged_direction(self) -> Option<StickDirection> {
        if self.reach() < FLICK_ENGAGE {
            return None;
        }
        Some(if self.x.abs() >= self.y.abs() {
            if self.x > 0.0 {
                StickDirection::Right
            } else {
                StickDirection::Left
            }
        } else if self.y > 0.0 {
            StickDirection::Up
        } else {
            StickDirection::Down
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct HeldFlick {
    direction: StickDirection,
    next_repeat: Instant,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum FlickHold {
    #[default]
    Released,
    Held(HeldFlick),
}

#[derive(Debug, Clone, Default)]
pub struct FlickTracker {
    pub position: StickPosition,
    hold: FlickHold,
}

impl FlickTracker {
    pub fn move_x(&mut self, horizontal_deflection: f32, now: Instant) -> Option<StickDirection> {
        self.move_to(
            StickPosition {
                x: horizontal_deflection,
                ..self.position
            },
            now,
        )
    }

    pub fn move_y(&mut self, vertical_deflection: f32, now: Instant) -> Option<StickDirection> {
        self.move_to(
            StickPosition {
                y: vertical_deflection,
                ..self.position
            },
            now,
        )
    }

    fn move_to(&mut self, position: StickPosition, now: Instant) -> Option<StickDirection> {
        self.position = position;
        if position.reach() < FLICK_RELEASE {
            self.hold = FlickHold::Released;
            return None;
        }
        let direction = position.engaged_direction()?;
        if let FlickHold::Held(held) = self.hold
            && held.direction == direction
        {
            return None;
        }
        self.hold = FlickHold::Held(HeldFlick {
            direction,
            next_repeat: now + FLICK_REPEAT_DELAY,
        });
        Some(direction)
    }

    pub fn repeat_due(&mut self, now: Instant) -> Option<StickDirection> {
        let engaged = self.position.engaged_direction();
        let FlickHold::Held(held) = &mut self.hold else {
            return None;
        };
        if now < held.next_repeat || engaged != Some(held.direction) {
            return None;
        }
        held.next_repeat = now + FLICK_REPEAT_INTERVAL;
        Some(held.direction)
    }

    #[must_use]
    pub fn next_repeat(&self) -> Option<Instant> {
        let engaged = self.position.engaged_direction();
        let FlickHold::Held(held) = self.hold else {
            return None;
        };
        (engaged == Some(held.direction)).then_some(held.next_repeat)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerDisplacement {
    pub dx: f64,
    pub dy: f64,
}

#[must_use]
pub fn pointer_displacement(
    position: StickPosition,
    pixels_per_second: u32,
    elapsed: Duration,
) -> PointerDisplacement {
    let precise_near_center = |deflection: f32| f64::from(deflection * deflection.abs());
    let scale = f64::from(pixels_per_second) * elapsed.as_secs_f64();
    PointerDisplacement {
        dx: precise_near_center(position.x) * scale,
        dy: -precise_near_center(position.y) * scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flick_fires_once_until_the_stick_returns_to_center() {
        let start = Instant::now();
        let mut tracker = FlickTracker::default();
        assert_eq!(tracker.move_x(0.9, start), Some(StickDirection::Right));
        assert_eq!(tracker.move_x(1.0, start), None);
        assert_eq!(tracker.move_x(0.5, start), None);
        assert_eq!(tracker.move_x(0.1, start), None);
        assert_eq!(tracker.move_x(0.9, start), Some(StickDirection::Right));
    }

    #[test]
    fn holding_a_flick_repeats_after_a_delay() {
        let start = Instant::now();
        let mut tracker = FlickTracker::default();
        tracker.move_y(-0.95, start);
        assert_eq!(tracker.repeat_due(start + Duration::from_millis(100)), None);
        assert_eq!(
            tracker.repeat_due(start + FLICK_REPEAT_DELAY),
            Some(StickDirection::Down)
        );
    }

    #[test]
    fn pushing_the_stick_up_moves_the_pointer_up_the_screen() {
        let displacement = pointer_displacement(
            StickPosition { x: 0.0, y: 1.0 },
            1000,
            Duration::from_millis(10),
        );
        assert!((displacement.dy + 10.0).abs() < 1e-9);
        assert!(displacement.dx.abs() < 1e-9);
    }
}
