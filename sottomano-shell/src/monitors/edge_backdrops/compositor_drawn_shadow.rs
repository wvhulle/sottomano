use iced::{
    Color, Element, Length,
    widget::{Space, container},
};
use iced_exwlshell::reexport::Margin;
use sottomano_toml_config::screen_edges::Position;

use super::{BackdropPlacement, panel_edge_strip_size};

pub(super) fn offscreen_caster_placement(
    position: Position,
    panel_height: u32,
) -> BackdropPlacement {
    let offscreen = -panel_height.cast_signed();
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
        size: panel_edge_strip_size(position, panel_height),
        margin,
    }
}

pub(super) fn opaque_caster_view<'a, Interaction: 'a>() -> Element<'a, Interaction> {
    container(Space::new().width(Length::Fill).height(Length::Fill))
        .style(|_| container::background(Color::BLACK))
        .into()
}
