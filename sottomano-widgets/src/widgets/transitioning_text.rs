use std::{
    f32::consts::{FRAC_PI_2, TAU},
    hash::{DefaultHasher, Hash, Hasher},
    slice,
};

use iced::{
    Background, Border, Color, Element, Event, Font, Gradient, Length, Pixels, Radians, Rectangle,
    Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay,
        renderer::{self, Quad},
        text::{Alignment, Renderer as _, Shaping, Text, Wrapping},
        widget::{Operation, Tree, tree},
    },
    alignment::Vertical,
    gradient::Linear,
    mouse::Cursor,
    time::{Duration, Instant},
    window,
};
use sottomano_config::{
    theme::font::ResolvedFont,
    tooltips::{IntroEffect, PlaceholderEffect, ReplaceEffect, TooltipLoading},
};

use crate::widgets::text::{FontFaces, LINE_HEIGHT};

const NOISE_GLYPHS: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789#%&@$*+=<>/\\|";
const SKELETON_ALPHA: f32 = 0.18;
const PULSE_FLOOR_ALPHA: f32 = 0.06;
const PULSE_SWING_ALPHA: f32 = 0.26;
const SHIMMER_BASE_ALPHA: f32 = 0.1;
const SHIMMER_CREST_ALPHA: f32 = 0.36;
const SHIMMER_HALF_BAND: f32 = 0.18;
const NOISE_ALPHA: f32 = 0.5;
const NOISE_GLYPH_ADVANCE_PER_SIZE: f32 = 0.6;
const BAR_HEIGHT_FRACTION: f32 = 0.7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextPresence {
    Placeholder,
    Shown,
}

#[derive(Debug, Clone, Copy)]
pub struct TransitionStyle {
    pub loading: TooltipLoading,
    pub presence: TextPresence,
    pub highlight: Color,
}

pub struct TransitioningText<'a, Message> {
    settled: Element<'a, Message>,
    text: String,
    identity: u64,
    style: TransitionStyle,
    font: Font,
    size: f32,
    color: Color,
}

impl<'a, Message> TransitioningText<'a, Message> {
    pub fn new(
        settled: impl Into<Element<'a, Message>>,
        text: String,
        style: TransitionStyle,
        font: &ResolvedFont,
        color: Color,
    ) -> Self {
        Self {
            settled: settled.into(),
            text,
            identity: 0,
            style,
            font: FontFaces::from(font).regular,
            size: font.text_size_px,
            color,
        }
    }

    #[must_use]
    pub fn identified_by(self, identity: &impl Hash) -> Self {
        let mut hasher = DefaultHasher::new();
        identity.hash(&mut hasher);
        Self {
            identity: hasher.finish(),
            ..self
        }
    }
}

impl<'a, Message: 'a> From<TransitioningText<'a, Message>> for Element<'a, Message> {
    fn from(text: TransitioningText<'a, Message>) -> Self {
        Element::new(text)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum ShownText {
    Placeholder,
    Text { text: String, identity: u64 },
}

impl ShownText {
    fn transition_to(&self, next: &ShownText) -> Option<TransitionKind> {
        match self {
            ShownText::Placeholder => {
                matches!(next, ShownText::Text { .. }).then_some(TransitionKind::Intro)
            }
            ShownText::Text { text, identity } => match next {
                ShownText::Text {
                    identity: next_identity,
                    ..
                } => (next_identity == identity)
                    .then(|| TransitionKind::Replace { from: text.clone() }),
                ShownText::Placeholder => None,
            },
        }
    }
}

enum TransitionKind {
    Intro,
    Replace { from: String },
}

struct Transition {
    kind: TransitionKind,
    started: Instant,
}

struct Running<'s> {
    kind: &'s TransitionKind,
    elapsed: Duration,
    length: Duration,
}

impl Running<'_> {
    fn progress(&self) -> f32 {
        (self.elapsed.as_secs_f32() / self.length.as_secs_f32().max(f32::EPSILON)).clamp(0.0, 1.0)
    }

    fn eased(&self) -> f32 {
        let progress = self.progress();
        progress * progress * (3.0 - 2.0 * progress)
    }
}

struct State {
    shown: ShownText,
    pending: Option<ShownText>,
    transition: Option<Transition>,
    placeholder_since: Option<Instant>,
    now: Option<Instant>,
}

