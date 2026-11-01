mod command_line;
mod desktop_shell;
mod message_subscriptions;
mod shell_message;

use std::{env, error::Error, sync::Arc};

use clap::Parser;
use command_line::CommandLine;
use desktop_shell::DesktopShell;
use iced_exwlshell::{
    build_pattern::daemon,
    settings::{ExWlSettings, LayerShellSettings, StartMode},
};
use sottomano_config::{Config, ConfigFile, SecretPaths};
use sottomano_sources::desktop_windows_apps::niri_kdl_config::NiriConfigFile;
use sottomano_widgets::{
    widget_groups::niri_window_group::niri_kdl_rules::NiriRulesFile, widgets::text::FontFaces,
};
use tracing_subscriber::{EnvFilter, fmt::format::FmtSpan};

const DEFAULT_LOG_DIRECTIVES: &str = "error,sottomano_shell=warn,sottomano_widgets=warn,\
                                      sottomano_sources=warn,exwlshellev=warn,iced_exwlshell=warn";

pub fn run_shell() -> Result<(), Box<dyn Error>> {
    let command_line = CommandLine::parse();
    if let Some(setup_command) = command_line.setup_command {
        return setup_command.print(&sottomano_config::config_home());
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(format!(
            "{DEFAULT_LOG_DIRECTIVES},{}",
            env::var(EnvFilter::DEFAULT_ENV).unwrap_or_default()
        )))
        .with_span_events(FmtSpan::CLOSE)
        .init();

    let config_home = sottomano_config::config_home();
    let config_file = ConfigFile::in_config_home(&config_home);
    let secrets = SecretPaths::discover(&config_home);
    let niri_rules = NiriRulesFile::in_config_home(&config_home);
    let niri_config = NiriConfigFile::in_config_home(&config_home);
    let loaded = tracing::debug_span!("load config").in_scope(|| config_file.load());
    let config = loaded
        .as_ref()
        .map_or_else(|_| Config::default(), Config::clone);
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
    let default_font = FontFaces::from(&config.theme.font).regular;
    let font_data = config.theme.font.data.to_vec();

    daemon(
        move || {
            DesktopShell::new(
                wayland.clone(),
                config.clone(),
                config_file.clone(),
                secrets.clone(),
                niri_rules.clone(),
                niri_config.clone(),
                loaded.clone(),
            )
        },
        DesktopShell::namespace,
        DesktopShell::update,
        DesktopShell::view,
    )
    .style(|_shell: &DesktopShell, theme| DesktopShell::style(theme))
    .theme(DesktopShell::theme)
    .subscription(message_subscriptions::subscription)
    .wl_settings(wl_settings)
    .default_font(default_font)
    .antialiasing(true)
    .font(font_data)
    .run()?;
    Ok(())
}
