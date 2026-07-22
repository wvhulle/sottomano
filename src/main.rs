//! odin-rs — a pure-Rust niri bar on iced + `iced_layershell`. Launches a
//! multi-output layer-shell daemon (one bar per output/edge); must run inside a
//! niri session.

mod bar_layout;
mod bar_program;
mod data_sources;
mod layer_surface;
mod message_subscriptions;
mod tooltip;
mod user_config;
mod widgets;

use bar_program::Bar;
use iced_layershell::{
    build_pattern::daemon,
    settings::{LayerShellSettings, Settings, StartMode},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    daemon(Bar::new, Bar::namespace, Bar::update, Bar::view)
        .style(Bar::style)
        .subscription(message_subscriptions::subscription)
        .settings(Settings {
            layer_settings: LayerShellSettings {
                start_mode: StartMode::Background,
                ..Default::default()
            },
            ..Default::default()
        })
        .run()?;
    Ok(())
}
