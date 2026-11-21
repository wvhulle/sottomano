use std::f32::consts::{FRAC_PI_2, PI};

use iced::{
    Background, Color, Element, Gradient, Length, Radians,
    gradient::Linear,
    widget::{Space, container},
};
use iced_exwlshell::reexport::Margin;
use sottomano_compositor_model::compositor_styling_model::PANEL_EDGE_SHADOW;
use sottomano_toml_config::screen_edges::Position;

use super::{BackdropPlacement, panel_edge_strip_size};

const FALLOFF_GRADIENT_STOPS: u8 = 8;

pub(super) fn onscreen_falloff_placement(position: Position) -> BackdropPlacement {
    BackdropPlacement {
        size: panel_edge_strip_size(position, PANEL_EDGE_SHADOW.reach_px() as u32),
        margin: Margin::default(),
    }
}

fn away_from_edge(position: Position) -> Radians {
    match position {
        Position::Top => Radians(PI),
        Position::Bottom => Radians(0.0),
        Position::Left => Radians(FRAC_PI_2),
    }
}

fn falloff_gradient(position: Position) -> Linear {
    let reach = PANEL_EDGE_SHADOW.reach_px();
    (0..FALLOFF_GRADIENT_STOPS).fold(
        Linear::new(away_from_edge(position)),
        |gradient, stop_index| {
            let offset = f32::from(stop_index) / f32::from(FALLOFF_GRADIENT_STOPS - 1);
            gradient.add_stop(
                offset,
                Color {
                    a: PANEL_EDGE_SHADOW.opacity_at(offset * reach),
                    ..Color::BLACK
                },
            )
        },
    )
}

pub(super) fn gradient_falloff_view<'a, Interaction: 'a>(
    position: Position,
) -> Element<'a, Interaction> {
    container(Space::new().width(Length::Fill).height(Length::Fill))
        .style(move |_| container::Style {
            background: Some(Background::Gradient(Gradient::Linear(falloff_gradient(
                position,
            )))),
            ..container::Style::default()
        })
        .into()
}
