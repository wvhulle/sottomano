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
        tooltip_body::{Cell, Section, TooltipBody},
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
        TooltipBody::sections(&[
            Section::untitled(&[("Plan", self.plan.as_str()), ("Source", USER_ENDPOINT)]),
            Section::titled(
                self.quota.clone(),
                &[
                    ("Used", Cell::from(to_cents(self.used))),
                    ("Used Share", Cell::from(percent_text(self.percent_used))),
                    ("Remaining", Cell::from(to_cents(self.remaining))),
                    ("Limit", Cell::from(self.limit.to_string())),
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

impl Occasional for CopilotMetered {
    fn present(&self) -> bool {
        self.used > 0.0
    }
}
