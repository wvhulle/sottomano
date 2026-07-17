//! odin-rs — a pure-Rust niri bar on iced + iced_layershell. Launches a multi-output
//! layer-shell daemon (one bar per output/edge); must run inside a niri session.

mod app;
mod config;
mod desktop;
mod niri;
mod source;
mod widget;

use app::Bar;
use iced_layershell::build_pattern::daemon;
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    daemon(Bar::new, Bar::namespace, Bar::update, Bar::view)
        .style(Bar::style)
        .subscription(app::subscription)
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
