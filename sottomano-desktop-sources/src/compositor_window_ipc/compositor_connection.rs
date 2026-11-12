use std::{collections::HashMap, env, fmt::Display, process};

use futures::{Stream, StreamExt, stream};
#[cfg(feature = "niri")]
use tokio::task::spawn_blocking;

use crate::compositor_window_ipc::{
    compositor_output_facts::OutputInfo,
    compositor_styling_model::{ControlSupport, StylingControl},
    compositor_window_state::CompositorUpdate,
};
#[cfg(feature = "hyprland")]
use crate::compositor_window_ipc::{
    hyprland_socket_connection::hyprland_signature_variable,
    hyprland_socket_events::{
        hyprland_compositor_updates, hyprland_outputs, hyprland_version, hyprland_window_pids,
    },
};
#[cfg(feature = "niri")]
use crate::compositor_window_ipc::{
    niri_event_stream::{niri_compositor_updates, niri_socket_variable},
    niri_output_facts::{niri_outputs, niri_version, niri_window_pids},
};
#[cfg(feature = "sway")]
use crate::compositor_window_ipc::{
    sway_ipc_connection::sway_socket_variable,
    sway_ipc_events::{sway_compositor_updates, sway_outputs, sway_version, sway_window_pids},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorKind {
    #[cfg(feature = "niri")]
    Niri,
    #[cfg(feature = "sway")]
    Sway,
    #[cfg(feature = "hyprland")]
    Hyprland,
}

impl CompositorKind {
    #[must_use]
    pub fn detected() -> Option<Self> {
        #[cfg(feature = "niri")]
        if niri_socket_variable().is_some() {
            return Some(CompositorKind::Niri);
        }
        #[cfg(feature = "hyprland")]
        if hyprland_signature_variable().is_some() {
            return Some(CompositorKind::Hyprland);
        }
        #[cfg(feature = "sway")]
        if sway_socket_variable().is_some() {
            return Some(CompositorKind::Sway);
        }
        None
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            #[cfg(feature = "niri")]
            CompositorKind::Niri => "niri",
            #[cfg(feature = "sway")]
            CompositorKind::Sway => "sway",
            #[cfg(feature = "hyprland")]
            CompositorKind::Hyprland => "Hyprland",
        }
    }

    #[must_use]
    pub fn overview_support(self) -> OverviewSupport {
        match self {
            #[cfg(feature = "niri")]
            CompositorKind::Niri => OverviewSupport::Available,
            #[cfg(feature = "sway")]
            CompositorKind::Sway => OverviewSupport::Missing,
            #[cfg(feature = "hyprland")]
            CompositorKind::Hyprland => OverviewSupport::Missing,
        }
    }

    #[must_use]
    pub fn styling_support(self, control: StylingControl) -> ControlSupport {
        match (self, control) {
            #[cfg(feature = "sway")]
            (
                CompositorKind::Sway,
                StylingControl::WindowCornerRadius
                | StylingControl::ShellBlurDepth
                | StylingControl::SwitchHeatBorders,
            ) => ControlSupport::Missing,
            _ => ControlSupport::Available,
        }
    }

    #[must_use]
    pub fn rules_file_name(self) -> &'static str {
        match self {
            #[cfg(feature = "niri")]
            CompositorKind::Niri => "niri.kdl",
            #[cfg(feature = "sway")]
            CompositorKind::Sway => "sway.conf",
            #[cfg(feature = "hyprland")]
            CompositorKind::Hyprland => "hyprland.lua",
        }
    }

    #[must_use]
    pub fn workspace_config_source(self) -> WorkspaceConfigSource {
        match self {
            #[cfg(feature = "niri")]
            CompositorKind::Niri => WorkspaceConfigSource::NiriKdl,
            #[cfg(feature = "sway")]
            CompositorKind::Sway => WorkspaceConfigSource::Unread,
            #[cfg(feature = "hyprland")]
            CompositorKind::Hyprland => WorkspaceConfigSource::Unread,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompositorSessionVariable {
    display_name: &'static str,
    feature_name: &'static str,
    session_variable: &'static str,
    compiled_in: bool,
}

const COMPOSITOR_SESSION_VARIABLES: [CompositorSessionVariable; 3] = [
    CompositorSessionVariable {
        display_name: "niri",
        feature_name: "niri",
        session_variable: "NIRI_SOCKET",
        compiled_in: cfg!(feature = "niri"),
    },
    CompositorSessionVariable {
        display_name: "sway",
        feature_name: "sway",
        session_variable: "SWAYSOCK",
        compiled_in: cfg!(feature = "sway"),
    },
    CompositorSessionVariable {
        display_name: "Hyprland",
        feature_name: "hyprland",
        session_variable: "HYPRLAND_INSTANCE_SIGNATURE",
        compiled_in: cfg!(feature = "hyprland"),
    },
];

#[must_use]
pub fn missing_compositor_reason() -> String {
    missing_compositor_reason_among(&COMPOSITOR_SESSION_VARIABLES, |session_variable| {
        env::var_os(session_variable).is_some()
    })
}

fn missing_compositor_reason_among(
    candidates: &[CompositorSessionVariable],
    session_variable_is_set: impl Fn(&str) -> bool,
) -> String {
    let compiled_in: Vec<&CompositorSessionVariable> = candidates
        .iter()
        .filter(|candidate| candidate.compiled_in)
        .collect();
    let running_but_compiled_out = candidates.iter().find(|candidate| {
        !candidate.compiled_in && session_variable_is_set(candidate.session_variable)
    });
    if let Some(compiled_out) = running_but_compiled_out {
        let included_names: Vec<&str> = compiled_in
            .iter()
            .map(|candidate| candidate.display_name)
            .collect();
        return format!(
            "a {} session is running ({} is set), but this build only includes {}; add \"{}\" to \
             services.sottomano.compositors or build with the {} cargo feature",
            compiled_out.display_name,
            compiled_out.session_variable,
            included_names.join(", "),
            compiled_out.feature_name,
            compiled_out.feature_name,
        );
    }
    let unset_variables: Vec<&str> = compiled_in
        .iter()
        .map(|candidate| candidate.session_variable)
        .collect();
    format!(
        "no supported compositor session was found: {} is unset",
        unset_variables.join(", ")
    )
}

fn exit_without_compositor_events(compositor: &str, reason: &dyn Display) -> ! {
    tracing::error!("{compositor}: {reason}; the bar cannot run without compositor events");
    process::exit(1)
}

#[cfg_attr(not(any(feature = "sway", feature = "hyprland")), allow(dead_code))]
fn followed_until_failure<Failure: Display>(
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

pub fn compositor_updates() -> impl Stream<Item = CompositorUpdate> + Send {
    match CompositorKind::detected() {
        #[cfg(feature = "niri")]
        Some(CompositorKind::Niri) => niri_compositor_updates().boxed(),
        #[cfg(feature = "sway")]
        Some(CompositorKind::Sway) => {
            followed_until_failure("sway", sway_compositor_updates()).boxed()
        }
        #[cfg(feature = "hyprland")]
        Some(CompositorKind::Hyprland) => {
            followed_until_failure("Hyprland", hyprland_compositor_updates()).boxed()
        }
        None => {
            tracing::error!("{}", missing_compositor_reason());
            stream::empty().boxed()
        }
    }
}

#[cfg_attr(
    not(any(feature = "niri", feature = "sway", feature = "hyprland")),
    allow(dead_code)
)]
fn logged_query<Reply, Failure: Display>(
    query: &str,
    reply: Result<Reply, Failure>,
) -> Option<Reply> {
    reply
        .inspect_err(|query_failure| tracing::warn!("compositor {query}: {query_failure}"))
        .ok()
}

pub async fn compositor_version() -> Option<String> {
    match CompositorKind::detected()? {
        #[cfg(feature = "niri")]
        CompositorKind::Niri => logged_query(
            "version",
            spawn_blocking(niri_version)
                .await
                .unwrap_or_else(|join_failure| Err(join_failure.into())),
        ),
        #[cfg(feature = "sway")]
        CompositorKind::Sway => logged_query("version", sway_version().await),
        #[cfg(feature = "hyprland")]
        CompositorKind::Hyprland => logged_query("version", hyprland_version().await).flatten(),
    }
}

pub async fn compositor_outputs() -> HashMap<String, OutputInfo> {
    let outputs = match CompositorKind::detected() {
        #[cfg(feature = "niri")]
        Some(CompositorKind::Niri) => logged_query(
            "outputs",
            spawn_blocking(niri_outputs)
                .await
                .unwrap_or_else(|join_failure| Err(join_failure.into())),
        ),
        #[cfg(feature = "sway")]
        Some(CompositorKind::Sway) => logged_query("outputs", sway_outputs().await),
        #[cfg(feature = "hyprland")]
        Some(CompositorKind::Hyprland) => logged_query("outputs", hyprland_outputs().await),
        None => None,
    };
    outputs.unwrap_or_default()
}

pub async fn compositor_window_pids() -> Option<Vec<i32>> {
    match CompositorKind::detected()? {
        #[cfg(feature = "niri")]
        CompositorKind::Niri => logged_query(
            "windows",
            spawn_blocking(niri_window_pids)
                .await
                .unwrap_or_else(|join_failure| Err(join_failure.into())),
        ),
        #[cfg(feature = "sway")]
        CompositorKind::Sway => logged_query("windows", sway_window_pids().await),
        #[cfg(feature = "hyprland")]
        CompositorKind::Hyprland => logged_query("windows", hyprland_window_pids().await),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewSupport {
    Available,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceConfigSource {
    NiriKdl,
    Unread,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates_with(compiled_in_features: &[&str]) -> Vec<CompositorSessionVariable> {
        COMPOSITOR_SESSION_VARIABLES
            .iter()
            .map(|candidate| CompositorSessionVariable {
                compiled_in: compiled_in_features.contains(&candidate.feature_name),
                ..*candidate
            })
            .collect()
    }

    #[test]
    fn a_compiled_out_session_names_the_missing_compositor() {
        let reason = missing_compositor_reason_among(&candidates_with(&["niri"]), |variable| {
            variable == "SWAYSOCK"
        });
        assert_eq!(
            reason,
            "a sway session is running (SWAYSOCK is set), but this build only includes niri; add \
             \"sway\" to services.sottomano.compositors or build with the sway cargo feature"
        );
    }

    #[test]
    fn without_any_session_the_compiled_in_variables_are_listed() {
        let reason =
            missing_compositor_reason_among(&candidates_with(&["niri", "hyprland"]), |_| false);
        assert_eq!(
            reason,
            "no supported compositor session was found: NIRI_SOCKET, HYPRLAND_INSTANCE_SIGNATURE \
             is unset"
        );
    }
}
