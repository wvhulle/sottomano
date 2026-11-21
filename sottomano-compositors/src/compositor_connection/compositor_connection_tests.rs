use super::*;

fn candidates_with(compiled_in_features: &[&str]) -> Vec<CompositorSessionVariable> {
    COMPOSITOR_SESSION_VARIABLES
        .iter()
        .map(|candidate| CompositorSessionVariable {
            build_inclusion: if compiled_in_features.contains(&candidate.feature_name) {
                BuildInclusion::CompiledIn
            } else {
                BuildInclusion::CompiledOut
            },
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
        "no supported compositor session was found: NIRI_SOCKET, HYPRLAND_INSTANCE_SIGNATURE is \
         unset"
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
        assert_eq!(entry.build_inclusion, BuildInclusion::CompiledIn);
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
