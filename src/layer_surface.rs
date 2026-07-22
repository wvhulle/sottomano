//! Layer-shell surface/output bookkeeping: which screen edge a surface anchors
//! to, the identity of one open surface (output + edge), and the
//! `NewLayerShellSettings` that open it.

use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};

/// Which screen edge a surface anchors to. Odin opens both per screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Top,
    Bottom,
}

impl Position {
    pub(crate) const ALL: [Position; 2] = [Position::Top, Position::Bottom];

    /// Full-width bar anchored to this edge; left+right makes width
    /// compositor-driven.
    fn anchor(self) -> Anchor {
        let edge = match self {
            Position::Top => Anchor::Top,
            Position::Bottom => Anchor::Bottom,
        };
        edge | Anchor::Left | Anchor::Right
    }
}

/// The identity of one open surface: which output it lives on and which edge it
/// hugs. Rendering picks out that output's workspaces — the whole point of
/// going multi-output.
#[derive(Debug, Clone)]
pub struct Surface {
    pub output: String,
    pub position: Position,
}

/// Margin (top, right, bottom, left) that insets a bar from the screen edges
/// in floating mode — `0` on every side flush-mounts it instead.
pub fn float_margin(position: Position, offset_px: i32) -> (i32, i32, i32, i32) {
    match position {
        Position::Top => (offset_px, offset_px, 0, offset_px),
        Position::Bottom => (0, offset_px, offset_px, offset_px),
    }
}

/// Layer-shell settings for one bar. The bar reserves its own strip via an
/// exclusive zone equal to its height (odin's `exclusiveZone: barOuterHeight`),
/// so niri tiles windows clear of it instead of letting them overlap. `height`
/// is odin's `barOuterHeight` from the shared config; it also stacks below
/// odin's own reserved zone, so the two bars sit flush without overlapping
/// windows. `margin` is odin's floating inset — `(0, 0, 0, 0)` when flush.
pub fn layer_settings(
    output: &str,
    position: Position,
    height: u32,
    margin: (i32, i32, i32, i32),
) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: Some((0, height)),
        exclusive_zone: Some(height as i32),
        anchor: position.anchor(),
        layer: Layer::Top,
        // Never take keyboard focus — mirrors Odin's WlrKeyboardFocus.None.
        keyboard_interactivity: KeyboardInteractivity::None,
        // Bind this surface to a specific niri output by name.
        output_option: OutputOption::OutputName(output.to_string()),
        margin: Some(margin),
        ..Default::default()
    }
}
