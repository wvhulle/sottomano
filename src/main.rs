//! odin-rs — a pure-Rust niri bar on iced + `iced_layershell`. Launches a
//! multi-output layer-shell daemon (one bar per output/edge); must run inside a
//! niri session.

mod bar_layout;
mod bar_program;
mod data_sources;
mod layer_surface;
mod message_subscriptions;
mod tooltip;
mod config;
mod widgets;

use bar_program::Bar;
use iced_layershell::{
    build_pattern::daemon,
    settings::{LayerShellSettings, Settings, StartMode},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load config (which resolves the sans-serif font) before building the daemon,
    // so we can register that font and make it the default *before* the first
    // frame — otherwise iced resolves no matching family and text renders in the
    // Font Awesome icon font (tofu). Hand the same config to the bar.
    let config = config::resolved::load();

    let mut settings = Settings {
        layer_settings: LayerShellSettings {
            start_mode: StartMode::Background,
            ..Default::default()
        },
        default_font: config.font.iced_font,
        ..Default::default()
    };
    settings.fonts.push(config.font.data.to_vec().into());

    daemon(
        move || Bar::new(config.clone()),
        Bar::namespace,
        Bar::update,
        Bar::view,
    )
    .style(Bar::style)
    .subscription(message_subscriptions::subscription)
    .settings(settings)
    .run()?;
    Ok(())
}
