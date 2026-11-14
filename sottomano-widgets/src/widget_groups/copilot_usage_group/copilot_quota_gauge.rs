use std::borrow::Cow;

use chrono::{DateTime, Utc};
use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::{
        copilot_usage::{CopilotMetered, USER_ENDPOINT},
        usage_quota::Quota,
    },
};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::{GroupItemReading, PanelVisibility},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::{DisplayUnit, Quantity},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        spend_to_level::level_from_used,
        value_cells::{LabeledValue, ValueCell},
    },
};

impl GroupItemReading for CopilotMetered {
    fn name(&self) -> Cow<'static, str> {
        Self::NAME.into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Gauge(Weight::Regular).into()
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::Gauge(Weight::Regular).into()
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        Quota::worst_severity(&self.quotas(clock.utc()))
            .unwrap_or_else(|| level_from_used(self.used_share))
            .into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.used_share)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([
            Section::dictionary(
                self.plan
                    .as_deref()
                    .map(|plan| LabeledValue::new("Plan", plan))
                    .into_iter()
                    .chain([LabeledValue::new("Source", USER_ENDPOINT)]),
            ),
            Section::dictionary(
                [
                    LabeledValue::new(
                        "Used",
                        Quantity::measured(self.used, DisplayUnit::Unitless, 2),
                    ),
                    LabeledValue::new("Used Share", Quantity::share(self.used_share).decimals(2)),
                    LabeledValue::new(
                        "Remaining",
                        Quantity::measured(self.remaining, DisplayUnit::Unitless, 2),
                    ),
                    LabeledValue::new("Limit", Quantity::count(self.limit)),
                ]
                .into_iter()
                .chain(self.reset.map(|at| {
                    LabeledValue::new(
                        "Resets",
                        ValueCell::Time {
                            at,
                            format: LocalizedTimeFormat::Date,
                        },
                    )
                })),
            )
            .titled(self.quota.clone()),
        ])
    }

    fn panel_visibility(&self) -> PanelVisibility {
        if self.used > 0.0 {
            PanelVisibility::Shown
        } else {
            PanelVisibility::Hidden
        }
    }

    fn quota_windows(&self, now: DateTime<Utc>) -> Vec<Quota> {
        self.quotas(now)
    }
}
