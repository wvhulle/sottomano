mod app;
mod config;
mod data;
mod message_subscriptions;
mod state;
mod wayland_layers;
mod widget_groups;
mod widgets;

use std::{error::Error, sync::Arc};

use app::Bar;
use config::{Config, ConfigFile, SecretPaths};
use iced_exwlshell::{
    build_pattern::daemon,
    settings::{LayerShellSettings, Settings, StartMode},
};
use tracing_subscriber::{EnvFilter, fmt::format::FmtSpan};

fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("error,vetrata=warn,exwlshellev=warn,iced_exwlshell=warn")
        }))
        .with_span_events(FmtSpan::CLOSE)
        .init();

    let config_home = config::config_home();
    let config_file = ConfigFile::in_config_home(&config_home);
    let secrets = SecretPaths::discover(&config_home);
    let loaded = tracing::debug_span!("load config").in_scope(|| config_file.load());
    let config = loaded
        .as_ref()
        .map_or_else(|_| Config::fallback(), Config::clone);
    let loaded = loaded.map(Box::new).map_err(Arc::new);

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
        move || {
            Bar::new(
                config.clone(),
                config_file.clone(),
                secrets.clone(),
                loaded.clone(),
            )
        },
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
