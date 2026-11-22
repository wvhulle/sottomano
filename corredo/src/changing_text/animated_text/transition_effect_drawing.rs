use std::f32::consts::{FRAC_PI_2, TAU};

use iced::{
    Background, Border, Color, Gradient, Pixels, Radians, Rectangle, Size, Vector,
    advanced::{
        Renderer as _,
        renderer::Quad,
        text::{Alignment, Renderer as _, Shaping, Text, Wrapping},
    },
    alignment::Vertical,
    gradient::Linear,
    time::Duration,
};

use super::{
    TextPresence, TransitioningText,
    glyph_scramble_hashing::{fnv, lock_time, noise_glyph},
    text_transition_state::{Running, ShownText, State, TextTransition, TransitionKind},
};
use crate::{
    changing_text::{IntroEffect, PlaceholderEffect, ReplaceEffect},
    theming::LINE_HEIGHT,
    transitions::{MotionState, RedrawClock},
};

const SKELETON_ALPHA: f32 = 0.18;
const PULSE_FLOOR_ALPHA: f32 = 0.06;
const PULSE_SWING_ALPHA: f32 = 0.26;
const SHIMMER_BASE_ALPHA: f32 = 0.1;
const SHIMMER_CREST_ALPHA: f32 = 0.36;
const SHIMMER_HALF_BAND: f32 = 0.18;
const NOISE_ALPHA: f32 = 0.5;
const NOISE_GLYPH_ADVANCE_PER_SIZE: f32 = 0.6;
const BAR_HEIGHT_FRACTION: f32 = 0.7;

