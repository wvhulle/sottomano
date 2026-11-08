use sottomano_desktop_sources::compositor_window_ipc::compositor_styling_model::CompositorStyling;

#[must_use]
pub fn rendered_sway_rules(styling: CompositorStyling) -> String {
    format!("gaps inner {}\n", styling.layout.gaps_px.round())
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::compositor_window_ipc::compositor_styling_model::{
        AppWindowLayout, CompositorStyling,
    };

    use super::rendered_sway_rules;

    #[test]
    fn gaps_are_rounded_to_whole_pixels() {
        let styling = CompositorStyling {
            layout: AppWindowLayout {
                gaps_px: 7.6,
                ..AppWindowLayout::default()
            },
            ..CompositorStyling::default()
        };
        assert_eq!(rendered_sway_rules(styling), "gaps inner 8\n");
    }
}
