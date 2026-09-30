use iced::{Element, Length, Rectangle, widget::Space, window::Id};
pub use iced_exwlshell::reexport::Margin;
use iced_exwlshell::{
    actions::ActionCallback,
    reexport::{
        Anchor, KeyboardInteractivity, Layer, LayerSize, NewLayerShellSettings, OutputOption,
    },
};

use crate::{app::Message, wayland_layers::niri_rules::StyledSurface};

pub fn blank<'a>() -> Element<'a, Message> {
    Space::new().width(Length::Fill).height(Length::Fill).into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Top,
    Bottom,
    Left,
}

impl Position {
    pub(crate) const BARS: [Position; 2] = [Position::Top, Position::Bottom];
    pub(crate) const ALL: [Position; 3] = [Position::Top, Position::Bottom, Position::Left];

    pub(crate) fn edge(self) -> Anchor {
        match self {
            Position::Top => Anchor::Top,
            Position::Bottom => Anchor::Bottom,
            Position::Left => Anchor::Left,
        }
    }

    pub(crate) fn anchor(self) -> Anchor {
        match self {
            Position::Top | Position::Bottom => self.edge() | Anchor::Left | Anchor::Right,
            Position::Left => self.edge() | Anchor::Top | Anchor::Bottom,
        }
    }

    pub(crate) fn strip_size(self, thickness: u32) -> LayerSize {
        match self {
            Position::Top | Position::Bottom => LayerSize::fill_width(thickness),
            Position::Left => LayerSize::fill_height(thickness),
        }
    }

    pub(crate) fn is_vertical(self) -> bool {
        matches!(self, Position::Left)
    }
}

pub(crate) fn input_region(surface: Id, bounds: Rectangle) -> Message {
    input_regions(surface, vec![bounds])
}

pub(crate) fn input_regions(surface: Id, rects: Vec<Rectangle>) -> Message {
    Message::SetInputRegion {
        id: surface,
        callback: ActionCallback::new(move |region| {
            rects
                .iter()
                .filter(|rect| rect.width > 0.0 && rect.height > 0.0)
                .for_each(|rect| {
                    region.add(
                        rect.x as i32,
                        rect.y as i32,
                        rect.width.ceil() as i32,
                        rect.height.ceil() as i32,
                    );
                });
        }),
    }
}

pub(crate) fn base_settings(output: Option<&str>, namespace: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: output.map_or(OutputOption::Active, |name| {
            OutputOption::OutputName(name.to_string())
        }),
        namespace: Some(namespace.to_string()),
        ..Default::default()
    }
}

pub(crate) fn panel_settings(output: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: LayerSize::FILL,
        exclusive_zone: Some(-1),
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        layer: Layer::Top,
        ..base_settings(Some(output), StyledSurface::Panel.namespace())
    }
}

pub(crate) fn reservation_settings(
    output: &str,
    position: Position,
    zone: i32,
) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: position.strip_size(1),
        exclusive_zone: Some(zone),
        anchor: position.anchor(),
        layer: Layer::Bottom,
        events_transparent: true,
        ..base_settings(Some(output), "vetrata-reservation")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceRole {
    Panel,
    Reservation(Position),
}

#[derive(Debug, Clone)]
pub struct Surface {
    pub output: String,
    pub role: SurfaceRole,
}

#[derive(Debug, Clone)]
pub struct PanelEdge {
    pub output: String,
    pub position: Position,
}
