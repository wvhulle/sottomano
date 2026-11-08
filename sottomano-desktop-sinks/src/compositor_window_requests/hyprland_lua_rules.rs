use std::fmt::Write;

use sottomano_desktop_sources::compositor_window_ipc::compositor_styling_model::{
    CompositorStyling, HeatedWindow, ShellBlurDepth, StyledSurface, SwitchHeat, escaped_for_regex,
};

fn lua_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn hyprland_color(palette_color: &str) -> String {
    let hex_digits = palette_color.trim_start_matches('#');
    match hex_digits.len() {
        6 => format!("rgb({hex_digits})"),
        _ => format!("rgba({hex_digits})"),
    }
}

fn general_section(styling: CompositorStyling, switch_heat: &SwitchHeat) -> String {
    let gaps = styling.layout.gaps_px.round();
    let border_colors = match switch_heat {
        SwitchHeat::Hidden => String::new(),
        SwitchHeat::Shown {
            focus_ring_color,
            neutral_border_color,
            ..
        } => format!(
            "        col = {{\n            active_border = {},\n            inactive_border = \
             {},\n        }},\n",
            lua_string(&hyprland_color(focus_ring_color)),
            lua_string(&hyprland_color(neutral_border_color)),
        ),
    };
    format!(
        "    general = {{\n        gaps_in = {},\n        gaps_out = {gaps},\n{border_colors}    \
         }},\n",
        (gaps / 2.0).round()
    )
}

fn layer_rule_of(surface: StyledSurface, styling: CompositorStyling) -> Option<String> {
    let (StyledSurface::Panel | StyledSurface::NotificationToasts) = surface else {
        return None;
    };
    let xray = styling.blur_depth == ShellBlurDepth::WallpaperOnly;
    Some(format!(
            "hl.layer_rule({{\n    name = {},\n    match = {{ namespace = {} }},\n    blur = \
             true,\n    xray = {xray},\n}})\n",
            lua_string(surface.namespace()),
            lua_string(&format!("^{}$", surface.namespace())),
        ))
}

fn heated_window_rule(heat_index: usize, heated: &HeatedWindow) -> String {
    let mut window_match = format!(
        "title = {}",
        lua_string(&format!(
            "^\\W*{}$",
            escaped_for_regex(heated.title_without_status_prefix())
        ))
    );
    if let Some(app_id) = heated.app_id() {
        let _ = write!(
            window_match,
            ", class = {}",
            lua_string(&format!("^{}$", escaped_for_regex(app_id)))
        );
    }
    format!(
        "hl.window_rule({{\n    name = \"sottomano-switch-heat-{heat_index}\",\n    match = {{ \
         {window_match} }},\n    border_color = {},\n}})\n",
        lua_string(&hyprland_color(heated.border_color())),
    )
}

#[must_use]
pub fn rendered_hyprland_rules(styling: CompositorStyling, switch_heat: &SwitchHeat) -> String {
    let config = format!(
        "hl.config({{\n{}    decoration = {{\n        rounding = {},\n    }},\n}})\n",
        general_section(styling, switch_heat),
        styling.layout.window_corner_radius_px.round()
    );
    let layer_rules = StyledSurface::ALL
        .into_iter()
        .filter_map(|surface| layer_rule_of(surface, styling));
    let window_rules = switch_heat
        .heated_windows()
        .iter()
        .enumerate()
        .map(|(heat_index, heated)| heated_window_rule(heat_index, heated));
    [config]
        .into_iter()
        .chain(layer_rules)
        .chain(window_rules)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::compositor_window_ipc::compositor_styling_model::{
        AppWindowLayout, CompositorStyling, HeatedWindow, ShellBlurDepth, SwitchHeat,
    };

    use super::rendered_hyprland_rules;

    fn styled() -> CompositorStyling {
        CompositorStyling {
            blur_depth: ShellBlurDepth::WallpaperOnly,
            layout: AppWindowLayout {
                window_corner_radius_px: 7.5,
                gaps_px: 10.0,
            },
        }
    }

    #[test]
    fn layout_and_shell_blur_render_without_heat() {
        assert_eq!(
            rendered_hyprland_rules(styled(), &SwitchHeat::Hidden),
            r#"hl.config({
    general = {
        gaps_in = 5,
        gaps_out = 10,
    },
    decoration = {
        rounding = 8,
    },
})

hl.layer_rule({
    name = "sottomano-panel",
    match = { namespace = "^sottomano-panel$" },
    blur = true,
    xray = true,
})

hl.layer_rule({
    name = "sottomano-notification-toasts",
    match = { namespace = "^sottomano-notification-toasts$" },
    blur = true,
    xray = true,
})
"#
        );
    }

    #[test]
    fn a_heated_window_gets_a_border_rule_matched_by_its_escaped_title() {
        let switch_heat = SwitchHeat::Shown {
            focus_ring_color: "#95e6cb".to_owned(),
            neutral_border_color: "#0b0e14".to_owned(),
            windows: HeatedWindow::titled(
                Some("kitty".to_owned()),
                "✳ nvim (main)",
                "#ffb454".to_owned(),
            )
            .into_iter()
            .collect(),
        };
        let rendered = rendered_hyprland_rules(styled(), &switch_heat);
        assert!(rendered.contains("active_border = \"rgb(95e6cb)\""));
        assert!(rendered.contains(
            "match = { title = \"^\\\\W*nvim \\\\(main\\\\)$\", class = \"^kitty$\" },\n    \
             border_color = \"rgb(ffb454)\","
        ));
    }
}
