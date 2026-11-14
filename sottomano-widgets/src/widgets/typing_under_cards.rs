use corredo::transitions::MotionState;
use iced::{
    Animation, Rectangle, Subscription,
    animation::Easing,
    event::listen_with,
    futures::{Stream, StreamExt, stream},
    mouse,
    time::{Duration, Instant},
};
use sottomano_desktop_sources::{
    compositor_window_ipc::compositor_window_state::CompositorWindowState,
    desktop_windows_apps::window_edge_placement::Placement,
    login_session_presence::wayland_idle_notify::{IdleEdge, input_idle_edges},
};
use sottomano_toml_config::overlay_surfaces::OverlaySurfacesConfig;
use tokio::time::sleep_until;

const POINTER_SETTLE_IDLE_DELAYS: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum CardBackgroundFade {
    #[default]
    Resting,
    Fading {
        focused_window: Rectangle,
        fade_level: f32,
    },
}

impl CardBackgroundFade {
    #[must_use]
    pub fn fade_level_over(self, card_bounds: Rectangle) -> f32 {
        match self {
            Self::Fading {
                focused_window,
                fade_level,
            } if card_bounds.intersects(&focused_window) => fade_level,
            Self::Fading { .. } | Self::Resting => 0.0,
        }
    }

    #[must_use]
    pub fn fade_level_anywhere(self) -> f32 {
        match self {
            Self::Fading { fade_level, .. } => fade_level,
            Self::Resting => 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TypingSignal {
    InputEdge(IdleEdge),
    ShellPointerMoved,
    RecheckDue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardVisibility {
    Shown,
    Hidden,
}

pub struct TypingUnderCards {
    input_is_active: bool,
    last_shell_pointer_motion: Instant,
    background_fading: Animation<bool>,
}

fn fading_animation(is_fading: bool, fade_duration: Duration) -> Animation<bool> {
    Animation::new(is_fading)
        .easing(Easing::EaseOutCubic)
        .duration(fade_duration)
}

fn input_edge_signals(idle_delay: &Duration) -> impl Stream<Item = TypingSignal> + Send + use<> {
    input_idle_edges(*idle_delay).map(|edge| TypingSignal::InputEdge(edge.edge()))
}

#[must_use]
pub fn viewed_window_bounds(placement: &Placement, window_id: u64) -> Option<Rectangle> {
    let layout = placement.viewed_window_layout(window_id)?;
    let (tile_x, tile_y) = layout.tile_position_in_view?;
    let (offset_x, offset_y) = layout.window_offset_in_tile;
    let (width, height) = layout.window_size;
    Some(Rectangle {
        x: (tile_x + offset_x) as f32,
        y: (tile_y + offset_y) as f32,
        width: width as f32,
        height: height as f32,
    })
}

fn focused_window_bounds_on(output: &str, placement: &Placement) -> Option<Rectangle> {
    let window_id = placement.focused_window()?;
    let workspace_id = placement.workspace_of_window(window_id)?;
    (placement.output_of_workspace(workspace_id)? == output)
        .then(|| viewed_window_bounds(placement, window_id))
        .flatten()
}

impl TypingUnderCards {
    #[must_use]
    pub fn new(now: Instant, overlay_surfaces: &OverlaySurfacesConfig) -> Self {
        Self {
            input_is_active: false,
            last_shell_pointer_motion: now,
            background_fading: fading_animation(false, overlay_surfaces.surface_morph_duration),
        }
    }

    pub fn adopt_config(&mut self, overlay_surfaces: &OverlaySurfacesConfig) {
        self.background_fading = fading_animation(
            self.background_fading.value(),
            overlay_surfaces.surface_morph_duration,
        );
    }

    pub fn update(
        &mut self,
        signal: TypingSignal,
        now: Instant,
        overlay_surfaces: &OverlaySurfacesConfig,
    ) {
        match signal {
            TypingSignal::InputEdge(edge) => self.input_is_active = edge == IdleEdge::Resumed,
            TypingSignal::ShellPointerMoved => self.last_shell_pointer_motion = now,
            TypingSignal::RecheckDue => {}
        }
        let typing_suspected = self.input_is_active
            && self.pointer_settled_at(overlay_surfaces.typing_idle_delay) <= now;
        self.background_fading.go_mut(typing_suspected, now);
    }

    pub fn subscription(
        &self,
        card_visibility: CardVisibility,
        overlay_surfaces: &OverlaySurfacesConfig,
    ) -> Subscription<TypingSignal> {
        let input_edges =
            Subscription::run_with(overlay_surfaces.typing_idle_delay, input_edge_signals);
        if card_visibility == CardVisibility::Hidden {
            return input_edges;
        }
        let pointer_motion = listen_with(|event, _status, _window| match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                Some(TypingSignal::ShellPointerMoved)
            }
            _ => None,
        });
        let recheck = self
            .next_recheck(overlay_surfaces.typing_idle_delay)
            .map_or_else(Subscription::none, |deadline| {
                Subscription::run_with(deadline, |&deadline| {
                    stream::once(sleep_until(deadline.into())).map(|()| TypingSignal::RecheckDue)
                })
            });
        Subscription::batch([input_edges, pointer_motion, recheck])
    }

    fn pointer_settled_at(&self, idle_delay: Duration) -> Instant {
        self.last_shell_pointer_motion + idle_delay * POINTER_SETTLE_IDLE_DELAYS
    }

    fn next_recheck(&self, idle_delay: Duration) -> Option<Instant> {
        (self.input_is_active && !self.background_fading.value())
            .then(|| self.pointer_settled_at(idle_delay))
    }

    #[must_use]
    pub fn background_motion(&self, now: Instant) -> MotionState {
        if self.background_fading.is_animating(now) {
            MotionState::Moving
        } else {
            MotionState::Settled
        }
    }

    fn fade_level(&self, now: Instant, overlay_surfaces: &OverlaySurfacesConfig) -> f32 {
        let kept_opacity = overlay_surfaces.typing_background_opacity.clamp(0.0, 1.0);
        self.background_fading
            .interpolate(0.0, 1.0 - kept_opacity, now)
    }

    #[must_use]
    pub fn background_fade_on(
        &self,
        output: &str,
        compositor: &CompositorWindowState,
        now: Instant,
        overlay_surfaces: &OverlaySurfacesConfig,
    ) -> CardBackgroundFade {
        let fade_level = self.fade_level(now, overlay_surfaces);
        if fade_level <= 0.0 {
            return CardBackgroundFade::Resting;
        }
        focused_window_bounds_on(output, &Placement::derive(compositor)).map_or(
            CardBackgroundFade::Resting,
            |focused_window| CardBackgroundFade::Fading {
                focused_window,
                fade_level,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use iced::{
        Point, Rectangle, Size,
        time::{Duration, Instant},
    };
    use sottomano_desktop_sources::login_session_presence::wayland_idle_notify::IdleEdge;
    use sottomano_toml_config::overlay_surfaces::OverlaySurfacesConfig;

    use super::{CardBackgroundFade, TypingSignal, TypingUnderCards};
    use crate::clock::Clock;

    const IDLE_DELAY: Duration = Duration::from_millis(500);
    const LONG_AFTER_FADE: Duration = Duration::from_secs(1);
    const SETTLED_AFTER: Duration = Duration::from_secs(1);
    const KEPT_OPACITY: f32 = 0.25;
    const FULL_FADE_LEVEL: f32 = 1.0 - KEPT_OPACITY;
    const LEVEL_TOLERANCE: f32 = 0.001;

    fn overlay_config() -> OverlaySurfacesConfig {
        OverlaySurfacesConfig {
            surface_morph_duration: Duration::from_millis(100),
            typing_idle_delay: IDLE_DELAY,
            typing_background_opacity: KEPT_OPACITY,
        }
    }

    fn tracker_with_input_active(start: Instant) -> TypingUnderCards {
        let mut tracker = TypingUnderCards::new(start, &overlay_config());
        tracker.update(
            TypingSignal::InputEdge(IdleEdge::Resumed),
            start,
            &overlay_config(),
        );
        tracker
    }

    fn settled_fade_level(tracker: &TypingUnderCards, now: Instant) -> f32 {
        tracker.fade_level(now + LONG_AFTER_FADE, &overlay_config())
    }

    #[test]
    fn input_with_a_resting_pointer_fades_down_to_the_kept_opacity() {
        let start = Clock::real().instant;
        let mut tracker = tracker_with_input_active(start);
        let settled = start + SETTLED_AFTER;
        tracker.update(TypingSignal::RecheckDue, settled, &overlay_config());
        let reached = settled_fade_level(&tracker, settled);
        assert!((reached - FULL_FADE_LEVEL).abs() <= LEVEL_TOLERANCE);
    }

    #[test]
    fn pointer_motion_over_the_shell_holds_the_background() {
        let start = Clock::real().instant;
        let mut tracker = tracker_with_input_active(start);
        let moved = start + SETTLED_AFTER;
        tracker.update(TypingSignal::ShellPointerMoved, moved, &overlay_config());
        let recheck = moved + IDLE_DELAY;
        tracker.update(TypingSignal::RecheckDue, recheck, &overlay_config());
        assert!(settled_fade_level(&tracker, recheck) <= LEVEL_TOLERANCE);
    }

    #[test]
    fn pointer_only_input_goes_idle_before_the_pointer_settles() {
        let start = Clock::real().instant;
        let mut tracker = tracker_with_input_active(start);
        let idled = start + IDLE_DELAY;
        tracker.update(
            TypingSignal::InputEdge(IdleEdge::Idled),
            idled,
            &overlay_config(),
        );
        let settled = start + SETTLED_AFTER;
        tracker.update(TypingSignal::RecheckDue, settled, &overlay_config());
        assert!(settled_fade_level(&tracker, settled) <= LEVEL_TOLERANCE);
    }

    #[test]
    fn going_idle_restores_a_faded_background() {
        let start = Clock::real().instant;
        let mut tracker = tracker_with_input_active(start);
        let settled = start + SETTLED_AFTER;
        tracker.update(TypingSignal::RecheckDue, settled, &overlay_config());
        tracker.update(
            TypingSignal::InputEdge(IdleEdge::Idled),
            settled,
            &overlay_config(),
        );
        assert!(settled_fade_level(&tracker, settled) <= LEVEL_TOLERANCE);
    }

    #[test]
    fn a_recheck_is_due_only_while_active_input_has_not_faded_yet() {
        let start = Clock::real().instant;
        let idle_tracker = TypingUnderCards::new(start, &overlay_config());
        assert_eq!(idle_tracker.next_recheck(IDLE_DELAY), None);
        let mut tracker = tracker_with_input_active(start);
        let settled = start + SETTLED_AFTER;
        assert_eq!(tracker.next_recheck(IDLE_DELAY), Some(settled));
        tracker.update(TypingSignal::RecheckDue, settled, &overlay_config());
        assert_eq!(tracker.next_recheck(IDLE_DELAY), None);
    }

    #[test]
    fn only_cards_over_the_focused_window_fade() {
        let background_fade = CardBackgroundFade::Fading {
            focused_window: Rectangle::new(Point::ORIGIN, Size::new(100.0, 100.0)),
            fade_level: FULL_FADE_LEVEL,
        };
        let card_size = Size::new(10.0, 10.0);
        let overlapping = Rectangle::new(Point::new(95.0, 0.0), card_size);
        let elsewhere = Rectangle::new(Point::new(200.0, 0.0), card_size);
        assert!(background_fade.fade_level_over(overlapping) >= FULL_FADE_LEVEL);
        assert!(background_fade.fade_level_over(elsewhere) <= 0.0);
    }
}
