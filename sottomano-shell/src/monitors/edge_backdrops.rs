use iced::{
    Color, Element, Length, Shadow, Task, Vector,
    widget::{Space, column, container, row},
    window::Id,
};
use iced_exwlshell::reexport::{Anchor, Layer, LayerSize, Margin, NewLayerShellSettings};
use sottomano_compositor_model::compositor_styling_model::{
    EdgeShadowRendering, PANEL_EDGE_SHADOW, StyledSurface,
};
use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::layer_surfaces::base_settings;

use crate::{desktop_shell::DesktopShell, shell_message::Message};

const SELF_DRAWN_CASTER_DEPTH_PX: f32 = PANEL_EDGE_SHADOW.spread_px - PANEL_EDGE_SHADOW.lift_px;

const ICED_SHADOW_OPACITY_AT_CASTER_EDGE: f32 = 0.5;

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
        EdgeShadowRendering::SelfDrawnFalloff => {
            let caster_depth = SELF_DRAWN_CASTER_DEPTH_PX.ceil() as u32;
            BackdropStrip {
                thickness: caster_depth
                    + (SELF_DRAWN_CASTER_DEPTH_PX + PANEL_EDGE_SHADOW.softness_px).ceil() as u32,
                offscreen_depth: caster_depth,
            }
        }
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
    let depth = SELF_DRAWN_CASTER_DEPTH_PX;
    let caster = match position {
        Position::Top | Position::Bottom => {
            container(Space::new()).width(Length::Fill).height(depth)
        }
        Position::Left => container(Space::new()).width(depth).height(Length::Fill),
    }
    .style(move |_| container::Style {
        background: Some(Color::BLACK.into()),
        shadow: Shadow {
            color: Color::BLACK.scale_alpha(
                f32::from(PANEL_EDGE_SHADOW.alpha) / 255.0 / ICED_SHADOW_OPACITY_AT_CASTER_EDGE,
            ),
            offset: match position {
                Position::Top => Vector::new(0.0, depth),
                Position::Bottom => Vector::new(0.0, -depth),
                Position::Left => Vector::new(depth, 0.0),
            },
            blur_radius: PANEL_EDGE_SHADOW.softness_px,
        },
        ..container::Style::default()
    });
    let onscreen = Space::new().width(Length::Fill).height(Length::Fill);
    match position {
        Position::Top => column![caster, onscreen].into(),
        Position::Bottom => column![onscreen, caster].into(),
        Position::Left => row![caster, onscreen].into(),
    }
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

    use super::{BackdropStrip, backdrop_strip};

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
    fn a_self_drawn_caster_hides_beyond_the_edge_and_shadows_the_screen() {
        let caster_depth = (PANEL_EDGE_SHADOW.spread_px - PANEL_EDGE_SHADOW.lift_px) as u32;
        let onscreen_reach = caster_depth + PANEL_EDGE_SHADOW.softness_px as u32;
        assert_eq!(
            backdrop_strip(EdgeShadowRendering::SelfDrawnFalloff, 40),
            BackdropStrip {
                thickness: caster_depth + onscreen_reach,
                offscreen_depth: caster_depth,
            }
        );
    }
}
