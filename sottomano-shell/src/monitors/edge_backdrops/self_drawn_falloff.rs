use iced::{
    Color, Element, Length, Shadow, Vector,
    widget::{Space, column, container, row},
};
use iced_exwlshell::reexport::Margin;
use sottomano_compositor_model::compositor_styling_model::PANEL_EDGE_SHADOW;
use sottomano_toml_config::screen_edges::Position;

use super::{BackdropPlacement, panel_edge_strip_size};

const ICED_SHADOW_OPACITY_AT_CASTER_EDGE: f32 = 0.5;

fn caster_depth_px() -> f32 {
    PANEL_EDGE_SHADOW.spread_px - PANEL_EDGE_SHADOW.lift_px
}

fn onscreen_reach_px() -> f32 {
    caster_depth_px() + PANEL_EDGE_SHADOW.softness_px
}

pub(super) fn edge_caster_placement(position: Position) -> BackdropPlacement {
    let caster_depth = caster_depth_px().ceil() as u32;
    let offscreen = -caster_depth.cast_signed();
    let margin = match position {
        Position::Top => Margin {
            top: offscreen,
            ..Margin::default()
        },
        Position::Bottom => Margin {
            bottom: offscreen,
            ..Margin::default()
        },
        Position::Left => Margin {
            left: offscreen,
            ..Margin::default()
        },
    };
    BackdropPlacement {
        size: panel_edge_strip_size(position, caster_depth + onscreen_reach_px().ceil() as u32),
        margin,
    }
}

fn shadow_onto_screen(position: Position) -> Vector {
    let depth = caster_depth_px();
    match position {
        Position::Top => Vector::new(0.0, depth),
        Position::Bottom => Vector::new(0.0, -depth),
        Position::Left => Vector::new(depth, 0.0),
    }
}

fn offscreen_caster<'a, Interaction: 'a>(position: Position) -> Element<'a, Interaction> {
    let caster = match position {
        Position::Top | Position::Bottom => container(Space::new())
            .width(Length::Fill)
            .height(caster_depth_px()),
        Position::Left => container(Space::new())
            .width(caster_depth_px())
            .height(Length::Fill),
    };
    caster
        .style(move |_| container::Style {
            background: Some(Color::BLACK.into()),
            shadow: Shadow {
                color: Color::BLACK.scale_alpha(
                    f32::from(PANEL_EDGE_SHADOW.alpha) / 255.0 / ICED_SHADOW_OPACITY_AT_CASTER_EDGE,
                ),
                offset: shadow_onto_screen(position),
                blur_radius: PANEL_EDGE_SHADOW.softness_px,
            },
            ..container::Style::default()
        })
        .into()
}

pub(super) fn shadow_casting_view<'a, Interaction: 'a>(
    position: Position,
) -> Element<'a, Interaction> {
    let onscreen = || Space::new().width(Length::Fill).height(Length::Fill);
    match position {
        Position::Top => column![offscreen_caster(position), onscreen()].into(),
        Position::Bottom => column![onscreen(), offscreen_caster(position)].into(),
        Position::Left => row![offscreen_caster(position), onscreen()].into(),
    }
}
