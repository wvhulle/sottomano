//! The backdrop shadow: an off-screen caster surface per bar edge whose only
//! job is to make niri bloom a gradient drop-shadow back onto the screen edge,
//! so the bar's white text stays legible over any window (odin's
//! `GradientBackdrop`). The caster itself is never seen — only the niri shadow
//! it casts.

use iced::{
    Color, Element, Length,
    widget::{Space, container},
};
use iced_layershell::reexport::{
    KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};

use crate::{app::Message, wayland_layers::surface::Position};

/// niri layer-shell namespace of the shadow caster for this edge, matched by
/// the `^vetrata-backdrop-<pos>$` layer-rules that carry the gradient shadow.
pub fn namespace(position: Position) -> &'static str {
    match position {
        Position::Top => "vetrata-backdrop-top",
        Position::Bottom => "vetrata-backdrop-bottom",
    }
}

/// The off-screen caster surface: a full-width opaque strip on the background
/// layer, pushed entirely off-screen by a negative margin equal to its height
/// so only its niri drop-shadow (from the `^vetrata-backdrop-<pos>$`
/// layer-rules) blooms back onto the screen. A niri shadow never draws inside
/// its own surface rect, so the caster must sit off the edge with its inner
/// edge flush to it.
pub fn settings(output: &str, position: Position, height: u32) -> NewLayerShellSettings {
    let h = height as i32;
    let margin = match position {
        Position::Top => (-h, 0, 0, 0),
        Position::Bottom => (0, 0, -h, 0),
    };
    NewLayerShellSettings {
        size: Some((0, height)),
        // -1 = "ignore exclusive zones" (like odin's ExclusionMode.Ignore).
        // With 0, niri positions the caster inside the usable area and clamps it
        // on-screen, defeating the negative margin that should push it fully off
        // the edge — leaving an opaque black strip behind the transparent bar.
        exclusive_zone: Some(-1),
        anchor: position.anchor(),
        layer: Layer::Background,
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: OutputOption::OutputName(output.to_string()),
        margin: Some(margin),
        events_transparent: true,
        namespace: Some(namespace(position).to_string()),
    }
}

/// The caster's fill: opaque black. niri casts the edge shadow from its
/// silhouette, so only opacity matters — the shadow's colour/alpha comes from
/// the layer-rule, not this fill.
pub fn fill<'a>() -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Color::BLACK.into()),
            ..Default::default()
        })
        .into()
}
