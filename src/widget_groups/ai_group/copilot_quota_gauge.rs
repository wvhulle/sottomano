use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    data::source::ai::copilot::{CopilotMetered, USER_ENDPOINT},
    widget_groups::ai_group::{
        amount_text::{percent_text, to_cents},
        spend_to_level::level_from_used,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        localized_time::TimePart,
        simple_icons::SimpleIcon,
        tooltip_body::{TooltipBody, row, section, time_row, untitled},
        widget_state::WidgetState,
    },
};

impl BarWidget for CopilotMetered {
    fn icon(&self) -> Glyph {
        SimpleIcon::GithubCopilot.into()
    }

    fn state(&self) -> WidgetState {
        level_from_used(self.percent_used).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(Ratio::new::<percent>(self.percent_used))
    }

    fn tooltip(&self) -> TooltipBody {
        let mut quota = vec![
            row("Used", to_cents(self.used)),
            row("Used Share", percent_text(self.percent_used)),
            row("Remaining", to_cents(self.remaining)),
            row("Limit", self.limit.to_string()),
        ];
        quota.extend(
            self.reset
                .map(|reset| time_row("Resets", reset, TimePart::Date)),
        );
        TooltipBody::Sections(vec![
            untitled(vec![
                row("Plan", self.plan.clone()),
                row("Source", USER_ENDPOINT),
            ]),
            section(self.quota.clone(), quota),
        ])
    }
}

impl Occasional for CopilotMetered {
    fn present(&self) -> bool {
        self.used > 0.0
    }
}
