use std::{collections::HashMap, env, path::Path};

use futures::{Stream, StreamExt, stream};
use sottomano_compositor_model::{
    compositor_backend::{CompositorBackend, CompositorFeatureSupport},
    compositor_output_facts::OutputInfo,
    compositor_requests::{CompositorRequest, CompositorRequestFailure},
    compositor_styling_model::{
        CompositorStyling, EdgeShadowRendering, StylingControl, SwitchHeat,
    },
    compositor_window_state::CompositorUpdate,
};
#[cfg(feature = "hyprland")]
use sottomano_hyprland::HyprlandBackend;
#[cfg(feature = "niri")]
use sottomano_niri::NiriBackend;
#[cfg(feature = "sway")]
use sottomano_sway::SwayBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompiledBackend {
    #[cfg(feature = "niri")]
    Niri,
    #[cfg(feature = "sway")]
    Sway,
    #[cfg(feature = "hyprland")]
    Hyprland,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompositorKind {
    backend: CompiledBackend,
}

impl CompositorKind {
    pub const COMPILED_IN: &[Self] = &[
        #[cfg(feature = "niri")]
        Self {
            backend: CompiledBackend::Niri,
        },
        #[cfg(feature = "hyprland")]
        Self {
            backend: CompiledBackend::Hyprland,
        },
        #[cfg(feature = "sway")]
        Self {
            backend: CompiledBackend::Sway,
        },
    ];

    #[must_use]
    pub fn detected() -> Option<Self> {
        Self::COMPILED_IN
            .iter()
            .copied()
            .find(|compositor| env::var_os(compositor.session_variable()).is_some())
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self.backend {
            #[cfg(feature = "niri")]
            CompiledBackend::Niri => NiriBackend::DISPLAY_NAME,
            #[cfg(feature = "sway")]
            CompiledBackend::Sway => SwayBackend::DISPLAY_NAME,
            #[cfg(feature = "hyprland")]
            CompiledBackend::Hyprland => HyprlandBackend::DISPLAY_NAME,
        }
    }

    fn session_variable(self) -> &'static str {
        match self.backend {
            #[cfg(feature = "niri")]
            CompiledBackend::Niri => NiriBackend::SESSION_VARIABLE,
            #[cfg(feature = "sway")]
            CompiledBackend::Sway => SwayBackend::SESSION_VARIABLE,
            #[cfg(feature = "hyprland")]
            CompiledBackend::Hyprland => HyprlandBackend::SESSION_VARIABLE,
        }
    }

    #[must_use]
    pub fn rules_file_name(self) -> &'static str {
        match self.backend {
            #[cfg(feature = "niri")]
            CompiledBackend::Niri => NiriBackend::RULES_FILE_NAME,
            #[cfg(feature = "sway")]
            CompiledBackend::Sway => SwayBackend::RULES_FILE_NAME,
            #[cfg(feature = "hyprland")]
            CompiledBackend::Hyprland => HyprlandBackend::RULES_FILE_NAME,
        }
    }

    #[must_use]
    pub fn notification_window_id(self, hint_text: &str) -> Option<u64> {
        match self.backend {
            #[cfg(feature = "niri")]
            CompiledBackend::Niri => NiriBackend::notification_window_id(hint_text),
            #[cfg(feature = "sway")]
            CompiledBackend::Sway => SwayBackend::notification_window_id(hint_text),
            #[cfg(feature = "hyprland")]
            CompiledBackend::Hyprland => HyprlandBackend::notification_window_id(hint_text),
        }
    }

    #[must_use]
    pub fn edge_shadow_rendering(self) -> EdgeShadowRendering {
        match self.backend {
            #[cfg(feature = "niri")]
            CompiledBackend::Niri => NiriBackend::EDGE_SHADOW_RENDERING,
            #[cfg(feature = "sway")]
            CompiledBackend::Sway => SwayBackend::EDGE_SHADOW_RENDERING,
            #[cfg(feature = "hyprland")]
            CompiledBackend::Hyprland => HyprlandBackend::EDGE_SHADOW_RENDERING,
        }
    }

    #[must_use]
    pub fn overview_support(self) -> CompositorFeatureSupport {
        match self.backend {
            #[cfg(feature = "niri")]
            CompiledBackend::Niri => NiriBackend::overview_support(),
            #[cfg(feature = "sway")]
            CompiledBackend::Sway => SwayBackend::overview_support(),
            #[cfg(feature = "hyprland")]
            CompiledBackend::Hyprland => HyprlandBackend::overview_support(),
        }
    }

    #[must_use]
    pub fn styling_support(self, control: StylingControl) -> CompositorFeatureSupport {
        match self.backend {
            #[cfg(feature = "niri")]
            CompiledBackend::Niri => NiriBackend::styling_support(control),
            #[cfg(feature = "sway")]
            CompiledBackend::Sway => SwayBackend::styling_support(control),
            #[cfg(feature = "hyprland")]
            CompiledBackend::Hyprland => HyprlandBackend::styling_support(control),
        }
    }

    pub fn rendered_rules(
        self,
        styling: CompositorStyling,
        switch_heat: &SwitchHeat,
    ) -> Result<String, String> {
        match self.backend {
            #[cfg(feature = "niri")]
            CompiledBackend::Niri => NiriBackend::rendered_rules(styling, switch_heat),
            #[cfg(feature = "sway")]
            CompiledBackend::Sway => SwayBackend::rendered_rules(styling, switch_heat),
            #[cfg(feature = "hyprland")]
            CompiledBackend::Hyprland => HyprlandBackend::rendered_rules(styling, switch_heat),
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

pub fn compositor_updates() -> impl Stream<Item = CompositorUpdate> + Send {
    let Some(compositor) = CompositorKind::detected() else {
        tracing::error!("{}", missing_compositor_reason());
        return stream::empty().boxed();
    };
    match compositor.backend {
        #[cfg(feature = "niri")]
        CompiledBackend::Niri => NiriBackend::compositor_updates().boxed(),
        #[cfg(feature = "sway")]
        CompiledBackend::Sway => SwayBackend::compositor_updates().boxed(),
        #[cfg(feature = "hyprland")]
        CompiledBackend::Hyprland => HyprlandBackend::compositor_updates().boxed(),
    }
}

pub async fn compositor_version() -> Option<String> {
    match CompositorKind::detected()?.backend {
        #[cfg(feature = "niri")]
        CompiledBackend::Niri => NiriBackend::compositor_version().await,
        #[cfg(feature = "sway")]
        CompiledBackend::Sway => SwayBackend::compositor_version().await,
        #[cfg(feature = "hyprland")]
        CompiledBackend::Hyprland => HyprlandBackend::compositor_version().await,
    }
}

pub async fn compositor_outputs() -> HashMap<String, OutputInfo> {
    let Some(compositor) = CompositorKind::detected() else {
        return HashMap::new();
    };
    let outputs = match compositor.backend {
        #[cfg(feature = "niri")]
        CompiledBackend::Niri => NiriBackend::compositor_outputs().await,
        #[cfg(feature = "sway")]
        CompiledBackend::Sway => SwayBackend::compositor_outputs().await,
        #[cfg(feature = "hyprland")]
        CompiledBackend::Hyprland => HyprlandBackend::compositor_outputs().await,
    };
    outputs.unwrap_or_default()
}

pub async fn compositor_window_pids() -> Option<Vec<i32>> {
    match CompositorKind::detected()?.backend {
        #[cfg(feature = "niri")]
        CompiledBackend::Niri => NiriBackend::compositor_window_pids().await,
        #[cfg(feature = "sway")]
        CompiledBackend::Sway => SwayBackend::compositor_window_pids().await,
        #[cfg(feature = "hyprland")]
        CompiledBackend::Hyprland => HyprlandBackend::compositor_window_pids().await,
    }
}

pub async fn send_compositor_request(
    request: CompositorRequest,
) -> Result<(), CompositorRequestFailure> {
    let Some(compositor) = CompositorKind::detected() else {
        return Err(CompositorRequestFailure::Unreachable {
            request: format!("{request:?}"),
            reason: missing_compositor_reason(),
        });
    };
    match compositor.backend {
        #[cfg(feature = "niri")]
        CompiledBackend::Niri => NiriBackend::send_request(request).await,
        #[cfg(feature = "sway")]
        CompiledBackend::Sway => SwayBackend::send_request(request).await,
        #[cfg(feature = "hyprland")]
        CompiledBackend::Hyprland => HyprlandBackend::send_request(request).await,
    }
}

pub async fn configured_workspace_names(config_home: &Path) -> Option<Result<Vec<String>, String>> {
    let compositor = CompositorKind::detected()?;
    match compositor.backend {
        #[cfg(feature = "niri")]
        CompiledBackend::Niri => NiriBackend::configured_workspace_names(config_home).await,
        #[cfg(feature = "sway")]
        CompiledBackend::Sway => SwayBackend::configured_workspace_names(config_home).await,
        #[cfg(feature = "hyprland")]
        CompiledBackend::Hyprland => HyprlandBackend::configured_workspace_names(config_home).await,
    }
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

    #[test]
    fn every_compiled_in_backend_matches_its_session_variable_entry() {
        for compositor in CompositorKind::COMPILED_IN {
            let entry = COMPOSITOR_SESSION_VARIABLES
                .iter()
                .find(|candidate| candidate.display_name == compositor.name())
                .expect("every backend has a session variable entry");
            assert_eq!(entry.session_variable, compositor.session_variable());
            assert!(entry.compiled_in);
        }
    }

    fn default_styling_rules(compositor: CompositorKind) -> String {
        compositor
            .rendered_rules(CompositorStyling::default(), &SwitchHeat::Hidden)
            .expect("default styling renders")
    }

    #[cfg(feature = "niri")]
    #[test]
    fn niri_alone_draws_the_edge_shadow_through_its_rules() {
        assert_eq!(
            CompositorKind {
                backend: CompiledBackend::Niri,
            }
            .edge_shadow_rendering(),
            EdgeShadowRendering::CompositorShadow
        );
        let rules = default_styling_rules(CompositorKind {
            backend: CompiledBackend::Niri,
        });
        assert!(rules.contains("namespace=\"^sottomano-backdrop-top$\""));
        assert!(rules.contains("softness 27"));
        assert!(rules.contains("spread 25"));
        assert!(rules.contains("color \"#00000060\""));
        assert!(rules.contains("offset x=0 y=-3"));
        assert!(rules.contains("offset x=0 y=3"));
    }

    #[cfg(feature = "hyprland")]
    #[test]
    fn hyprland_leaves_the_edge_shadow_to_sottomano() {
        assert_eq!(
            CompositorKind {
                backend: CompiledBackend::Hyprland,
            }
            .edge_shadow_rendering(),
            EdgeShadowRendering::SelfDrawnFalloff
        );
        assert!(
            !default_styling_rules(CompositorKind {
                backend: CompiledBackend::Hyprland,
            })
            .contains("sottomano-backdrop")
        );
    }

    #[cfg(feature = "sway")]
    #[test]
    fn sway_leaves_the_edge_shadow_to_sottomano() {
        assert_eq!(
            CompositorKind {
                backend: CompiledBackend::Sway,
            }
            .edge_shadow_rendering(),
            EdgeShadowRendering::SelfDrawnFalloff
        );
        assert!(
            !default_styling_rules(CompositorKind {
                backend: CompiledBackend::Sway,
            })
            .contains("sottomano-backdrop")
        );
    }
}
