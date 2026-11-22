use std::collections::BTreeMap;

use iced::window::Id;
use iced_exwlshell::reexport::Layer;
use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::hover_cards::hover_card_surfaces::HoverCardPresence;

use super::{
    OutputSurface, Surface, SurfaceRole, ToastSurfaceChanges, restack_around_hover_card,
    toast_surface_changes,
};

fn hover_card_on_first_output(output: &str) -> HoverCardPresence {
    if output == "DP-1" {
        HoverCardPresence::OnThisOutput
    } else {
        HoverCardPresence::Elsewhere
    }
}

fn hover_card_closed(_output: &str) -> HoverCardPresence {
    HoverCardPresence::Elsewhere
}

fn resting_surface(output: &str, role: SurfaceRole) -> Surface {
    Surface {
        output: output.to_owned(),
        role,
        layer: role.layer(HoverCardPresence::Elsewhere),
    }
}

fn surfaces_on_two_outputs() -> BTreeMap<Id, Surface> {
    ["DP-1", "DP-2"]
        .into_iter()
        .flat_map(|output| {
            [
                SurfaceRole::Panel,
                SurfaceRole::NotificationToasts,
                SurfaceRole::Reservation(Position::Top),
            ]
            .map(|role| (Id::unique(), resting_surface(output, role)))
        })
        .collect()
}

fn surface_layer(surfaces: &BTreeMap<Id, Surface>, output: &str, role: SurfaceRole) -> Layer {
    surfaces
        .values()
        .find(|surface| surface.output == output && surface.role == role)
        .expect("the surface exists")
        .layer
}

#[test]
fn a_hover_card_lifts_its_panel_above_the_toasts_of_its_output_only() {
    let mut surfaces = surfaces_on_two_outputs();
    let changes = restack_around_hover_card(&mut surfaces, hover_card_on_first_output);
    assert_eq!(changes.len(), 2);
    assert_eq!(
        surface_layer(&surfaces, "DP-1", SurfaceRole::Panel),
        Layer::Overlay
    );
    assert_eq!(
        surface_layer(&surfaces, "DP-1", SurfaceRole::NotificationToasts),
        Layer::Top
    );
    assert_eq!(
        surface_layer(&surfaces, "DP-2", SurfaceRole::Panel),
        Layer::Top
    );
    assert_eq!(
        surface_layer(&surfaces, "DP-2", SurfaceRole::NotificationToasts),
        Layer::Overlay
    );
}

#[test]
fn an_unchanged_hover_card_requests_no_further_layer_changes() {
    let mut surfaces = surfaces_on_two_outputs();
    let _lifted = restack_around_hover_card(&mut surfaces, hover_card_on_first_output);
    assert!(restack_around_hover_card(&mut surfaces, hover_card_on_first_output).is_empty());
}

#[test]
fn every_panel_output_gets_a_toast_surface_without_toasts() {
    let first_panel = Id::unique();
    let second_panel = Id::unique();
    let changes = toast_surface_changes(
        &[
            OutputSurface {
                surface: first_panel,
                output: "DP-1",
            },
            OutputSurface {
                surface: second_panel,
                output: "DP-2",
            },
        ],
        &[],
    );
    assert_eq!(
        changes,
        ToastSurfaceChanges {
            orphaned_surfaces: Vec::new(),
            missing_outputs: vec!["DP-1".to_owned(), "DP-2".to_owned()],
        }
    );
}

#[test]
fn an_open_toast_surface_stays_while_its_panel_exists() {
    let panel = Id::unique();
    let toast_surface = Id::unique();
    let changes = toast_surface_changes(
        &[OutputSurface {
            surface: panel,
            output: "DP-1",
        }],
        &[OutputSurface {
            surface: toast_surface,
            output: "DP-1",
        }],
    );
    assert_eq!(
        changes,
        ToastSurfaceChanges {
            orphaned_surfaces: Vec::new(),
            missing_outputs: Vec::new(),
        }
    );
}

#[test]
fn a_toast_surface_closes_with_its_output() {
    let toast_surface = Id::unique();
    let changes = toast_surface_changes(
        &[],
        &[OutputSurface {
            surface: toast_surface,
            output: "DP-1",
        }],
    );
    assert_eq!(changes.orphaned_surfaces, vec![toast_surface]);
}

#[test]
fn closing_the_hover_card_restores_the_resting_layers() {
    let mut surfaces = surfaces_on_two_outputs();
    let _lifted = restack_around_hover_card(&mut surfaces, hover_card_on_first_output);
    let restored = restack_around_hover_card(&mut surfaces, hover_card_closed);
    assert_eq!(restored.len(), 2);
    assert_eq!(
        surface_layer(&surfaces, "DP-1", SurfaceRole::Panel),
        Layer::Top
    );
    assert_eq!(
        surface_layer(&surfaces, "DP-1", SurfaceRole::NotificationToasts),
        Layer::Overlay
    );
}
