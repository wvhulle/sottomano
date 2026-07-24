//! Per-toast layer-shell surface: one Wayland surface per notification, anchored
//! to its corner and sized to the card, so niri can target each toast by its
//! namespace (blur/shadow layer rules). Same anchored+`Sensor`-resized shape as
//! the tooltip surface. Cards sharing a corner stack via a per-toast margin
//! offset; a corner change re-anchors (a jump, not a slide).

use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};

use crate::{
    config::resolved::ResolvedConfig,
    data_sources::niri::{HEdge, VEdge},
};

pub const NAMESPACE: &str = "odin-rs-notifications";
/// Vertical gap between two cards sharing a corner.
pub const STACK_SPACING: f32 = 8.0;
/// Height a toast surface opens at before its card is measured. Generous on
/// purpose (like the tooltip surface): the card lays out shrink-to-fit inside
/// it, so a tall card reports its true height via the `Sensor` instead of being
/// clamped to the surface — then `SizeChange` shrinks the surface to match.
const PROVISIONAL_HEIGHT: u32 = 900;

/// The anchor for a corner. A `Center` axis is left unanchored so the compositor
/// centres the surface on that axis.
pub fn corner_anchor(vertical: VEdge, horizontal: HEdge) -> Anchor {
    let mut anchor = Anchor::empty();
    match vertical {
        VEdge::Top => anchor |= Anchor::Top,
        VEdge::Bottom => anchor |= Anchor::Bottom,
        VEdge::Center => {}
    }
    match horizontal {
        HEdge::Left => anchor |= Anchor::Left,
        HEdge::Right => anchor |= Anchor::Right,
        HEdge::Center => {}
    }
    anchor
}

/// Margin (top, right, bottom, left) placing a card `offset` px in from its
/// corner along the stacking axis. The bar height plus the configured gap insets
/// the freshest card from the panel edge; `offset` pushes older siblings inward.
/// A centered axis takes no margin (the compositor centres it).
pub fn corner_margin(
    vertical: VEdge,
    horizontal: HEdge,
    offset: f32,
    config: &ResolvedConfig,
) -> (i32, i32, i32, i32) {
    let edge = (config.layout.outer_height_px + config.notifications.gap_px + offset).round() as i32;
    let side = config.notifications.gap_px.round() as i32;
    let top = i32::from(vertical == VEdge::Top) * edge;
    let bottom = i32::from(vertical == VEdge::Bottom) * edge;
    let left = i32::from(horizontal == HEdge::Left) * side;
    let right = i32::from(horizontal == HEdge::Right) * side;
    (top, right, bottom, left)
}

/// Layer-shell settings for one toast surface, bound to `output` (or the active
/// output when none resolves).
pub fn toast_settings(
    output: Option<&str>,
    anchor: Anchor,
    margin: (i32, i32, i32, i32),
    size: (u32, u32),
) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: Some(size),
        exclusive_zone: Some(0),
        anchor,
        layer: Layer::Overlay,
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: output.map_or(OutputOption::Active, |o| {
            OutputOption::OutputName(o.to_string())
        }),
        margin: Some(margin),
        events_transparent: false,
        namespace: Some(NAMESPACE.to_string()),
    }
}

/// The surface height to open/resize a toast to, given its measured card height
/// (or the provisional height before the first measurement).
pub fn surface_height(measured: Option<f32>) -> u32 {
    measured.map_or(PROVISIONAL_HEIGHT, |h| h.ceil() as u32)
}
