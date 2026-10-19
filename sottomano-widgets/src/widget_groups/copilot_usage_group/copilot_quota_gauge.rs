use chrono::{DateTime, Utc};
use sottomano_sources::ai_usage_spend::{
    copilot_usage::{CopilotMetered, USER_ENDPOINT},
    usage_quota::Quota,
};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        amount_text::{percent_text, two_decimals},
        bar_widget::{BarWidget, Glyph},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        spend_to_level::level_from_used,
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for CopilotMetered {
    fn name(&self) -> &'static str {
        "Copilot quota"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Gauge(Weight::Regular).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Gauge(Weight::Regular).into()
    }

    fn state(&self, clock: Clock) -> WidgetState {
        Quota::worst_severity(&self.quotas(clock.utc()))
            .unwrap_or_else(|| level_from_used(self.used_share))
            .into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.used_share)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
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
                    KeyValue::new("Used", two_decimals(self.used)),
                    KeyValue::new("Used Share", percent_text(self.used_share)),
                    KeyValue::new("Remaining", two_decimals(self.remaining)),
                    KeyValue::new("Limit", self.limit.to_string()),
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
            .titled(self.quota.clone()),
        ])
    }

    fn visible_in_bar(&self) -> bool {
        self.used > 0.0
    }

    fn quota_windows(&self, now: DateTime<Utc>) -> Vec<Quota> {
        self.quotas(now)
    }
}
