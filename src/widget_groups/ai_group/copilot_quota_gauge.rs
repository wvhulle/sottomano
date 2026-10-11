use chrono::Utc;
use uom::si::f64::Ratio;

use crate::{
    data::source::ai::{
        copilot::{CopilotMetered, USER_ENDPOINT},
        quota::QuotaLimited,
    },
    widget_groups::ai_group::{
        amount_text::{percent_text, to_cents},
        spend_to_level::level_from_used,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        localized_time::TimePart,
        simple_icons::SimpleIcon,
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for CopilotMetered {
    fn name(&self) -> &'static str {
        "Copilot quota"
    }

    fn icon(&self) -> Glyph {
        SimpleIcon::GithubCopilot.into()
    }

    fn state(&self) -> WidgetState {
        self.quota_severity(Utc::now())
            .unwrap_or_else(|| level_from_used(self.used_share))
            .into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.used_share)
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
                self.quota.clone(),
                [
                    KeyValue::new("Used", to_cents(self.used)),
                    KeyValue::new("Used Share", percent_text(self.used_share)),
                    KeyValue::new("Remaining", to_cents(self.remaining)),
                    KeyValue::new("Limit", self.limit.to_string()),
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

impl Occasional for CopilotMetered {
    fn present(&self) -> bool {
        self.used > 0.0
    }
}