impl<Message> TransitioningText<'_, Message> {
    pub(super) fn shown_text(&self) -> ShownText {
        match self.style.presence {
            TextPresence::Placeholder => ShownText::Placeholder,
            TextPresence::Shown => ShownText::Text {
                text: self.text.clone(),
                identity: self.identity,
            },
        }
    }

    pub(super) fn running<'state>(&self, state: &'state State) -> Option<Running<'state>> {
        let TextTransition::Running { kind, started } = &state.transition else {
            return None;
        };
        let RedrawClock::FrameAt(now) = state.redraw_clock else {
            return None;
        };
        let length = match kind {
            TransitionKind::Intro => self.style.effects.intro.length(&self.text),
            TransitionKind::Replace { .. } => self.style.effects.replace.length(),
        };
        let elapsed = now
            .checked_duration_since(*started)
            .expect("a transition starts at or before the latest redraw");
        (elapsed < length).then_some(Running {
            kind,
            elapsed,
            length,
        })
    }

    pub(super) fn text_motion(&self, state: &State) -> MotionState {
        match &state.shown {
            ShownText::Placeholder => MotionState::Moving,
            ShownText::Text { .. } => match self.running(state) {
                Some(_) => MotionState::Moving,
                None => MotionState::Settled,
            },
        }
    }

    fn line_height(&self) -> f32 {
        LINE_HEIGHT.to_absolute(Pixels(self.size)).0
    }

    fn seed(&self) -> u64 {
        fnv(self.text.as_bytes())
    }

    fn fill_text(
        &self,
        renderer: &mut iced::Renderer,
        content: String,
        bounds: Rectangle,
        clip: Rectangle,
        color: Color,
    ) {
        renderer.fill_text(
            Text {
                content,
                bounds: Size::new(bounds.width, bounds.height),
                size: Pixels(self.size),
                line_height: LINE_HEIGHT,
                font: self.font,
                align_x: Alignment::Left,
                align_y: Vertical::Top,
                shaping: Shaping::Advanced,
                wrapping: Wrapping::Word,
            },
            bounds.position(),
            color,
            clip,
        );
    }

    fn fill_bars(&self, renderer: &mut iced::Renderer, area: Rectangle, background: Background) {
        let line_height = self.line_height();
        let panel_height = line_height * BAR_HEIGHT_FRACTION;
        let rows = ((area.height / line_height).round() as usize).max(1);
        (0..rows).for_each(|row| {
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle {
                        x: area.x,
                        y: area.y + row as f32 * line_height + (line_height - panel_height) / 2.0,
                        width: area.width,
                        height: panel_height,
                    },
                    border: Border::default().rounded(panel_height / 4.0),
                    ..Quad::default()
                },
                background,
            );
        });
    }

    fn scrambled(
        &self,
        elapsed: Duration,
        glyph_change: Duration,
        locked: impl Fn(usize, char) -> bool,
    ) -> String {
        let frame_seed = self.frame_seed(elapsed, glyph_change);
        self.text
            .chars()
            .enumerate()
            .map(|(position, target)| {
                if target.is_whitespace() || locked(position, target) {
                    target
                } else {
                    noise_glyph(frame_seed, position)
                }
            })
            .collect()
    }

    #[expect(
        clippy::disallowed_methods,
        reason = "hash mixing relies on wrapping arithmetic"
    )]
    fn frame_seed(&self, elapsed: Duration, glyph_change: Duration) -> u64 {
        let frame = (elapsed.as_millis() / glyph_change.as_millis().max(1)) as u64;
        self.seed() ^ frame.wrapping_mul(0x9E37_79B9)
    }

    fn noise_run(&self, area: Rectangle, elapsed: Duration, glyph_change: Duration) -> String {
        if !self.text.is_empty() {
            return self.scrambled(elapsed, glyph_change, |_, _| false);
        }
        let glyph_count =
            (area.width / (self.size * NOISE_GLYPH_ADVANCE_PER_SIZE)).floor() as usize;
        let frame_seed = self.frame_seed(elapsed, glyph_change);
        (0..glyph_count)
            .map(|position| noise_glyph(frame_seed, position))
            .collect()
    }

    fn letter_lock(&self, running: &Running<'_>, position: usize) -> LetterLock {
        let length = self.text.chars().count().max(1);
        if running.elapsed >= lock_time(running.length, self.seed(), position, length) {
            LetterLock::Locked
        } else {
            LetterLock::Scrambling
        }
    }

    pub(super) fn draw_placeholder(
        &self,
        renderer: &mut iced::Renderer,
        area: Rectangle,
        elapsed: Duration,
        color: Color,
    ) {
        match self.style.effects.placeholder {
            PlaceholderEffect::Pulse { period } => {
                let breath = 0.5 - (cycle_fraction(elapsed, period) * TAU).cos() * 0.5;
                let alpha = PULSE_FLOOR_ALPHA + PULSE_SWING_ALPHA * breath;
                self.fill_bars(renderer, area, Background::Color(color.scale_alpha(alpha)));
            }
            PlaceholderEffect::Shimmer { sweep_period } => {
                let crest = cycle_fraction(elapsed, sweep_period) * (1.0 + 2.0 * SHIMMER_HALF_BAND)
                    - SHIMMER_HALF_BAND;
                self.fill_bars(renderer, area, shimmer_background(color, crest));
            }
            PlaceholderEffect::Noise { glyph_change } => {
                let content = self.noise_run(area, elapsed, glyph_change);
                self.fill_text(
                    renderer,
                    content,
                    area,
                    area,
                    color.scale_alpha(NOISE_ALPHA),
                );
            }
        }
    }

    pub(super) fn draw_intro(
        &self,
        renderer: &mut iced::Renderer,
        area: Rectangle,
        running: &Running<'_>,
        color: Color,
    ) {
        match self.style.effects.intro {
            IntroEffect::Fade { .. } => {
                let eased = running.eased();
                let skeleton = color.scale_alpha(SKELETON_ALPHA * (1.0 - eased));
                self.fill_bars(renderer, area, Background::Color(skeleton));
                let faded_in = color.scale_alpha(eased);
                self.fill_text(renderer, self.text.clone(), area, area, faded_in);
            }
            IntroEffect::Decode { glyph_change, .. } => {
                let content = self.scrambled(running.elapsed, glyph_change, |position, _| {
                    self.letter_lock(running, position) == LetterLock::Locked
                });
                self.fill_text(renderer, content, area, area, color);
            }
            IntroEffect::Wipe { .. } => {
                let edge = area.width * running.eased();
                let revealed = Rectangle {
                    width: edge,
                    ..area
                };
                let pending = Rectangle {
                    x: area.x + edge,
                    width: area.width - edge,
                    ..area
                };
                let skeleton = color.scale_alpha(SKELETON_ALPHA);
                self.fill_bars(renderer, pending, Background::Color(skeleton));
                self.fill_text(renderer, self.text.clone(), area, revealed, color);
            }
            IntroEffect::Typewriter { per_char } => {
                let typed = (running.elapsed.as_nanos() / per_char.as_nanos().max(1)) as usize;
                let content = self.text.chars().take(typed + 1).collect();
                self.fill_text(renderer, content, area, area, color);
            }
            IntroEffect::Immediate => {
                self.fill_text(renderer, self.text.clone(), area, area, color);
            }
        }
    }

    pub(super) fn draw_replace(
        &self,
        renderer: &mut iced::Renderer,
        area: Rectangle,
        from: &str,
        running: &Running<'_>,
        text_colors: TextColors,
    ) {
        match self.style.effects.replace {
            ReplaceEffect::Decode { glyph_change, .. } => {
                let previous: Vec<char> = from.chars().collect();
                let content = self.scrambled(running.elapsed, glyph_change, |position, target| {
                    previous.get(position) == Some(&target)
                        || self.letter_lock(running, position) == LetterLock::Locked
                });
                self.fill_text(renderer, content, area, area, text_colors.text);
            }
            ReplaceEffect::Crossfade { .. } => {
                let eased = running.eased();
                let leaving = text_colors.text.scale_alpha(1.0 - eased);
                self.fill_text(renderer, from.to_owned(), area, area, leaving);
                let arriving = text_colors.text.scale_alpha(eased);
                self.fill_text(renderer, self.text.clone(), area, area, arriving);
            }
            ReplaceEffect::Roll { .. } => {
                let shift = area.height * running.eased();
                let leaving = area + Vector::new(0.0, -shift);
                self.fill_text(renderer, from.to_owned(), leaving, area, text_colors.text);
                let arriving = area + Vector::new(0.0, area.height - shift);
                self.fill_text(
                    renderer,
                    self.text.clone(),
                    arriving,
                    area,
                    text_colors.text,
                );
            }
            ReplaceEffect::Flash { .. } => {
                let flashed = blend(text_colors.highlight, text_colors.text, running.eased());
                self.fill_text(renderer, self.text.clone(), area, area, flashed);
            }
            ReplaceEffect::Immediate => {
                self.fill_text(renderer, self.text.clone(), area, area, text_colors.text);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TextColors {
    pub(super) text: Color,
    pub(super) highlight: Color,
}

fn cycle_fraction(elapsed: Duration, period: Duration) -> f32 {
    (elapsed.as_secs_f32() / period.as_secs_f32().max(f32::EPSILON)).fract()
}

fn shimmer_background(color: Color, crest: f32) -> Background {
    let base = color.scale_alpha(SHIMMER_BASE_ALPHA);
    let gradient = Linear::new(Radians(FRAC_PI_2))
        .add_stop(0.0, base)
        .add_stop((crest - SHIMMER_HALF_BAND).clamp(0.0, 1.0), base)
        .add_stop(
            crest.clamp(0.0, 1.0),
            color.scale_alpha(SHIMMER_CREST_ALPHA),
        )
        .add_stop((crest + SHIMMER_HALF_BAND).clamp(0.0, 1.0), base)
        .add_stop(1.0, base);
    Background::Gradient(Gradient::Linear(gradient))
}

fn blend(from: Color, to: Color, fraction: f32) -> Color {
    let towards = |start: f32, end: f32| start + (end - start) * fraction;
    Color {
        r: towards(from.r, to.r),
        g: towards(from.g, to.g),
        b: towards(from.b, to.b),
        a: towards(from.a, to.a),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LetterLock {
    Locked,
    Scrambling,
}
