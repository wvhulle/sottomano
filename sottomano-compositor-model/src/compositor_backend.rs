use std::{collections::HashMap, fmt::Display, path::Path, process};

use futures::{Stream, StreamExt, stream};

use crate::{
    compositor_output_facts::OutputInfo,
    compositor_requests::{CompositorRequest, CompositorRequestFailure},
    compositor_styling_model::{CompositorStyling, StylingControl, SwitchHeat},
    compositor_window_state::CompositorUpdate,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorFeatureSupport {
    Available,
    Missing,
}

pub trait CompositorBackend {
    const DISPLAY_NAME: &'static str;
    const FEATURE_NAME: &'static str;
    const SESSION_VARIABLE: &'static str;
    const RULES_FILE_NAME: &'static str;
    const NOTIFICATION_WINDOW_ID_HINT: Option<&'static str>;

    fn overview_support() -> CompositorFeatureSupport;

    fn styling_support(control: StylingControl) -> CompositorFeatureSupport;

    fn compositor_updates() -> impl Stream<Item = CompositorUpdate> + Send;

    fn compositor_version() -> impl Future<Output = Option<String>> + Send;

    fn compositor_outputs() -> impl Future<Output = Option<HashMap<String, OutputInfo>>> + Send;

    fn compositor_window_pids() -> impl Future<Output = Option<Vec<i32>>> + Send;

    fn send_request(
        request: CompositorRequest,
    ) -> impl Future<Output = Result<(), CompositorRequestFailure>> + Send;

    fn rendered_rules(
        styling: CompositorStyling,
        switch_heat: &SwitchHeat,
    ) -> Result<String, String>;

    fn configured_workspace_names(
        config_home: &Path,
    ) -> impl Future<Output = Option<Result<Vec<String>, String>>> + Send + use<Self>;
}

pub fn logged_query<Reply, Failure: Display>(
    query: &str,
    reply: Result<Reply, Failure>,
) -> Option<Reply> {
    reply
        .inspect_err(|query_failure| tracing::warn!("compositor {query}: {query_failure}"))
        .ok()
}

fn exit_without_compositor_events(compositor: &str, reason: &dyn Display) -> ! {
    tracing::error!("{compositor}: {reason}; the bar cannot run without compositor events");
    process::exit(1)
}

pub fn followed_until_failure<Failure: Display>(
    compositor: &'static str,
    updates: impl Stream<Item = Result<CompositorUpdate, Failure>> + Send,
) -> impl Stream<Item = CompositorUpdate> + Send {
    updates
        .map(move |update| {
            update.unwrap_or_else(|follow_failure| {
                exit_without_compositor_events(compositor, &follow_failure)
            })
        })
        .chain(stream::poll_fn(move |_| {
            exit_without_compositor_events(compositor, &"the event stream ended")
        }))
}
