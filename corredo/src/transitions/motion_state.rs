use iced::{
    Animation,
    animation::{Easing, Float},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionState {
    Moving,
    Settled,
}

pub(crate) fn eased_animation<AnimatedValue>(
    resting_value: AnimatedValue,
    duration: Duration,
) -> Animation<AnimatedValue>
where
    AnimatedValue: Clone + Copy + PartialEq + Float,
{
    Animation::new(resting_value)
        .easing(Easing::EaseOutCubic)
        .duration(duration)
}

pub(crate) fn motion_of<'animation, AnimatedValue>(
    animations: impl IntoIterator<Item = &'animation Animation<AnimatedValue>>,
    now: Instant,
) -> MotionState
where
    AnimatedValue: Clone + Copy + PartialEq + Float + 'animation,
{
    if animations
        .into_iter()
        .any(|animation| animation.is_animating(now))
    {
        MotionState::Moving
    } else {
        MotionState::Settled
    }
}

#[cfg(test)]
#[expect(
    clippy::disallowed_methods,
    reason = "an animation test needs a real starting instant"
)]
mod tests {
    use iced::time::{Duration, Instant};

    use super::{MotionState, eased_animation, motion_of};

    #[test]
    fn an_eased_animation_moves_until_its_duration_passes() {
        let start = Instant::now();
        let duration = Duration::from_millis(100);
        let travel = eased_animation(0.0_f32, duration).go(10.0, start);
        assert_eq!(motion_of([&travel], start), MotionState::Moving);
        assert_eq!(
            motion_of([&travel], start + duration * 2),
            MotionState::Settled
        );
    }

    #[test]
    fn a_resting_animation_is_settled() {
        let resting = eased_animation(false, Duration::from_millis(100));
        assert_eq!(motion_of([&resting], Instant::now()), MotionState::Settled);
    }
}
