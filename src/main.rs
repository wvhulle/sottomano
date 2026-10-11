mod command_line;
mod config;
mod data;
mod desktop_shell;
mod message_subscriptions;
mod state;
mod widget_groups;
mod widgets;

use std::{error::Error, sync::Arc};

use clap::Parser;
use command_line::CommandLine;
use config::{Config, ConfigFile, SecretPaths};
use desktop_shell::DesktopShell;
use iced_exwlshell::{
    build_pattern::daemon,
    redraw::Scope,
    settings::{ExWlSettings, LayerShellSettings, StartMode},
};
use state::niri_rules::NiriRulesFile;
use tracing_subscriber::{EnvFilter, fmt::format::FmtSpan};

fn main() -> Result<(), Box<dyn Error>> {
    let command_line = CommandLine::parse();
    if command_line.describes_setup() {
        return command_line.print_setup(&config::config_home());
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("error,vetrata=warn,exwlshellev=warn,iced_exwlshell=warn")
        }))
        .with_span_events(FmtSpan::CLOSE)
        .init();

    let config_home = config::config_home();
    let config_file = ConfigFile::in_config_home(&config_home);
    let secrets = SecretPaths::discover(&config_home);
    let niri_rules = NiriRulesFile::in_config_home(&config_home);
    let loaded = tracing::debug_span!("load config").in_scope(|| config_file.load());
    let config = loaded
        .as_ref()
        .map_or_else(|_| Config::fallback(), Config::clone);
    let loaded = loaded.map(Box::new).map_err(Arc::new);

    let wayland = wayland_client::Connection::connect_to_env()
        .inspect_err(|e| {
            tracing::error!("wayland: no connection, outputs will not be tracked: {e}");
        })
        .ok();
    let wl_settings = ExWlSettings {
        layer_settings: LayerShellSettings {
            start_mode: StartMode::Background,
            ..Default::default()
        },
        with_connection: wayland.clone().map(Into::into),
        ..Default::default()
    };
    let default_font = config.theme.font.iced_font;
    let font_data = config.theme.font.data.to_vec();

    daemon(
        move || {
            DesktopShell::new(
                wayland.clone(),
                config.clone(),
                config_file.clone(),
                secrets.clone(),
                niri_rules.clone(),
                loaded.clone(),
            )
        },
        DesktopShell::namespace,
        DesktopShell::update,
        DesktopShell::view,
    )
    .style(DesktopShell::style)
    .theme(DesktopShell::theme)
    .subscription(message_subscriptions::subscription)
    .redraw_scope(|_| Scope::None)
    .wl_settings(wl_settings)
    .default_font(default_font)
    .antialiasing(true)
    .font(font_data)
    .run()?;
    Ok(())
}
