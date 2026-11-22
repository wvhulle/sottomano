use iced::time::{Duration, Instant};

use crate::transitions::RedrawClock;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum ShownText {
    Placeholder,
    Text { text: String, identity: u64 },
}

impl ShownText {
    fn transition_to(&self, next: &ShownText, started: Instant) -> TextTransition {
        let kind = match (self, next) {
            (ShownText::Placeholder, ShownText::Text { .. }) => TransitionKind::Intro,
            (
                ShownText::Text { text, identity },
                ShownText::Text {
                    identity: next_identity,
                    ..
                },
            ) if next_identity == identity => TransitionKind::Replace { from: text.clone() },
            (ShownText::Placeholder | ShownText::Text { .. }, _) => return TextTransition::Still,
        };
        TextTransition::Running { kind, started }
    }
}

pub(super) enum TextTransition {
    Still,
    Running {
        kind: TransitionKind,
        started: Instant,
    },
}

enum PlaceholderTimer {
    Stopped,
    RunningSince(Instant),
}

pub(super) enum TransitionKind {
    Intro,
    Replace { from: String },
}

pub(super) struct Running<'state> {
    pub(super) kind: &'state TransitionKind,
    pub(super) elapsed: Duration,
    pub(super) length: Duration,
}

impl Running<'_> {
    pub(super) fn progress(&self) -> f32 {
        (self.elapsed.as_secs_f32() / self.length.as_secs_f32().max(f32::EPSILON)).clamp(0.0, 1.0)
    }

    pub(super) fn eased(&self) -> f32 {
        let progress = self.progress();
        progress * progress * (3.0 - 2.0 * progress)
    }
}

pub(super) struct State {
    pub(super) shown: ShownText,
    expected: ShownText,
    pub(super) transition: TextTransition,
    placeholder_timer: PlaceholderTimer,
    pub(super) redraw_clock: RedrawClock,
}

impl State {
    pub(super) fn settled(shown: ShownText) -> Self {
        Self {
            expected: shown.clone(),
            shown,
            transition: TextTransition::Still,
            placeholder_timer: PlaceholderTimer::Stopped,
            redraw_clock: RedrawClock::AwaitingFirstFrame,
        }
    }

    pub(super) fn expect(&mut self, next: ShownText) {
        self.expected = next;
    }

    pub(super) fn frame(&mut self, now: Instant) {
        self.redraw_clock = RedrawClock::FrameAt(now);
        if self.expected != self.shown {
            self.transition = self.shown.transition_to(&self.expected, now);
            self.shown = self.expected.clone();
        }
        self.placeholder_timer = match (&self.shown, &self.placeholder_timer) {
            (ShownText::Placeholder, PlaceholderTimer::RunningSince(since)) => {
                PlaceholderTimer::RunningSince(*since)
            }
            (ShownText::Placeholder, PlaceholderTimer::Stopped) => {
                PlaceholderTimer::RunningSince(now)
            }
            (ShownText::Text { .. }, _) => PlaceholderTimer::Stopped,
        };
    }

    pub(super) fn placeholder_elapsed(&self) -> Duration {
        let RedrawClock::FrameAt(now) = self.redraw_clock else {
            return Duration::ZERO;
        };
        let PlaceholderTimer::RunningSince(since) = self.placeholder_timer else {
            return Duration::ZERO;
        };
        now.checked_duration_since(since)
            .expect("the placeholder appeared at or before the latest redraw")
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    #[test]
    fn a_placeholder_shown_from_the_start_keeps_pulsing() {
        let first_frame = Instant::now();
        let mut state = State::settled(ShownText::Placeholder);
        state.frame(first_frame);
        state.frame(first_frame + Duration::from_millis(700));
        assert_eq!(state.placeholder_elapsed(), Duration::from_millis(700));
    }
}
