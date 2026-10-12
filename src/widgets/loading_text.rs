use std::{f32::consts::TAU, slice};

use iced::{
    Background, Border, Color, Element, Event, Font, Length, Pixels, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay,
        renderer::{self, Quad},
        text::{Alignment, Renderer as _, Shaping, Text, Wrapping},
        widget::{Operation, Tree, tree},
    },
    alignment::Vertical,
    mouse::Cursor,
    time::{Duration, Instant},
    window,
};

use crate::{
    config::{
        theme::font::ResolvedFont,
        tooltips::{TooltipLoading, TooltipLoadingEffect},
    },
    widgets::text::LINE_HEIGHT,
};

const NOISE_GLYPHS: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789#%&@$*+=<>/\\|";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadingPhase {
    Estimated(TooltipLoading),
    Arriving(TooltipLoading),
}

pub fn loading_text<'a, Message: 'a>(
    revealed: impl Into<Element<'a, Message>>,
    text: String,
    phase: Option<LoadingPhase>,
    font: &ResolvedFont,
    color: Color,
) -> Element<'a, Message> {
    let (loading, settles) = match phase {
        None => return revealed.into(),
        Some(LoadingPhase::Estimated(loading)) => (loading, false),
        Some(LoadingPhase::Arriving(loading)) => (loading, true),
    };
    Element::new(LoadingText {
        revealed: revealed.into(),
        text,
        settles,
        loading,
        font: font.iced_font,
        size: font.text_size_px,
        color,
    })
}

struct State {
    born: Instant,
    now: Instant,
    settling: bool,
}

impl State {
    fn elapsed(&self) -> Duration {
        self.now.saturating_duration_since(self.born)
    }
}

struct LoadingText<'a, Message> {
    revealed: Element<'a, Message>,
    text: String,
    settles: bool,
    loading: TooltipLoading,
    font: Font,
    size: f32,
    color: Color,
}

impl<Message> LoadingText<'_, Message> {
    fn settled(&self, state: &State) -> bool {
        self.settles && state.elapsed() >= self.loading.reveal_duration
    }

    fn revealed(&self, state: &State) -> f32 {
        if !self.settles {
            return 0.0;
        }
        let reveal = self.loading.reveal_duration.as_secs_f32().max(f32::EPSILON);
        (state.elapsed().as_secs_f32() / reveal).clamp(0.0, 1.0)
    }

    fn line_height(&self) -> f32 {
        LINE_HEIGHT.to_absolute(Pixels(self.size)).0
    }

    fn fill_text(
        &self,
        renderer: &mut iced::Renderer,
        content: String,
        area: Rectangle,
        color: Color,
    ) {
        renderer.fill_text(
            Text {
                content,
                bounds: Size::new(area.width, area.height),
                size: Pixels(self.size),
                line_height: LINE_HEIGHT,
                font: self.font,
                align_x: Alignment::Left,
                align_y: Vertical::Top,
                shaping: Shaping::Advanced,
                wrapping: Wrapping::Word,
            },
            area.position(),
            color,
            area,
        );
    }

    fn draw_decode(&self, renderer: &mut iced::Renderer, state: &State, area: Rectangle) {
        let elapsed = state.elapsed();
        let glyph_change = self.loading.decode.glyph_change.as_millis().max(1);
        let frame = (elapsed.as_millis() / glyph_change) as u64;
        let settle = self.loading.reveal_duration;
        let seed = fnv(self.text.as_bytes());
        let length = self.text.chars().count().max(1);
        let content = self
            .text
            .chars()
            .enumerate()
            .map(|(position, target)| {
                let locked = self.settles && elapsed >= lock_time(settle, seed, position, length);
                if locked || target.is_whitespace() {
                    target
                } else {
                    noise_glyph(seed ^ frame.wrapping_mul(0x9E37_79B9), position)
                }
            })
            .collect();
        self.fill_text(renderer, content, area, self.color);
    }

    fn draw_skeleton(&self, renderer: &mut iced::Renderer, state: &State, area: Rectangle) {
        let elapsed = state.elapsed().as_secs_f32();
        let period = self
            .loading
            .skeleton
            .pulse_period
            .as_secs_f32()
            .max(f32::EPSILON);
        let phase = 0.5 - (elapsed / period * TAU).cos() * 0.5;
        let revealed = self.revealed(state);
        let bar_color = Color {
            a: self.color.a * (0.06 + 0.26 * phase) * (1.0 - revealed),
            ..self.color
        };
        let line_height = self.line_height();
        let bar_height = line_height * 0.7;
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
                Background::Color(bar_color),
            );
        });
        if self.settles {
            let text_color = Color {
                a: self.color.a * revealed,
                ..self.color
            };
            self.fill_text(renderer, self.text.clone(), area, text_color);
        }
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

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for LoadingText<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        let now = Instant::now();
        tree::State::new(State {
            born: now,
            now,
            settling: self.settles,
        })
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.revealed)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(slice::from_ref(&self.revealed));
    }

    fn size(&self) -> Size<Length> {
        self.revealed.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let child = self
            .revealed
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
        let revealed = layout.children().next().unwrap();
        if self.settled(state) {
            self.revealed.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                revealed,
                cursor,
                viewport,
            );
            return;
        }
        let area = revealed.bounds();
        match self.loading.effect {
            TooltipLoadingEffect::Decode => self.draw_decode(renderer, state, area),
            TooltipLoadingEffect::Skeleton => self.draw_skeleton(renderer, state, area),
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.revealed.as_widget_mut().operate(
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
            state.now = *now;
            if self.settles && !state.settling {
                state.born = *now;
            }
            state.settling = self.settles;
            if !self.settled(state) {
                shell.request_redraw();
            }
        }
        self.revealed.as_widget_mut().update(
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
        self.revealed.as_widget().mouse_interaction(
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
        self.revealed.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}
