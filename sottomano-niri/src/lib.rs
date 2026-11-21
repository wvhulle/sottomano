mod niri_action_requests;
mod niri_event_stream;
mod niri_kdl_config;
mod niri_kdl_rules;
mod niri_output_facts;

use std::{collections::HashMap, future::ready, path::Path};

use futures::{Stream, StreamExt};
use niri_ipc::socket::SOCKET_PATH_ENV;
use sottomano_compositor_model::{
    compositor_backend::{CompositorBackend, CompositorFeatureSupport, logged_query},
    compositor_output_facts::OutputInfo,
    compositor_requests::{CompositorRequest, CompositorRequestFailure},
    compositor_styling_model::{CompositorStyling, StylingControl, SwitchHeat},
    compositor_window_state::CompositorUpdate,
};
use tokio::task::spawn_blocking;

use crate::{
    niri_action_requests::send_niri_request,
    niri_event_stream::{LARGEST_EVENT_BATCH, NiriStreamProgress, niri_events},
    niri_kdl_config::configured_niri_workspace_names,
    niri_kdl_rules::SottomanoRules,
    niri_output_facts::{niri_outputs, niri_version, niri_window_pids},
};

pub struct NiriBackend;

impl CompositorBackend for NiriBackend {
    const DISPLAY_NAME: &'static str = "niri";
    const FEATURE_NAME: &'static str = "niri";
    const SESSION_VARIABLE: &'static str = SOCKET_PATH_ENV;
    const RULES_FILE_NAME: &'static str = "niri.kdl";
    const NOTIFICATION_WINDOW_ID_HINT: Option<&'static str> = Some("x-niri-window-id");

    fn overview_support() -> CompositorFeatureSupport {
        CompositorFeatureSupport::Available
    }

    fn styling_support(_control: StylingControl) -> CompositorFeatureSupport {
        CompositorFeatureSupport::Available
    }

    fn compositor_updates() -> impl Stream<Item = CompositorUpdate> + Send {
        niri_events()
            .ready_chunks(LARGEST_EVENT_BATCH)
            .scan(NiriStreamProgress::default(), |progress, events| {
                ready(Some(progress.applied(events)))
            })
            .filter_map(ready)
    }

    async fn compositor_version() -> Option<String> {
        logged_query(
            "version",
            spawn_blocking(niri_version)
                .await
                .unwrap_or_else(|join_failure| Err(join_failure.into())),
        )
    }

    async fn compositor_outputs() -> Option<HashMap<String, OutputInfo>> {
        logged_query(
            "outputs",
            spawn_blocking(niri_outputs)
                .await
                .unwrap_or_else(|join_failure| Err(join_failure.into())),
        )
    }

    async fn compositor_window_pids() -> Option<Vec<i32>> {
        logged_query(
            "windows",
            spawn_blocking(niri_window_pids)
                .await
                .unwrap_or_else(|join_failure| Err(join_failure.into())),
        )
    }

    async fn send_request(request: CompositorRequest) -> Result<(), CompositorRequestFailure> {
        let request_text = format!("{request:?}");
        spawn_blocking(move || send_niri_request(&request))
            .await
            .unwrap_or_else(|join_error| {
                Err(CompositorRequestFailure::Unreachable {
                    request: request_text,
                    reason: join_error.to_string(),
                })
            })
    }

    fn rendered_rules(
        styling: CompositorStyling,
        switch_heat: &SwitchHeat,
    ) -> Result<String, String> {
        SottomanoRules::styled(styling, switch_heat)
            .to_kdl_v1()
            .map_err(|render_error| render_error.to_string())
    }

    fn configured_workspace_names(
        config_home: &Path,
    ) -> impl Future<Output = Option<Result<Vec<String>, String>>> + Send + use<> {
        let configured_names = configured_niri_workspace_names(config_home);
        async move { Some(configured_names.await) }
    }
}
