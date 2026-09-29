use crate::{
    data::source::ai::copilot::{CopilotFree, USER_ENDPOINT},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
        simple_icons::SimpleIcon,
        tooltip_body::{TooltipBody, row, section, time_row, untitled},
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
        let mut remaining = vec![
            row("Chat", self.chat.to_string()),
            row("Completions", self.completions.to_string()),
        ];
        remaining.extend(
            self.reset
                .map(|reset| time_row("Resets", reset, TimePart::Date)),
        );
        TooltipBody::Sections(vec![
            untitled(vec![
                row("Plan", self.plan.clone()),
                row("Source", USER_ENDPOINT),
            ]),
            section("Remaining", remaining),
        ])
    }
}
