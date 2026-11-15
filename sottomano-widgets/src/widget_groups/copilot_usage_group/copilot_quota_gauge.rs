use chrono::Locale;
use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::{
        copilot_usage::{CopilotMetered, USER_ENDPOINT},
        usage_quota::Quota,
    },
};

use crate::{
    clock::Clock,
    group_items::{
        reading_row::{PanelVisibility, ReadingRow},
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::{DisplayUnit, Quantity},
        localized_time::LocalizedTimeFormat,
        ongoing_toast::ReadingAlerts,
        phosphor::{Phosphor, Weight},
        spend_to_level::level_from_used,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub(crate) const COPILOT_METERED_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::Gauge(Weight::Regular));

pub fn copilot_metered_reading_row(
    metered: &CopilotMetered,
    clock: Clock,
    _locale: Locale,
) -> ReadingRow<'_> {
    let state = Quota::worst_severity(&metered.quotas(clock.utc()))
        .unwrap_or_else(|| level_from_used(metered.used_share))
        .into();
    ReadingRow {
        readout: RowReadout {
            label: ReadoutLabel::Unlabeled,
            gauge: ReadoutGauge::Gauged {
                fill: metered.used_share,
                live: metered.used_share,
            },
            badge: ReadoutBadge::Unbadged,
        },
        alerts: ReadingAlerts {
            quota_windows: metered.quotas(clock.utc()),
            ..ReadingAlerts::default()
        },
        panel_visibility: copilot_metered_panel_visibility(metered),
        ..ReadingRow::glyph_only(
            CopilotMetered::NAME,
            Phosphor::Gauge(Weight::Regular).into(),
            state,
            Box::new(move |card_clock| copilot_metered_hover_card(metered, card_clock)),
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

pub fn copilot_metered_hover_card(metered: &CopilotMetered, _clock: Clock) -> Vec<Section> {
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
