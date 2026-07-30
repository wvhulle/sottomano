//! Per-toast layer-shell surface: one Wayland surface per notification,
//! anchored to its corner and sized to the card's pre-measured height, so niri
//! can target each toast by its namespace (blur/shadow layer rules). Cards
//! sharing a corner stack via a per-toast margin offset; a corner change
//! re-anchors (a jump, not a slide).

use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};

use crate::{
    config::resolved::ResolvedConfig,
    data::source::niri::{HEdge, VEdge},
    wayland_layers::surface::{Margin, SurfaceSize},
};

pub struct SlotPlacement {
    pub anchor: Anchor,
    pub margin: Margin,
}

pub const NAMESPACE: &str = "vetrata-notifications";
/// Vertical gap between two cards sharing a corner.
pub const STACK_SPACING: f32 = 8.0;

/// The anchor and margin (top, right, bottom, left) for one card in its corner
/// group. Anchored axes (top / bottom) inset the freshest card from the panel
/// edge and push older siblings inward by `offset`.
///
/// A centred *vertical* stack has no edge to offset against — the compositor
/// would centre each surface's own midpoint, so cards of different heights
/// overlap. It is instead anchored to the top and given an absolute margin that
/// centres the whole `group_height` on the output (odin's
/// `(win.height - groupHeight) / 2 + offset`), with `offset` stepping within
/// it. When the output height is unknown it falls back to top-anchoring, so
/// cards still stack rather than pile up. A centred *horizontal* axis stays
/// unanchored (the compositor centres it, which is what we want).
pub fn slot_placement(
    vertical: VEdge,
    horizontal: HEdge,
    offset: f32,
    group_height: f32,
    output_height: Option<i32>,
    config: &ResolvedConfig,
) -> SlotPlacement {
    let side = config.notifications.edge_gap.round() as i32;
    let mut anchor = Anchor::empty();
    let (mut left, mut right) = (0, 0);
    match horizontal {
        HEdge::Left => {
            anchor |= Anchor::Left;
            left = side;
        }
        HEdge::Right => {
            anchor |= Anchor::Right;
            right = side;
        }
        HEdge::Center => {}
    }

    let (top, bottom) = match vertical {
        VEdge::Top => {
            anchor |= Anchor::Top;
            (edge_inset(offset, config), 0)
        }
        VEdge::Bottom => {
            anchor |= Anchor::Bottom;
            (0, edge_inset(offset, config))
        }
        VEdge::Center => {
            anchor |= Anchor::Top;
            let top = match output_height {
                Some(height) => {
                    let origin = ((height as f32 - group_height) / 2.0).max(0.0);
                    (origin + offset).round() as i32
                }
                None => edge_inset(offset, config),
            };
            (top, 0)
        }
    };
    SlotPlacement {
        anchor,
        margin: Margin {
            top,
            right,
            bottom,
            left,
        },
    }
}

/// How far a card's edge sits from the panel edge on an anchored axis: the bar
/// height, the configured gap, and the stacking `offset` for older siblings.
fn edge_inset(offset: f32, config: &ResolvedConfig) -> i32 {
    (config.layout.outer_height_px + config.notifications.edge_gap + offset).round() as i32
}

/// Layer-shell settings for one toast surface, bound to `output` (or the active
/// output when none resolves).
pub fn toast_settings(
    output: Option<&str>,
    anchor: Anchor,
    margin: Margin,
    size: SurfaceSize,
) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: Some(size.dimensions()),
        exclusive_zone: Some(0),
        anchor,
        layer: Layer::Overlay,
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: output.map_or(OutputOption::Active, |o| {
            OutputOption::OutputName(o.to_string())
        }),
        margin: Some(margin.edges()),
        events_transparent: false,
        namespace: Some(NAMESPACE.to_string()),
    }
}

pub fn surface_height(height: f32) -> u32 {
    height.ceil() as u32
}
