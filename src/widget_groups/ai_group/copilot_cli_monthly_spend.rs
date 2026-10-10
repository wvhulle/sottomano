use crate::{
    data::source::ai::copilot::CopilotCcusage,
    widget_groups::ai_group::spend_to_level::level_from_cost,
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        simple_icons::SimpleIcon,
        tooltip_body::{KeyValue, Section, TooltipBody},
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
        TooltipBody::sections(&[
            Section::untitled_dictionary([
                KeyValue::new("Plan", "copilot-cli"),
                KeyValue::new("Source", "ccusage, local CLI session data"),
            ]),
            Section::dictionary(
                self.month.clone(),
                [
                    KeyValue::new("Tokens", self.usage.tokens.to_string()),
                    KeyValue::new("Cost", format!("${:.2}", self.usage.cost)),
                    KeyValue::new("Models", self.usage.models.clone()),
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
