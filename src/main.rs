//! odin-rs — a pure-Rust niri bar on iced + iced_layershell. Launches a multi-output
//! layer-shell daemon (one bar per output/edge); must run inside a niri session.

mod bar_layout;
mod bar_program;
mod data_sources;
mod layer_surface;
mod message_subscriptions;
mod user_config;
mod visual_elements;

use bar_program::Bar;
use iced_layershell::build_pattern::daemon;
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    daemon(Bar::new, Bar::namespace, Bar::update, Bar::view)
        .style(Bar::style)
        .subscription(message_subscriptions::subscription)
        .settings(Settings {
            layer_settings: LayerShellSettings {
                // `Background` opens no surface up-front: `Bar` enumerates niri outputs
                // and opens one `NewLayerShell` per (output, edge) itself, binding each
                // to its output by name. Per-surface size/anchor/zone come from those
                // `NewLayerShellSettings`, so nothing else is set here.
                start_mode: StartMode::Background,
                ..Default::default()
            },
            ..Default::default()
        })
        .run()?;
    Ok(())
}
