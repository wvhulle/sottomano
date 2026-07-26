//! odin's `InlineSlider`: a `focus`-accented horizontal slider, shared by the
//! write-capable groups (audio, idle, panel) that unfold a slider on click. The
//! caller supplies the range, value, and message mapping; committing the write
//! is the caller's concern.
//!
//! The whole face — rail, handle, and the hollow ghost — is drawn on a canvas;
//! the built-in `slider` underneath is made fully transparent and kept only for
//! pointer input. Drawing it ourselves lets the rail be *omitted* across the
//! ghost knob (rather than showing through a transparent ring) and lets the
//! handle grow while dragging. `ghost` marks the value last *confirmed* on disk
//! / in the OS, so the user can watch the write land while the handle leads.

use std::ops::RangeInclusive;

use iced::{
    Color, Element, Point, Rectangle, Renderer, Size, Theme,
    mouse::Cursor,
    widget::{Slider, canvas, slider, stack},
};

use crate::config::resolved::ResolvedConfig;

/// Handle/ghost radius at rest, and the larger handle radius while dragging.
const KNOB_RADIUS: f32 = 5.0;
const KNOB_RADIUS_DRAG: f32 = 7.0;
/// Horizontal inset of a knob centre from each end, sized to the largest knob so
/// nothing is clipped at the extremes. Shared by handle and ghost so they track
/// the same geometry.
const EDGE_INSET: f32 = KNOB_RADIUS_DRAG;
/// Rail thickness.
const RAIL_WIDTH: f32 = 4.0;

/// A `focus`-accented slider over `range`, stepping by `step`, emitting
/// `on_change` while dragging and `on_release` when let go. When `ghost` is
/// `Some`, a hollow marker trails at the last-confirmed value and the handle
/// grows to signal the active drag.
pub fn inline_slider<'a, Message: Clone + 'a>(
    config: &ResolvedConfig,
    range: RangeInclusive<f32>,
    value: f32,
    ghost: Option<f32>,
    step: f32,
    on_change: impl Fn(f32) -> Message + 'a,
    on_release: Message,
) -> Element<'a, Message> {
    let face = SliderFace {
        range: range.clone(),
        value,
        ghost,
        accent: config.palette.focus,
        track: config.palette.bar_foreground.scale_alpha(0.25),
    };
    // The transparent slider is *always* stacked under the same face so the
    // widget tree never changes shape mid-drag (which would reset its drag state).
    stack![
        interaction_slider(config, range, value, step, on_change, on_release),
        canvas(face)
            .width(config.layout.slider_width_px)
            .height(Slider::<f32, Message>::DEFAULT_HEIGHT)
    ]
    .into()
}

/// The built-in slider stripped to nothing visible: it exists only to own the
/// press/drag/release interaction; all pixels come from [`SliderFace`].
fn interaction_slider<'a, Message: Clone + 'a>(
    config: &ResolvedConfig,
    range: RangeInclusive<f32>,
    value: f32,
    step: f32,
    on_change: impl Fn(f32) -> Message + 'a,
    on_release: Message,
) -> Slider<'a, f32, Message> {
    slider(range, value, on_change)
        .step(step)
        .on_release(on_release)
        .width(config.layout.slider_width_px)
        .style(|_theme, _status| slider::Style {
            rail: slider::Rail {
                backgrounds: (Color::TRANSPARENT.into(), Color::TRANSPARENT.into()),
                width: RAIL_WIDTH,
                border: iced::Border::default(),
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle { radius: KNOB_RADIUS },
                background: Color::TRANSPARENT.into(),
                border_color: Color::TRANSPARENT,
                border_width: 0.0,
            },
        })
}

/// The full slider face: the rail (accent up to the handle, dim track beyond),
/// the solid handle at `value`, and a hollow ghost ring at the confirmed value.
/// The rail is omitted across the ghost so no line shows through its centre.
struct SliderFace {
    range: RangeInclusive<f32>,
    value: f32,
    ghost: Option<f32>,
    accent: Color,
    track: Color,
}

impl<Message> canvas::Program<Message> for SliderFace {
    type State = ();

    fn draw(
        &self,
        (): &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<canvas::Geometry> {
        let (start, end) = (*self.range.start(), *self.range.end());
        let span = end - start;
        let centre = |v: f32| {
            let fraction = if span > 0.0 {
                ((v - start) / span).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (bounds.width - 2.0 * EDGE_INSET) * fraction + EDGE_INSET
        };

        let rail_y = bounds.height / 2.0;
        let handle_x = centre(self.value);
        // The filled rail ends at the confirmed value (the lagging ghost); the
        // stretch from there to the handle is pending, so it reads as empty.
        let fill_x = centre(self.ghost.unwrap_or(self.value));
        // The rail is cut where the hollow ghost sits, so nothing shows through it.
        let gap = self
            .ghost
            .map(|g| (centre(g) - KNOB_RADIUS, centre(g) + KNOB_RADIUS));

        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // Rail: accent up to the confirmed value, dim track beyond — each span cut
        // where the hollow ghost sits.
        [(0.0, fill_x, self.accent), (fill_x, bounds.width, self.track)]
            .into_iter()
            .flat_map(|(from, to, color)| {
                without_gap(from, to, gap).map(move |(a, b)| (a, b, color))
            })
            .for_each(|(a, b, color)| {
                frame.fill_rectangle(
                    Point::new(a, rail_y - RAIL_WIDTH / 2.0),
                    Size::new(b - a, RAIL_WIDTH),
                    color,
                );
            });

        // Hollow ghost ring at the confirmed value, shown only while interacting.
        if let Some(ghost) = self.ghost {
            frame.stroke(
                &canvas::Path::circle(Point::new(centre(ghost), rail_y), KNOB_RADIUS),
                canvas::Stroke::default().with_width(1.5).with_color(self.accent),
            );
        }

        // Solid handle at the finger, larger while dragging.
        let handle_radius = if self.ghost.is_some() {
            KNOB_RADIUS_DRAG
        } else {
            KNOB_RADIUS
        };
        frame.fill(
            &canvas::Path::circle(Point::new(handle_x, rail_y), handle_radius),
            self.accent,
        );

        vec![frame.into_geometry()]
    }
}

/// The non-empty sub-intervals of `[from, to]` with `gap` removed — used to omit
/// the rail across the ghost knob. With no gap the interval passes through whole.
fn without_gap(from: f32, to: f32, gap: Option<(f32, f32)>) -> impl Iterator<Item = (f32, f32)> {
    let (g0, g1) = gap.unwrap_or((f32::INFINITY, f32::INFINITY));
    [(from, to.min(g0)), (from.max(g1), to)]
        .into_iter()
        .filter(|(a, b)| b > a)
}
