mod hyprland_dispatch_requests;
mod hyprland_lua_rules;
mod hyprland_socket_connection;
mod hyprland_socket_events;
mod hyprland_workspace_identity;

use std::{collections::HashMap, path::Path};

use futures::Stream;
use sottomano_compositor_model::{
    compositor_backend::{
        CompositorBackend, CompositorFeatureSupport, followed_until_failure, logged_query,
    },
    compositor_output_facts::OutputInfo,
    compositor_requests::{CompositorRequest, CompositorRequestFailure},
    compositor_styling_model::{CompositorStyling, StyledSurface, StylingControl, SwitchHeat},
    compositor_window_state::CompositorUpdate,
    systemd_run_launches::launch_in_user_service,
};

use crate::{
    hyprland_dispatch_requests::{
        HyprlandDispatch, hyprland_command_accepted, referenced_workspace_key,
    },
    hyprland_lua_rules::{general_section, heated_window_rule, hyprland_layer_rule},
    hyprland_socket_connection::HYPRLAND_SIGNATURE_ENV,
    hyprland_socket_events::{
        hyprland_compositor_updates, hyprland_outputs, hyprland_version, hyprland_window_pids,
    },
    hyprland_workspace_identity::hyprland_workspace_with_key,
};

pub struct HyprlandBackend;

impl CompositorBackend for HyprlandBackend {
    const DISPLAY_NAME: &'static str = "Hyprland";
    const FEATURE_NAME: &'static str = "hyprland";
    const SESSION_VARIABLE: &'static str = HYPRLAND_SIGNATURE_ENV;
    const RULES_FILE_NAME: &'static str = "hyprland.lua";
    const NOTIFICATION_WINDOW_ID_HINT: Option<&'static str> = None;

    fn overview_support() -> CompositorFeatureSupport {
        CompositorFeatureSupport::Available
    }

    fn styling_support(_control: StylingControl) -> CompositorFeatureSupport {
        CompositorFeatureSupport::Available
    }

    fn compositor_updates() -> impl Stream<Item = CompositorUpdate> + Send {
        followed_until_failure(Self::DISPLAY_NAME, hyprland_compositor_updates())
    }

    async fn compositor_version() -> Option<String> {
        logged_query("version", hyprland_version().await).flatten()
    }

    async fn compositor_outputs() -> Option<HashMap<String, OutputInfo>> {
        logged_query("outputs", hyprland_outputs().await)
    }

    async fn compositor_window_pids() -> Option<Vec<i32>> {
        logged_query("windows", hyprland_window_pids().await)
    }

    async fn send_request(request: CompositorRequest) -> Result<(), CompositorRequestFailure> {
        let listed_workspace = match referenced_workspace_key(&request) {
            Some(workspace_key) => {
                hyprland_workspace_with_key(workspace_key)
                    .await
                    .map_err(|request_error| CompositorRequestFailure::Unreachable {
                        request: format!("{request:?}"),
                        reason: request_error.to_string(),
                    })?
            }
            None => None,
        };
        match HyprlandDispatch::new(&request, |_| listed_workspace.clone()) {
            HyprlandDispatch::Launch(command) => launch_in_user_service(&command).await,
            HyprlandDispatch::WorkspaceGone => Err(CompositorRequestFailure::WorkspaceGone {
                request: format!("{request:?}"),
            }),
            HyprlandDispatch::Unsupported => Err(CompositorRequestFailure::Unsupported {
                request: format!("{request:?}"),
                compositor: "Hyprland",
            }),
            HyprlandDispatch::Dispatcher(dispatcher) => {
                hyprland_command_accepted(format!("dispatch {dispatcher}")).await
            }
            HyprlandDispatch::Evaluation(lua_code) => {
                hyprland_command_accepted(format!("eval {lua_code}")).await
            }
            HyprlandDispatch::Command(command) => {
                hyprland_command_accepted(command.to_owned()).await
            }
        }
    }

    fn rendered_rules(
        styling: CompositorStyling,
        switch_heat: &SwitchHeat,
    ) -> Result<String, String> {
        let config = format!(
            "hl.config({{\n{}    decoration = {{\n        rounding = {},\n    }},\n}})\n",
            general_section(styling, switch_heat),
            styling.layout.window_corner_radius_px.round()
        );
        let layer_rules = StyledSurface::ALL
            .into_iter()
            .filter_map(|surface| hyprland_layer_rule(surface, styling));
        let window_rules = switch_heat
            .heated_windows()
            .iter()
            .enumerate()
            .map(|(heat_index, heated)| heated_window_rule(heat_index, heated));
        Ok([config]
            .into_iter()
            .chain(layer_rules)
            .chain(window_rules)
            .collect::<Vec<_>>()
            .join("\n"))
    }

    fn configured_workspace_names(
        _config_home: &Path,
    ) -> impl Future<Output = Option<Result<Vec<String>, String>>> + Send + use<> {
        async { None }
    }
}
