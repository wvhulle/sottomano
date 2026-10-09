use crate::{
    data::source::ai::copilot::{CopilotFree, USER_ENDPOINT},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
        simple_icons::SimpleIcon,
        tooltip_body::{Cell, Section, TooltipBody},
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
            Section::untitled(&[("Plan", self.plan.as_str()), ("Source", USER_ENDPOINT)]),
            Section::titled(
                "Remaining",
                &[
                    ("Chat", Cell::from(self.chat.to_string())),
                    ("Completions", Cell::from(self.completions.to_string())),
                    (
                        "Resets",
                        self.reset.map_or(Cell::Missing, |at| Cell::Time {
                            at,
                            part: TimePart::Date,
                        }),
                    ),
                ],
            ),
        ])
    }
}