impl State {
    fn settled(shown: ShownText) -> Self {
        Self {
            shown,
            pending: None,
            transition: None,
            placeholder_since: None,
            now: None,
        }
    }

    fn expect(&mut self, next: ShownText) {
        if self.shown == next {
            self.pending = None;
        } else {
            self.pending = Some(next);
        }
    }

    fn frame(&mut self, now: Instant) {
        self.now = Some(now);
        let Some(next) = self.pending.take() else {
            return;
        };
        if next == ShownText::Placeholder {
            self.placeholder_since.get_or_insert(now);
        } else {
            self.placeholder_since = None;
        }
        self.transition = self
            .shown
            .transition_to(&next)
            .map(|kind| Transition { kind, started: now });
        self.shown = next;
    }

    fn placeholder_elapsed(&self) -> Duration {
        match (self.now, self.placeholder_since) {
            (Some(now), Some(since)) => now.saturating_duration_since(since),
            _ => Duration::ZERO,
        }
    }
}

impl<Message> TransitioningText<'_, Message> {
    fn shown_text(&self) -> ShownText {
        match self.style.presence {
            TextPresence::Placeholder => ShownText::Placeholder,
            TextPresence::Shown => ShownText::Text {
                text: self.text.clone(),
                identity: self.identity,
            },
        }
    }

    fn running<'s>(&self, state: &'s State) -> Option<Running<'s>> {
        let transition = state.transition.as_ref()?;
        let length = match &transition.kind {
            TransitionKind::Intro => self.style.loading.intro.length(&self.text),
            TransitionKind::Replace { .. } => self.style.loading.replace.length(),
        };
        let elapsed = state.now?.saturating_duration_since(transition.started);
        (elapsed < length).then_some(Running {
            kind: &transition.kind,
            elapsed,
            length,
        })
    }

    fn animating(&self, state: &State) -> bool {
        match state.shown {
            ShownText::Placeholder => true,
            ShownText::Text { .. } => self.running(state).is_some(),
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
        let bar_height = line_height * BAR_HEIGHT_FRACTION;
        let rows = ((area.height / line_height).round() as usize).max(1);
        (0..rows).for_each(|row| {
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle {
                        x: area.x,
                        y: area.y + row as f32 * line_height + (line_height - bar_height) / 2.0,
                        width: area.width,
                        height: bar_height,
                    },
                    border: Border::default().rounded(bar_height / 4.0),
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

    fn locks_by(&self, running: &Running<'_>, position: usize) -> bool {
        let length = self.text.chars().count().max(1);
        running.elapsed >= lock_time(running.length, self.seed(), position, length)
    }

    fn draw_placeholder(&self, renderer: &mut iced::Renderer, area: Rectangle, elapsed: Duration) {
        match self.style.loading.placeholder {
            PlaceholderEffect::Pulse { period } => {
                let breath = 0.5 - (cycle_fraction(elapsed, period) * TAU).cos() * 0.5;
                let alpha = PULSE_FLOOR_ALPHA + PULSE_SWING_ALPHA * breath;
                self.fill_bars(
                    renderer,
                    area,
                    Background::Color(self.color.scale_alpha(alpha)),
                );
            }
            PlaceholderEffect::Shimmer { sweep_period } => {
                let crest = cycle_fraction(elapsed, sweep_period) * (1.0 + 2.0 * SHIMMER_HALF_BAND)
                    - SHIMMER_HALF_BAND;
                self.fill_bars(renderer, area, shimmer_background(self.color, crest));
            }
            PlaceholderEffect::Noise { glyph_change } => {
                let content = self.noise_run(area, elapsed, glyph_change);
                self.fill_text(
                    renderer,
                    content,
                    area,
                    area,
                    self.color.scale_alpha(NOISE_ALPHA),
                );
            }
        }
    }

    fn draw_intro(&self, renderer: &mut iced::Renderer, area: Rectangle, running: &Running<'_>) {
        match self.style.loading.intro {
            IntroEffect::Fade { .. } => {
                let eased = running.eased();
                let skeleton = self.color.scale_alpha(SKELETON_ALPHA * (1.0 - eased));
                self.fill_bars(renderer, area, Background::Color(skeleton));
                let color = self.color.scale_alpha(eased);
                self.fill_text(renderer, self.text.clone(), area, area, color);
            }
            IntroEffect::Decode { glyph_change, .. } => {
                let content = self.scrambled(running.elapsed, glyph_change, |position, _| {
                    self.locks_by(running, position)
                });
                self.fill_text(renderer, content, area, area, self.color);
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
                let skeleton = self.color.scale_alpha(SKELETON_ALPHA);
                self.fill_bars(renderer, pending, Background::Color(skeleton));
                self.fill_text(renderer, self.text.clone(), area, revealed, self.color);
            }
            IntroEffect::Typewriter { per_char } => {
                let typed = (running.elapsed.as_nanos() / per_char.as_nanos().max(1)) as usize;
                let content = self.text.chars().take(typed + 1).collect();
                self.fill_text(renderer, content, area, area, self.color);
            }
            IntroEffect::Immediate => {
                self.fill_text(renderer, self.text.clone(), area, area, self.color);
            }
        }
    }

    fn draw_replace(
        &self,
        renderer: &mut iced::Renderer,
        area: Rectangle,
        from: &str,
        running: &Running<'_>,
    ) {
        match self.style.loading.replace {
            ReplaceEffect::Decode { glyph_change, .. } => {
                let previous: Vec<char> = from.chars().collect();
                let content = self.scrambled(running.elapsed, glyph_change, |position, target| {
                    previous.get(position) == Some(&target) || self.locks_by(running, position)
                });
                self.fill_text(renderer, content, area, area, self.color);
            }
            ReplaceEffect::Crossfade { .. } => {
                let eased = running.eased();
                let leaving = self.color.scale_alpha(1.0 - eased);
                self.fill_text(renderer, from.to_owned(), area, area, leaving);
                let arriving = self.color.scale_alpha(eased);
                self.fill_text(renderer, self.text.clone(), area, area, arriving);
            }
            ReplaceEffect::Roll { .. } => {
                let shift = area.height * running.eased();
                let leaving = area + Vector::new(0.0, -shift);
                self.fill_text(renderer, from.to_owned(), leaving, area, self.color);
                let arriving = area + Vector::new(0.0, area.height - shift);
                self.fill_text(renderer, self.text.clone(), arriving, area, self.color);
            }
            ReplaceEffect::Flash { .. } => {
                let color = blend(self.style.highlight, self.color, running.eased());
                self.fill_text(renderer, self.text.clone(), area, area, color);
            }
            ReplaceEffect::Immediate => {
                self.fill_text(renderer, self.text.clone(), area, area, self.color);
            }
        }
    }
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

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xCBF2_9CE4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01B3)
    })
}

