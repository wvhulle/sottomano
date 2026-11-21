mod command_line;
mod compact_tokio_executor;
mod compositor_updates;
mod config_adoption;
#[cfg(feature = "allocation-tally")]
mod counting_allocator;
mod desktop_shell;
mod hover_card_updates;
mod hover_content_routing;
mod indicator_groups;
mod menus;
mod message_dispatch;
mod message_subscriptions;
mod monitors;
mod notification_updates;
mod panels;
mod pointer_drag;
mod process_load_report;
mod shell_interactions;
mod shell_message;
mod sqlite_app_launches;

use std::{
    env::{self, VarError},
    error::Error,
    io,
    sync::Arc,
};

use clap::Parser;
use command_line::CommandLine;
use desktop_shell::{DesktopShell, WaylandLink};
use iced_exwlshell::{
    build_pattern::daemon,
    settings::{ExWlSettings, LayerShellSettings, StartMode},
};
use sottomano_compositors::compositor_connection::CompositorKind;
use sottomano_desktop_sinks::compositor_rule_files::CompositorRulesFile;
use sottomano_toml_config::{Config, ConfigFile, SecretPaths};
use sottomano_widgets::widgets::text::resolved_font_faces;
use tracing_chrome::{ChromeLayerBuilder, FlushGuard};
use tracing_subscriber::{
    EnvFilter, Layer, fmt, fmt::format::FmtSpan, layer::SubscriberExt, registry::LookupSpan,
    util::SubscriberInitExt,
};

const DEFAULT_LOG_DIRECTIVES: &str = "error,sottomano_shell=warn,sottomano_widgets=warn,\
                                      sottomano_desktop_sources=warn,exwlshellev=warn,\
                                      iced_exwlshell=warn";

const TRACE_FILE_VARIABLE: &str = "SOTTOMANO_TRACE_FILE";

const TRACE_FILTER_VARIABLE: &str = "SOTTOMANO_TRACE_FILTER";

const DEFAULT_TRACE_DIRECTIVES: &str =
    "error,sottomano_shell=debug,sottomano_widgets=debug,sottomano_desktop_sources=debug";

fn log_output_layer<Subscriber>() -> impl Layer<Subscriber>
where
    Subscriber: tracing::Subscriber + for<'span> LookupSpan<'span>,
{
    let environment_directives = match env::var(EnvFilter::DEFAULT_ENV) {
        Ok(directives) => directives,
        Err(VarError::NotPresent) => String::new(),
        Err(VarError::NotUnicode(raw_directives)) => panic!(
            "{} holds non-unicode log directives: {}",
            EnvFilter::DEFAULT_ENV,
            raw_directives.display()
        ),
    };
    fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_filter(EnvFilter::new(format!(
            "{DEFAULT_LOG_DIRECTIVES},{environment_directives}"
        )))
}

fn install_tracing_subscriber() -> io::Result<()> {
    let (chrome_layer, trace_flush_guard) = env::var_os(TRACE_FILE_VARIABLE)
        .map(|trace_file_path| {
            ChromeLayerBuilder::new()
                .file(trace_file_path)
                .include_args(true)
                .build()
        })
        .unzip();
    let trace_directives =
        env::var(TRACE_FILTER_VARIABLE).unwrap_or_else(|_| DEFAULT_TRACE_DIRECTIVES.to_string());
    tracing_subscriber::registry()
        .with(log_output_layer())
        .with(chrome_layer.map(|layer| layer.with_filter(EnvFilter::new(trace_directives))))
        .init();
    process_load_report::spawn_load_reports(move || {
        trace_flush_guard.iter().for_each(FlushGuard::flush);
    })
}

pub fn run_shell() -> Result<(), Box<dyn Error>> {
    let command_line = CommandLine::parse();
    if let Some(setup_command) = command_line.setup_command {
        return setup_command.print(&sottomano_toml_config::config_home());
    }

    install_tracing_subscriber()?;

    let config_home = sottomano_toml_config::config_home();
    let config_file = ConfigFile::in_config_home(&config_home);
    let secrets = SecretPaths::discover(&config_home);
    let compositor_rules =
        CompositorRulesFile::in_config_home(&config_home, CompositorKind::detected());
    let loaded = tracing::debug_span!("load config").in_scope(|| config_file.load());
    let config = loaded
        .as_ref()
        .map_or_else(|_| Config::default(), Config::clone);
    let loaded = loaded.map(Box::new).map_err(Arc::new);

    let wayland = match wayland_client::Connection::connect_to_env() {
        Ok(connection) => WaylandLink::Connected(connection),
        Err(connection_error) => {
            tracing::error!(
                "wayland: no connection, outputs will not be tracked: {connection_error}"
            );
            WaylandLink::Unconnected
        }
    };
    let wl_settings = ExWlSettings {
        layer_settings: LayerShellSettings {
            start_mode: StartMode::Background,
            ..Default::default()
        },
        with_connection: match &wayland {
            WaylandLink::Connected(connection) => Some(connection.clone().into()),
            WaylandLink::Unconnected => None,
        },
        ..Default::default()
    };
    let default_font = resolved_font_faces(&config.theme.font).regular;
    let font_data = config.theme.font.data.to_vec();

    daemon(
        move || {
            DesktopShell::new(
                wayland.clone(),
                config.clone(),
                config_file.clone(),
                secrets.clone(),
                compositor_rules.clone(),
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
    .executor::<compact_tokio_executor::CompactTokioExecutor>()
    .wl_settings(wl_settings)
    .default_font(default_font)
    .antialiasing(true)
    .font(font_data)
    .run()?;
    Ok(())
}
