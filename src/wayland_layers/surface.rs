use iced::{Element, Length, Rectangle, widget::Space, window::Id};
pub use iced_exwlshell::reexport::Margin;
use iced_exwlshell::{
    actions::ActionCallback,
    reexport::{Anchor, KeyboardInteractivity, LayerSize, NewLayerShellSettings, OutputOption},
};

use crate::app::Message;

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

    pub(crate) fn float_margin(self, offset_px: i32) -> Margin {
        match self {
            Position::Top => Margin {
                top: offset_px,
                right: offset_px,
                bottom: 0,
                left: offset_px,
            },
            Position::Bottom => Margin {
                top: 0,
                right: offset_px,
                bottom: offset_px,
                left: offset_px,
            },
            Position::Left => Margin {
                top: 0,
                right: 0,
                bottom: 0,
                left: offset_px,
            },
        }
    }

    pub(crate) fn is_vertical(self) -> bool {
        matches!(self, Position::Left)
    }
}

pub(crate) fn input_region(surface: Id, bounds: Rectangle) -> Message {
    Message::SetInputRegion {
        id: surface,
        callback: ActionCallback::new(move |region| {
            if bounds.width > 0.0 && bounds.height > 0.0 {
                region.add(
                    bounds.x as i32,
                    bounds.y as i32,
                    bounds.width as i32,
                    bounds.height as i32,
                );
            }
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

#[derive(Debug, Clone)]
pub struct Surface {
    pub output: String,
    pub position: Position,
}
