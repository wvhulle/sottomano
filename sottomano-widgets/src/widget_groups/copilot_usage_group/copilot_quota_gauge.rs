use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::{
        copilot_usage::{CopilotMetered, USER_ENDPOINT},
        usage_quota::Quota,
    },
};

use crate::{
    clock::Clock,
    group_indicators::{
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        reading_row::{PanelVisibility, ReadingRow},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::{DisplayUnit, Quantity},
        localized_time::LocalizedTimeFormat,
        ongoing_toast::IndicatorAlerts,
        phosphor::{Phosphor, Weight},
        spend_to_level::level_from_used,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub fn copilot_metered_indicator(metered: &CopilotMetered, clock: Clock) -> ReadingRow<'_> {
    let state = Quota::worst_severity(&metered.quotas(clock.utc()))
        .unwrap_or_else(|| level_from_used(metered.used_share))
        .into();
    ReadingRow {
        readout: IndicatorReadout {
            label: ReadoutLabel::Unlabeled,
            gauge: ReadoutGauge::Gauged {
                fill: metered.used_share,
                live: metered.used_share,
            },
            badge: ReadoutBadge::Unbadged,
        },
        alerts: IndicatorAlerts {
            quota_windows: metered.quotas(clock.utc()),
            ..IndicatorAlerts::default()
        },
        panel_visibility: copilot_metered_panel_visibility(metered),
        ..ReadingRow::glyph_only(
            CopilotMetered::NAME,
            Phosphor::Gauge(Weight::Regular).into(),
            state,
            Box::new(move |_| copilot_metered_hover_card(metered)),
        )
    }
}

pub fn copilot_metered_panel_visibility(metered: &CopilotMetered) -> PanelVisibility {
    if metered.used > 0.0 {
        PanelVisibility::Shown
    } else {
        PanelVisibility::Hidden
    }
}

pub fn copilot_metered_hover_card(metered: &CopilotMetered) -> Vec<Section> {
    vec![
        Section::new(
            "Subscription",
            dictionary_table(
                metered
                    .plan
                    .as_deref()
                    .map(|plan| LabeledValue::new("Plan", plan))
                    .into_iter()
                    .chain([LabeledValue::new("Source", USER_ENDPOINT)]),
            ),
        ),
        Section::new(
            metered.quota.clone(),
            dictionary_table(
                [
                    LabeledValue::new(
                        "Used",
                        Quantity::measured(metered.used, DisplayUnit::Unitless, 2),
                    ),
                    LabeledValue::new(
                        "Used Share",
                        Quantity::share(metered.used_share).decimals(2),
                    ),
                    LabeledValue::new(
                        "Remaining",
                        Quantity::measured(metered.remaining, DisplayUnit::Unitless, 2),
                    ),
                    LabeledValue::new("Limit", Quantity::count(metered.limit)),
                ]
                .into_iter()
                .chain(metered.reset.map(|at| {
                    LabeledValue::new(
                        "Resets",
                        ValueCell::Time {
                            at,
                            format: LocalizedTimeFormat::Date,
                        },
                    )
                })),
            ),
        ),
    ]
}
