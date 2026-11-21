use std::f32::consts::{FRAC_PI_2, PI, SQRT_2};

use iced::{
    Background, Color, Element, Gradient, Length, Radians, Task,
    gradient::Linear,
    widget::{Space, container},
    window::Id,
};
use iced_exwlshell::reexport::{Anchor, Layer, LayerSize, Margin, NewLayerShellSettings};
use sottomano_compositor_model::compositor_styling_model::{
    EdgeShadowRendering, PANEL_EDGE_SHADOW, StyledSurface,
};
use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::layer_surfaces::base_settings;

use crate::{desktop_shell::DesktopShell, shell_message::Message};

const SHADOW_HALF_STRENGTH_DEPTH_PX: f32 = PANEL_EDGE_SHADOW.spread_px - PANEL_EDGE_SHADOW.lift_px;

const SHADOW_GAUSSIAN_SIGMA_PX: f32 = PANEL_EDGE_SHADOW.softness_px / 2.0;

const SHADOW_TAIL_SIGMA_COUNT: f32 = 3.0;

const FALLOFF_GRADIENT_STOP_COUNT: usize = 8;

fn self_drawn_falloff_reach_px() -> u32 {
    (SHADOW_HALF_STRENGTH_DEPTH_PX + SHADOW_TAIL_SIGMA_COUNT * SHADOW_GAUSSIAN_SIGMA_PX).ceil()
        as u32
}

fn edge_shadow_alpha_at(distance_from_screen_edge_px: f32) -> f32 {
    let peak_alpha = f32::from(PANEL_EDGE_SHADOW.alpha) / 255.0;
    let normalized_distance = (distance_from_screen_edge_px - SHADOW_HALF_STRENGTH_DEPTH_PX)
        / (SHADOW_GAUSSIAN_SIGMA_PX * SQRT_2);
    peak_alpha * 0.5 * libm::erfcf(normalized_distance)
}

fn falloff_away_from_edge(position: Position) -> Radians {
    match position {
        Position::Top => Radians(PI),
        Position::Bottom => Radians(0.0),
        Position::Left => Radians(FRAC_PI_2),
    }
}

pub(crate) fn panel_edge_anchor(position: Position) -> Anchor {
    match position {
        Position::Top => Anchor::Top | Anchor::Left | Anchor::Right,
        Position::Bottom => Anchor::Bottom | Anchor::Left | Anchor::Right,
        Position::Left => Anchor::Left | Anchor::Top | Anchor::Bottom,
    }
}

pub(crate) fn panel_edge_strip_size(position: Position, thickness: u32) -> LayerSize {
    match position {
        Position::Top | Position::Bottom => LayerSize::fill_width(thickness),
        Position::Left => LayerSize::fill_height(thickness),
    }
}

