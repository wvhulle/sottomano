//! Layer-shell surface/output bookkeeping: which screen edge a surface anchors to, the
//! identity of one open surface (output + edge), and the `NewLayerShellSettings` that
//! open it.

use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption};

/// Which screen edge a surface anchors to. Odin opens both per screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Top,
    Bottom,
}

impl Position {
    pub(crate) const ALL: [Position; 2] = [Position::Top, Position::Bottom];

    /// Full-width bar anchored to this edge; left+right makes width compositor-driven.
    fn anchor(self) -> Anchor {
        let edge = match self {
            Position::Top => Anchor::Top,
            Position::Bottom => Anchor::Bottom,
        };
        edge | Anchor::Left | Anchor::Right
    }
}

/// The identity of one open surface: which output it lives on and which edge it hugs.
/// `Bar::view` renders that output's workspaces — the whole point of going multi-output.
#[derive(Debug, Clone)]
pub struct Surface {
    pub output: String,
    pub position: Position,
}

/// Layer-shell settings for one bar. The bar reserves its own strip via an exclusive
/// zone equal to its height (odin's `exclusiveZone: barOuterHeight`), so niri tiles
/// windows clear of it instead of letting them overlap. `height` is odin's
/// `barOuterHeight` from the shared config; it also stacks below odin's own reserved
/// zone, so the two bars sit flush without overlapping windows.
pub fn layer_settings(output: &str, position: Position, height: u32) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: Some((0, height)),
        exclusive_zone: Some(height as i32),
        anchor: position.anchor(),
        layer: Layer::Top,
        // Never take keyboard focus — mirrors Odin's WlrKeyboardFocus.None.
        keyboard_interactivity: KeyboardInteractivity::None,
        // Bind this surface to a specific niri output by name.
        output_option: OutputOption::OutputName(output.to_string()),
        ..Default::default()
    }
}
