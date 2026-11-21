mod sway_command_requests;
mod sway_ipc_connection;
mod sway_ipc_events;

use std::{collections::HashMap, path::Path};

use futures::Stream;
use sottomano_compositor_model::{
    compositor_backend::{
        CompositorBackend, CompositorFeatureSupport, followed_until_failure, logged_query,
    },
    compositor_output_facts::OutputInfo,
    compositor_requests::{CompositorRequest, CompositorRequestFailure},
    compositor_styling_model::{CompositorStyling, StylingControl, SwitchHeat},
    compositor_window_state::CompositorUpdate,
    systemd_run_launches::launch_in_user_service,
};
use swayipc_types::{CommandOutcome, CommandType};

use crate::{
    sway_command_requests::{SwayCommand, referenced_workspace, sway_workspace_name},
    sway_ipc_connection::{SWAY_SOCKET_ENV, SwayIpcConnection},
    sway_ipc_events::{sway_compositor_updates, sway_outputs, sway_version, sway_window_pids},
};

pub struct SwayBackend;

impl CompositorBackend for SwayBackend {
    const DISPLAY_NAME: &'static str = "sway";
    const FEATURE_NAME: &'static str = "sway";
    const SESSION_VARIABLE: &'static str = SWAY_SOCKET_ENV;
    const RULES_FILE_NAME: &'static str = "sway.conf";
    const NOTIFICATION_WINDOW_ID_HINT: Option<&'static str> = None;

    fn overview_support() -> CompositorFeatureSupport {
        CompositorFeatureSupport::Missing
    }

    fn styling_support(control: StylingControl) -> CompositorFeatureSupport {
        match control {
            StylingControl::WindowCornerRadius
            | StylingControl::ShellBlurDepth
            | StylingControl::SwitchHeatBorders => CompositorFeatureSupport::Missing,
            StylingControl::WindowGaps => CompositorFeatureSupport::Available,
        }
    }

    fn compositor_updates() -> impl Stream<Item = CompositorUpdate> + Send {
        followed_until_failure(Self::DISPLAY_NAME, sway_compositor_updates())
    }

    async fn compositor_version() -> Option<String> {
        logged_query("version", sway_version().await)
    }

    async fn compositor_outputs() -> Option<HashMap<String, OutputInfo>> {
        logged_query("outputs", sway_outputs().await)
    }

    async fn compositor_window_pids() -> Option<Vec<i32>> {
        logged_query("windows", sway_window_pids().await)
    }

    async fn send_request(request: CompositorRequest) -> Result<(), CompositorRequestFailure> {
        let unreachable = |reason: String| CompositorRequestFailure::Unreachable {
            request: format!("{request:?}"),
            reason,
        };
        let mut connection = SwayIpcConnection::connect()
            .await
            .map_err(|connect_error| unreachable(connect_error.to_string()))?;
        let workspace_name = match referenced_workspace(&request) {
            Some(workspace) => sway_workspace_name(&mut connection, workspace)
                .await
                .map_err(|request_error| unreachable(request_error.to_string()))?,
            None => None,
        };
        match SwayCommand::new(&request, |_| workspace_name.clone()) {
            SwayCommand::Launch(command) => launch_in_user_service(&command).await,
            SwayCommand::Unsupported => Err(CompositorRequestFailure::Unsupported {
                request: format!("{request:?}"),
                compositor: "sway",
            }),
            SwayCommand::Text(command) => {
                let outcomes: Vec<CommandOutcome> = connection
                    .request(CommandType::RunCommand, &command)
                    .await
                    .map_err(|request_error| unreachable(request_error.to_string()))?;
                outcomes
                    .into_iter()
                    .try_for_each(CommandOutcome::decode)
                    .map_err(|refusal| CompositorRequestFailure::Rejected {
                        request: command,
                        reason: refusal.to_string(),
                    })
            }
        }
    }

    fn rendered_rules(
        styling: CompositorStyling,
        _switch_heat: &SwitchHeat,
    ) -> Result<String, String> {
        Ok(format!("gaps inner {}\n", styling.layout.gaps_px.round()))
    }

    fn configured_workspace_names(
        _config_home: &Path,
    ) -> impl Future<Output = Option<Result<Vec<String>, String>>> + Send + use<> {
        async { None }
    }
}

#[cfg(test)]
mod tests {
    use sottomano_compositor_model::{
        compositor_backend::CompositorBackend,
        compositor_styling_model::{AppWindowLayout, CompositorStyling, SwitchHeat},
    };

    use crate::SwayBackend;

    #[test]
    fn gaps_are_rounded_to_whole_pixels() {
        let styling = CompositorStyling {
            layout: AppWindowLayout {
                gaps_px: 7.6,
                ..AppWindowLayout::default()
            },
            ..CompositorStyling::default()
        };
        assert_eq!(
            SwayBackend::rendered_rules(styling, &SwitchHeat::Hidden),
            Ok("gaps inner 8\n".to_owned())
        );
    }
}
