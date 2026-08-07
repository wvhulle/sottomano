mod app;
mod blend;
mod config;
mod data;
mod message_subscriptions;
mod state;
mod wayland_layers;
mod widget_groups;
mod widgets;

use std::error::Error;

use app::Bar;
use iced_layershell::{
    build_pattern::daemon,
    settings::{LayerShellSettings, Settings, StartMode},
};

fn main() -> Result<(), Box<dyn Error>> {
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
