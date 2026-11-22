use corredo::theming::{
    FrameBackground, FrameStyle, OutlineStyle, OutlineTracing, ShadowCasting, ShadowStyle,
};
use sottomano_desktop_sources::sqlite_state_store::StoreError;

use super::{AppearanceMessage, LayerAppearanceControls};
use crate::{
    desktop_panels::region_reporter::BackdropBlur,
    group_indicators::slider_control::ControlMessage,
    widgets::shell_layer_appearance::{LayerAppearance, ShellLayer},
};

fn panel_controls() -> LayerAppearanceControls {
    LayerAppearanceControls::new(ShellLayer::Panels)
}

#[test]
fn a_fresh_layer_reads_its_default_appearance() {
    let overlays = LayerAppearanceControls::new(ShellLayer::Overlays);
    assert_eq!(
        overlays.appearance(),
        ShellLayer::Overlays.default_appearance()
    );
}

#[test]
fn a_menu_layer_keeps_its_own_background_through_a_load() {
    let mut menus = LayerAppearanceControls::new(ShellLayer::Menus);
    let _ = menus.update(AppearanceMessage::Loaded(
        ShellLayer::Overlays.default_appearance(),
    ));
    assert_eq!(menus.appearance().frame.background, FrameBackground::Weak);
}

#[test]
fn a_loaded_appearance_reads_back_unchanged() {
    let loaded = LayerAppearance {
        frame: FrameStyle {
            background: FrameBackground::Base,
            opacity: 0.5,
            corner_radius_px: 3.0,
            shadow: ShadowStyle {
                casting: ShadowCasting::Omitted,
                softness_px: 12.0,
                opacity: 0.25,
            },
            outline: OutlineStyle {
                tracing: OutlineTracing::Omitted,
                opacity: 0.75,
            },
            ..FrameStyle::default()
        },
        backdrop_blur: BackdropBlur::Clear,
    };
    let mut appearance_controls = panel_controls();
    let _ = appearance_controls.update(AppearanceMessage::Loaded(loaded));
    assert_eq!(appearance_controls.appearance(), loaded);
}

#[test]
fn a_failed_save_keeps_the_settled_corner_radius() {
    let mut appearance_controls = panel_controls();
    let settled = appearance_controls.appearance().frame.corner_radius_px;
    let _ = appearance_controls.update(AppearanceMessage::CornerRadius(ControlMessage::Set(2.0)));
    let _ = appearance_controls.update(AppearanceMessage::CornerRadius(ControlMessage::Release));
    assert!(
        (appearance_controls.appearance().frame.corner_radius_px - settled).abs() < f32::EPSILON
    );
    let _ = appearance_controls.update(AppearanceMessage::CornerRadius(ControlMessage::persisted(
        2.0,
        Err(StoreError::NoRow),
    )));
    assert!(
        (appearance_controls.appearance().frame.corner_radius_px - settled).abs() < f32::EPSILON
    );
    let _ = appearance_controls.update(AppearanceMessage::CornerRadius(ControlMessage::persisted(
        2.0,
        Ok::<(), StoreError>(()),
    )));
    assert!((appearance_controls.appearance().frame.corner_radius_px - 2.0).abs() < f32::EPSILON);
}
