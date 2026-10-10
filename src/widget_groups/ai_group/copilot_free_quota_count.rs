use crate::{
    data::source::ai::copilot::{CopilotFree, USER_ENDPOINT},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
        simple_icons::SimpleIcon,
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for CopilotFree {
    fn icon(&self) -> Glyph {
        SimpleIcon::GithubCopilot.into()
    }

    fn label(&self) -> Option<String> {
        Some(self.chat.to_string())
    }

    fn state(&self) -> WidgetState {
        if self.chat <= 5 {
            WidgetState::Critical
        } else if self.chat <= 20 {
            WidgetState::Warning
        } else {
            WidgetState::Idle
        }
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[
            Section::untitled_dictionary(
                self.plan
                    .as_deref()
                    .map(|plan| KeyValue::new("Plan", plan))
                    .into_iter()
                    .chain([KeyValue::new("Source", USER_ENDPOINT)]),
            ),
            Section::dictionary(
                "Remaining",
                [
                    KeyValue::new("Chat", self.chat.to_string()),
                    KeyValue::new("Completions", self.completions.to_string()),
                ]
                .into_iter()
                .chain(self.reset.map(|at| {
                    KeyValue::new(
                        "Resets",
                        Cell::Time {
                            at,
                            part: TimePart::Date,
                        },
                    )
                })),
            ),
        ])
    }
}