fn offscreen_margin(position: Position, offscreen_depth: u32) -> Margin {
    let pulled_out = -offscreen_depth.cast_signed();
    match position {
        Position::Top => Margin {
            top: pulled_out,
            ..Margin::default()
        },
        Position::Bottom => Margin {
            bottom: pulled_out,
            ..Margin::default()
        },
        Position::Left => Margin {
            left: pulled_out,
            ..Margin::default()
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BackdropStrip {
    thickness: u32,
    offscreen_depth: u32,
}

fn backdrop_strip(rendering: EdgeShadowRendering, panel_height: u32) -> BackdropStrip {
    match rendering {
        EdgeShadowRendering::CompositorShadow => BackdropStrip {
            thickness: panel_height,
            offscreen_depth: panel_height,
        },
        EdgeShadowRendering::SelfDrawnFalloff => BackdropStrip {
            thickness: self_drawn_falloff_reach_px(),
            offscreen_depth: 0,
        },
    }
}

pub(crate) fn backdrop_view<'a, Interaction: 'a>(
    rendering: EdgeShadowRendering,
    position: Position,
) -> Element<'a, Interaction> {
    match rendering {
        EdgeShadowRendering::CompositorShadow => {
            container(Space::new().width(Length::Fill).height(Length::Fill))
                .style(|_| container::background(Color::BLACK))
                .into()
        }
        EdgeShadowRendering::SelfDrawnFalloff => self_drawn_shadow_view(position),
    }
}

fn self_drawn_shadow_view<'a, Interaction: 'a>(position: Position) -> Element<'a, Interaction> {
    let reach_px = self_drawn_falloff_reach_px() as f32;
    let last_stop_index = (FALLOFF_GRADIENT_STOP_COUNT - 1) as f32;
    let falloff_gradient = (0..FALLOFF_GRADIENT_STOP_COUNT).fold(
        Linear::new(falloff_away_from_edge(position)),
        |gradient, stop_index| {
            let stop_offset = stop_index as f32 / last_stop_index;
            gradient.add_stop(
                stop_offset,
                Color::BLACK.scale_alpha(edge_shadow_alpha_at(stop_offset * reach_px)),
            )
        },
    );
    container(Space::new().width(Length::Fill).height(Length::Fill))
        .style(move |_| {
            container::background(Background::Gradient(Gradient::Linear(falloff_gradient)))
        })
        .into()
}

impl DesktopShell {
    pub(crate) fn open_backdrop(&mut self, output: &str, position: Position) -> Task<Message> {
        let rendering = self.edge_shadow_rendering;
        let strip = backdrop_strip(rendering, self.config.desktop_panels.height as u32);
        let id = Id::unique();
        self.backdrops.insert(id, position);
        Task::done(Message::NewLayerShell {
            settings: NewLayerShellSettings {
                size: panel_edge_strip_size(position, strip.thickness),
                exclusive_zone: Some(-1),
                anchor: panel_edge_anchor(position),
                layer: match rendering {
                    EdgeShadowRendering::CompositorShadow => Layer::Background,
                    EdgeShadowRendering::SelfDrawnFalloff => Layer::Bottom,
                },
                margin: Some(offscreen_margin(position, strip.offscreen_depth)),
                events_transparent: true,
                ..base_settings(Some(output), StyledSurface::Backdrop(position).namespace())
            },
            id,
        })
    }

    pub fn resize_backdrops(&self) -> Task<Message> {
        let strip = backdrop_strip(
            self.edge_shadow_rendering,
            self.config.desktop_panels.height as u32,
        );
        Task::batch(self.backdrops.iter().flat_map(move |(&id, &position)| {
            [
                Task::done(Message::LayoutChange {
                    id,
                    anchor: panel_edge_anchor(position),
                    size: panel_edge_strip_size(position, strip.thickness),
                }),
                Task::done(Message::MarginChange {
                    id,
                    margin: offscreen_margin(position, strip.offscreen_depth),
                }),
            ]
        }))
    }
}

#[cfg(test)]
mod tests {
    use sottomano_compositor_model::compositor_styling_model::{
        EdgeShadowRendering, PANEL_EDGE_SHADOW,
    };

    use super::{
        BackdropStrip, SHADOW_HALF_STRENGTH_DEPTH_PX, backdrop_strip, edge_shadow_alpha_at,
        self_drawn_falloff_reach_px,
    };

    fn peak_alpha() -> f32 {
        f32::from(PANEL_EDGE_SHADOW.alpha) / 255.0
    }

    #[test]
    fn a_compositor_shadow_caster_sits_just_beyond_the_screen_edge() {
        assert_eq!(
            backdrop_strip(EdgeShadowRendering::CompositorShadow, 40),
            BackdropStrip {
                thickness: 40,
                offscreen_depth: 40,
            }
        );
    }

    #[test]
    fn a_self_drawn_falloff_starts_at_the_screen_edge_and_spans_the_gaussian_tail() {
        assert_eq!(
            backdrop_strip(EdgeShadowRendering::SelfDrawnFalloff, 40),
            BackdropStrip {
                thickness: self_drawn_falloff_reach_px(),
                offscreen_depth: 0,
            }
        );
    }

    #[test]
    fn the_falloff_reaches_half_strength_where_niri_spreads_the_caster() {
        let half_strength = edge_shadow_alpha_at(SHADOW_HALF_STRENGTH_DEPTH_PX);
        assert!((half_strength - peak_alpha() / 2.0).abs() < 1e-4);
    }

    #[test]
    fn the_falloff_is_darkest_at_the_edge_and_vanishes_at_its_reach() {
        assert!(edge_shadow_alpha_at(0.0) > 0.9 * peak_alpha());
        assert!(edge_shadow_alpha_at(self_drawn_falloff_reach_px() as f32) < 0.002 * peak_alpha());
    }

    #[test]
    fn the_falloff_decreases_steadily_without_a_flat_band() {
        let reach_px = self_drawn_falloff_reach_px();
        let alphas: Vec<f32> = (0..=reach_px)
            .map(|distance_px| edge_shadow_alpha_at(distance_px as f32))
            .collect();
        assert!(alphas.windows(2).all(|neighbours| neighbours[1] < neighbours[0]));
    }
}
