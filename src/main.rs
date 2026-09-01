mod app;
mod config;
mod data;
mod message_subscriptions;
mod state;
mod wayland_layers;
mod widget_groups;
mod widgets;

use std::error::Error;

use app::Bar;
use iced_exwlshell::{
    build_pattern::daemon,
    settings::{LayerShellSettings, Settings, StartMode},
};
use tracing_subscriber::EnvFilter;

fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(
            "error,vetrata=warn,exwlshellev=warn,iced_exwlshell=warn",
        ))
        .init();

    let config = config::load();
    data::source::font_awesome::load_fonts();

    let mut settings = Settings {
        layer_settings: LayerShellSettings {
            start_mode: StartMode::Background,
            ..Default::default()
        },
        with_connection: message_subscriptions::wayland_connection()
            .map(|connection| connection.clone().into()),
        default_font: config.font.iced_font,
        antialiasing: true,
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
    .theme(Bar::theme)
    .subscription(message_subscriptions::subscription)
    .settings(settings)
    .run()?;
    Ok(())
}
