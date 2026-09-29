use crate::{
    data::source::ai::copilot::CopilotCcusage,
    widget_groups::ai_group::spend_to_level::level_from_cost,
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        simple_icons::SimpleIcon,
        tooltip_body::{TooltipBody, row, section, untitled},
        widget_state::WidgetState,
    },
};

impl BarWidget for CopilotCcusage {
    fn icon(&self) -> Glyph {
        SimpleIcon::GithubCopilot.into()
    }

    fn label(&self) -> Option<String> {
        Some(if self.usage.cost > 0.0 {
            format!("${:.2}", self.usage.cost)
        } else {
            "cli".to_owned()
        })
    }

    fn state(&self) -> WidgetState {
        level_from_cost(self.usage.cost).into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Sections(vec![
            untitled(vec![
                row("Plan", "copilot-cli"),
                row("Source", "ccusage, local CLI session data"),
            ]),
            section(
                self.month.clone(),
                vec![
                    row("Tokens", self.usage.tokens.to_string()),
                    row("Cost", format!("${:.2}", self.usage.cost)),
                    row("Models", self.usage.models.clone()),
                ],
            ),
        ])
    }
}

impl Occasional for CopilotCcusage {
    fn present(&self) -> bool {
        self.usage.active()
    }
}
