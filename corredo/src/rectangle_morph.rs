use std::mem::replace;

use iced::{
    Animation, Rectangle,
    animation::Easing,
    time::{Duration, Instant},
};

enum MorphPhase {
    Settled,
    AwaitingFrame,
    Running {
        animation: Animation<bool>,
        now: Instant,
    },
}

pub struct RectangleMorph {
    from: Rectangle,
    current: Rectangle,
    phase: MorphPhase,
}

impl Default for RectangleMorph {
    fn default() -> Self {
        Self {
            from: Rectangle::default(),
            current: Rectangle::default(),
            phase: MorphPhase::Settled,
        }
    }
}

impl RectangleMorph {
    pub fn begin(&mut self, collapsed: Rectangle) {
        self.from = if self.current.width > 0.0 {
            self.current
        } else {
            collapsed
        };
        self.phase = MorphPhase::AwaitingFrame;
    }

    pub fn tick(&mut self, now: Instant, duration: Duration) {
        self.phase = match replace(&mut self.phase, MorphPhase::Settled) {
            MorphPhase::Settled => MorphPhase::Settled,
            MorphPhase::AwaitingFrame => MorphPhase::Running {
                animation: Animation::new(false)
                    .easing(Easing::EaseOutCubic)
                    .duration(duration)
                    .go(true, now),
                now,
            },
            MorphPhase::Running { animation, .. } => MorphPhase::Running { animation, now },
        };
    }

    fn progress(&self) -> f32 {
        match &self.phase {
            MorphPhase::Settled => 1.0,
            MorphPhase::AwaitingFrame => 0.0,
            MorphPhase::Running { animation, now } => animation.interpolate(0.0, 1.0, *now),
        }
    }

    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.progress() >= 1.0
    }

    pub fn follow(&mut self, target: Rectangle) -> Rectangle {
        self.current = interpolated(self.from, target, self.progress());
        self.current
    }

    #[must_use]
    pub fn current(&self) -> Rectangle {
        self.current
    }
}

#[must_use]
pub fn interpolated(from: Rectangle, to: Rectangle, fraction: f32) -> Rectangle {
    let mix = |start: f32, end: f32| start + (end - start) * fraction;
    Rectangle {
        x: mix(from.x, to.x),
        y: mix(from.y, to.y),
        width: mix(from.width, to.width),
        height: mix(from.height, to.height),
    }
}

#[cfg(test)]
mod tests {
    use iced::{
        Point, Rectangle, Size,
        time::{Duration, Instant},
    };

    use super::RectangleMorph;

    #[test]
    fn a_fresh_morph_starts_from_the_collapsed_rectangle() {
        let collapsed = Rectangle::new(Point::new(4.0, 4.0), Size::ZERO);
        let target = Rectangle::new(Point::ORIGIN, Size::new(100.0, 50.0));
        let mut morph = RectangleMorph::default();
        morph.begin(collapsed);
        assert_eq!(morph.follow(target), collapsed);
        assert!(!morph.is_settled());
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn a_finished_morph_rests_on_the_target() {
        let target = Rectangle::new(Point::ORIGIN, Size::new(100.0, 50.0));
        let mut morph = RectangleMorph::default();
        morph.begin(Rectangle::default());
        let started = Instant::now();
        morph.tick(started, Duration::from_millis(10));
        morph.tick(started + Duration::from_secs(1), Duration::from_millis(10));
        assert_eq!(morph.follow(target), target);
        assert!(morph.is_settled());
    }
}
