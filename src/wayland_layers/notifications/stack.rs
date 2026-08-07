use iced_layershell::reexport::{Anchor, Layer, NewLayerShellSettings};

use crate::{
    config::Config,
    data::source::niri::{HEdge, VEdge},
    wayland_layers::surface::{self, Margin, SurfaceSize},
};

pub struct SlotPlacement {
    pub anchor: Anchor,
    pub margin: Margin,
}

pub const NAMESPACE: &str = "vetrata-notifications";
pub const STACK_SPACING: f32 = 8.0;

pub fn slot_placement(
    vertical: VEdge,
    horizontal: HEdge,
    offset: f32,
    group_height: f32,
    output_height: Option<i32>,
    config: &Config,
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

fn edge_inset(offset: f32, config: &Config) -> i32 {
    (config.chrome.outer_height_px + config.notifications.edge_gap + offset).round() as i32
}

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
        margin: Some(margin.edges()),
        ..surface::base_settings(output, NAMESPACE)
    }
}

pub fn surface_height(height: f32) -> u32 {
    height.floor() as u32
}
