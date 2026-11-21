use sottomano_compositor_model::compositor_styling_model::{
    AppWindowLayout, CompositorStyling, HeatedWindow, ShellBlurDepth, StyledSurface, SwitchHeat,
};

use super::{BackgroundEffect, NamespaceMatch, SottomanoRules, WindowAppId};
use crate::niri_kdl_config::parse_niri_kdl;

fn parsed_rules(text: &str) -> SottomanoRules {
    club_kdl::from_kdl_document(&parse_niri_kdl(text).unwrap()).unwrap()
}

impl From<SottomanoRules> for CompositorStyling {
    fn from(rules: SottomanoRules) -> Self {
        let panel = NamespaceMatch::from(StyledSurface::Panel);
        let defaults = AppWindowLayout::default();
        Self {
            blur_depth: if rules
                .layer_rules
                .into_iter()
                .filter(|rule| rule.matches == panel)
                .flat_map(|rule| rule.background_effects)
                .any(|effect| effect.xray)
            {
                ShellBlurDepth::WallpaperOnly
            } else {
                ShellBlurDepth::WindowsBehind
            },
            layout: AppWindowLayout {
                window_corner_radius_px: rules
                    .window_rules
                    .iter()
                    .filter(|rule| rule.matches.is_none())
                    .find_map(|rule| rule.geometry_corner_radius)
                    .unwrap_or(defaults.window_corner_radius_px),
                gaps_px: rules.layout.gaps,
            },
        }
    }
}

fn heated_terminal_titled(title: &str) -> SwitchHeat {
    SwitchHeat::Shown {
        focus_ring_color: "#95e6cb".to_owned(),
        neutral_border_color: "#0b0e14".to_owned(),
        windows: HeatedWindow::titled(
            WindowAppId::Reported("kitty".to_owned()),
            title,
            "#ffb454".to_owned(),
        )
        .into_iter()
        .collect(),
    }
}

fn heated_terminal() -> SwitchHeat {
    heated_terminal_titled("nvim (main) \"notes\".md")
}

fn styled() -> CompositorStyling {
    CompositorStyling {
        blur_depth: ShellBlurDepth::WallpaperOnly,
        layout: AppWindowLayout {
            window_corner_radius_px: 7.5,
            gaps_px: 5.0,
        },
    }
}

#[test]
fn a_rendered_look_reads_back_unchanged() {
    let written = styled();
    let text = SottomanoRules::styled(written, &SwitchHeat::Hidden)
        .to_kdl_v1()
        .unwrap();
    let read = CompositorStyling::from(parsed_rules(&text));
    assert_eq!(read, written);
}

#[test]
fn heated_windows_leave_the_look_unchanged_on_sample_back() {
    let written = styled();
    let text = SottomanoRules::styled(written, &heated_terminal())
        .to_kdl_v1()
        .unwrap();
    let read = CompositorStyling::from(parsed_rules(&text));
    assert_eq!(read, written);
}

#[test]
fn a_heated_window_is_matched_by_its_escaped_title() {
    let rules = SottomanoRules::styled(styled(), &heated_terminal());
    let heated_match = rules.window_rules[1].matches.as_ref().unwrap();
    assert_eq!(heated_match.app_id.as_deref(), Some("^kitty$"));
    assert_eq!(heated_match.title, "^\\W*nvim \\(main\\) \"notes\"\\.md$");
}

#[test]
fn a_spinning_status_prefix_keeps_the_same_rule() {
    assert_eq!(
        heated_terminal_titled("✳ enable-flash-attention"),
        heated_terminal_titled("◐ enable-flash-attention")
    );
}

#[test]
fn every_window_gets_the_neutral_border_while_heat_is_shown() {
    let shown = SottomanoRules::styled(styled(), &heated_terminal());
    let neutral = shown.window_rules[0].border.as_ref().unwrap();
    assert_eq!(neutral.active_color, "#0b0e14");
    assert_eq!(neutral.inactive_color, "#0b0e14");
    let hidden = SottomanoRules::styled(styled(), &SwitchHeat::Hidden);
    assert!(hidden.window_rules[0].border.is_none());
}

#[test]
fn the_focus_ring_marks_focus_only_while_heat_is_shown() {
    let shown = SottomanoRules::styled(styled(), &heated_terminal());
    let hidden = SottomanoRules::styled(styled(), &SwitchHeat::Hidden);
    assert!(shown.window_rules[0].focus_ring.is_some());
    assert!(hidden.window_rules[0].focus_ring.is_none());
    assert_eq!(hidden.window_rules.len(), 1);
}

#[test]
fn the_panel_leaves_shadows_and_corners_to_sottomano() {
    let panel = NamespaceMatch::from(StyledSurface::Panel);
    let rules = SottomanoRules::styled(styled(), &SwitchHeat::Hidden);
    let panel_rule = rules
        .layer_rules
        .iter()
        .find(|rule| rule.matches == panel)
        .expect("panel rule");
    assert!(matches!(
        panel_rule.background_effects.as_slice(),
        [BackgroundEffect { blur: true, .. }]
    ));
    assert!(panel_rule.shadows.is_empty());
}
