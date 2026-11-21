mod compositor_drawn_shadow;
mod self_drawn_falloff;

use iced::{Element, Task, window::Id};
use iced_exwlshell::reexport::{Anchor, Layer, LayerSize, Margin, NewLayerShellSettings};
use sottomano_compositor_model::compositor_styling_model::{EdgeShadowRendering, StyledSurface};
use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::layer_surfaces::base_settings;

use crate::{desktop_shell::DesktopShell, shell_message::Message};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BackdropPlacement {
    size: LayerSize,
    margin: Margin,
}

fn backdrop_placement(
    rendering: EdgeShadowRendering,
    position: Position,
    panel_height: u32,
) -> BackdropPlacement {
    match rendering {
        EdgeShadowRendering::CompositorShadow => {
            compositor_drawn_shadow::offscreen_caster_placement(position, panel_height)
        }
        EdgeShadowRendering::SelfDrawnFalloff => {
            self_drawn_falloff::onscreen_falloff_placement(position)
        }
    }
}

pub(crate) fn backdrop_view<'a, Interaction: 'a>(
    rendering: EdgeShadowRendering,
    position: Position,
) -> Element<'a, Interaction> {
    match rendering {
        EdgeShadowRendering::CompositorShadow => compositor_drawn_shadow::opaque_caster_view(),
        EdgeShadowRendering::SelfDrawnFalloff => {
            self_drawn_falloff::gradient_falloff_view(position)
        }
    }
}

fn backdrop_settings(
    output: &str,
    position: Position,
    placement: BackdropPlacement,
) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: placement.size,
        exclusive_zone: Some(-1),
        anchor: panel_edge_anchor(position),
        layer: Layer::Background,
        margin: Some(placement.margin),
        events_transparent: true,
        ..base_settings(Some(output), StyledSurface::Backdrop(position).namespace())
    }
}

impl DesktopShell {
    pub(crate) fn open_backdrop(&mut self, output: &str, position: Position) -> Task<Message> {
        let placement = backdrop_placement(
            self.edge_shadow_rendering,
            position,
            self.config.desktop_panels.height as u32,
        );
        let id = Id::unique();
        self.backdrops.insert(id, position);
        Task::done(Message::NewLayerShell {
            settings: backdrop_settings(output, position, placement),
            id,
        })
    }

    pub fn resize_backdrops(&self) -> Task<Message> {
        let panel_height = self.config.desktop_panels.height as u32;
        let rendering = self.edge_shadow_rendering;
        Task::batch(self.backdrops.iter().flat_map(move |(&id, &position)| {
            let placement = backdrop_placement(rendering, position, panel_height);
            [
                Task::done(Message::LayoutChange {
                    id,
                    anchor: panel_edge_anchor(position),
                    size: placement.size,
                }),
                Task::done(Message::MarginChange {
                    id,
                    margin: placement.margin,
                }),
            ]
        }))
    }
}

#[cfg(test)]
mod tests {
    use iced_exwlshell::reexport::{LayerSize, Margin};
    use sottomano_compositor_model::compositor_styling_model::{
        EdgeShadowRendering, PANEL_EDGE_SHADOW,
    };
    use sottomano_toml_config::screen_edges::Position;

    use super::{BackdropPlacement, backdrop_placement};

    #[test]
    fn a_compositor_shadow_caster_sits_just_beyond_the_screen_edge() {
        assert_eq!(
            backdrop_placement(EdgeShadowRendering::CompositorShadow, Position::Top, 40),
            BackdropPlacement {
                size: LayerSize::fill_width(40),
                margin: Margin {
                    top: -40,
                    ..Margin::default()
                },
            }
        );
    }

    #[test]
    fn a_self_drawn_falloff_covers_its_reach_on_screen() {
        assert_eq!(
            backdrop_placement(EdgeShadowRendering::SelfDrawnFalloff, Position::Bottom, 40),
            BackdropPlacement {
                size: LayerSize::fill_width(PANEL_EDGE_SHADOW.reach_px() as u32),
                margin: Margin::default(),
            }
        );
    }
}