fn mix(value: u64) -> u64 {
    let value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn unit(value: u64) -> f32 {
    (mix(value) >> 40) as f32 / (1u64 << 24) as f32
}

fn lock_time(settle: Duration, seed: u64, position: usize, length: usize) -> Duration {
    let order = position as f32 / length as f32;
    let chance = unit(seed.wrapping_add(position as u64));
    settle.mul_f32(0.15 + 0.35 * order + 0.5 * chance)
}

fn noise_glyph(seed: u64, position: usize) -> char {
    let pick = mix(seed ^ position as u64);
    char::from(NOISE_GLYPHS[(pick % NOISE_GLYPHS.len() as u64) as usize])
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for TransitioningText<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::settled(self.shown_text()))
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.settled)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.state.downcast_mut::<State>().expect(self.shown_text());
        tree.diff_children(slice::from_ref(&self.settled));
    }

    fn size(&self) -> Size<Length> {
        self.settled.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let child = self
            .settled
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        Node::with_children(child.size(), vec![child])
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let settled = layout.children().next().unwrap();
        let area = settled.bounds();
        match &state.shown {
            ShownText::Placeholder => {
                self.draw_placeholder(renderer, area, state.placeholder_elapsed());
            }
            ShownText::Text { .. } => match self.running(state) {
                None => self.settled.as_widget().draw(
                    &tree.children[0],
                    renderer,
                    theme,
                    style,
                    settled,
                    cursor,
                    viewport,
                ),
                Some(running) => match running.kind {
                    TransitionKind::Intro => self.draw_intro(renderer, area, &running),
                    TransitionKind::Replace { from } => {
                        self.draw_replace(renderer, area, from, &running);
                    }
                },
            },
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.settled.as_widget_mut().operate(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let state = tree.state.downcast_mut::<State>();
            state.frame(*now);
            if self.animating(state) {
                shell.request_redraw();
            }
        }
        self.settled.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.settled.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, iced::Theme, iced::Renderer>> {
        self.settled.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}
