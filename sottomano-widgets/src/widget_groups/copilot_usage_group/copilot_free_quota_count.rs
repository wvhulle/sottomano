use sottomano_sources::ai::copilot::{CopilotFree, USER_ENDPOINT};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    localized_time::LocalizedTimeFormat,
    phosphor::{Phosphor, Weight},
    tooltip_body::{Cell, KeyValue, Section, TooltipBody},
    widget_state::WidgetState,
};

impl BarWidget for CopilotFree {
    fn name(&self) -> &'static str {
        "Copilot free quota"
    }

    fn icon(&self) -> Glyph {
        Phosphor::ChatCircleDots(self.state().weight()).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::ChatCircleDots(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        Some(format!("{} chats", self.chat))
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
        TooltipBody::sections([
            Section::dictionary(
                self.plan
                    .as_deref()
                    .map(|plan| KeyValue::new("Plan", plan))
                    .into_iter()
                    .chain([KeyValue::new("Source", USER_ENDPOINT)]),
            ),
            Section::dictionary(
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
                            format: LocalizedTimeFormat::Date,
                        },
                    )
                })),
            )
            .titled("Remaining"),
        ])
    }
}
