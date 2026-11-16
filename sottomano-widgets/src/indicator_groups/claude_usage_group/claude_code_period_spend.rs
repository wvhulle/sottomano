use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::{
        ccusage_cli_reports::Period, claude_usage::ClaudeCcusagePeriods, usage_quota::Quota,
    },
};

use crate::{
    clock::Clock,
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        amount_text::spend_text,
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        ongoing_toast::IndicatorAlerts,
        phosphor::{Phosphor, Weight},
        spend_forecast_basis::{MONTH_HEADING_FORMAT, WEEK_HEADING_FORMAT},
        spend_to_level::level_from_cost,
        value_cells::LabeledValue,
    },
};

pub(crate) const CLAUDE_CCUSAGE_PERIODS_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::CalendarDots(Weight::Regular));

pub fn claude_ccusage_periods_indicator(
    periods: &ClaudeCcusagePeriods,
    clock: Clock,
) -> GroupIndicator<'_> {
    let quota_windows = periods.quotas(clock.utc());
    let state = Quota::worst_severity(&quota_windows)
        .unwrap_or_else(|| level_from_cost(&periods.recent.today.usage.cost))
        .into();
    GroupIndicator::glyph_only(
        ClaudeCcusagePeriods::NAME,
        Phosphor::CalendarDots(Weight::Regular).into(),
        state,
    )
    .with_readout(IndicatorReadout {
        label: ReadoutLabel::Labeled(format!(
            "{} today",
            spend_text(&periods.recent.today.usage.cost)
        )),
        gauge: ReadoutGauge::Ungauged,
        badge: ReadoutBadge::Unbadged,
    })
    .with_alerts(IndicatorAlerts {
        quota_windows,
        ..IndicatorAlerts::default()
    })
    .with_hover_card(Box::new(move |card_clock| {
        claude_ccusage_periods_hover_card(periods, card_clock)
    }))
}

pub fn claude_ccusage_periods_hover_card(
    periods: &ClaudeCcusagePeriods,
    clock: Clock,
) -> Vec<Section> {
    let recent = &periods.recent;
    let now = clock.local();
    let period_section = |title: String, period: &Period, threshold: Option<LabeledValue>| {
        let projected = period
            .projected_cost(now, &periods.budget)
            .map(|cost| LabeledValue::new("Projected Cost", Quantity::spend(cost)));
        Section::new(
            title,
            dictionary_table(
                [
                    Some(LabeledValue::new(
                        "Cost",
                        Quantity::spend(period.usage.cost),
                    )),
                    projected,
                    threshold,
                    Some(LabeledValue::new(
                        "Tokens",
                        Quantity::compact_count(period.usage.tokens),
                    )),
                    Some(LabeledValue::new("Models", period.usage.models.to_string())),
                ]
                .into_iter()
                .flatten(),
            ),
        )
    };
    vec![
        Section::spend_forecast_basis(
            [
                LabeledValue::new("Source", "ccusage, local Claude Code logs"),
                LabeledValue::new("Cost Basis", "API list prices"),
            ],
            &periods.budget,
        ),
        period_section("Today".to_owned(), &recent.today, None),
        period_section(
            recent
                .week
                .span
                .since
                .format(WEEK_HEADING_FORMAT)
                .to_string(),
            &recent.week,
            None,
        ),
        period_section(
            recent
                .month
                .span
                .since
                .format(MONTH_HEADING_FORMAT)
                .to_string(),
            &recent.month,
            Some(LabeledValue::monthly_budget(
                &periods.budget,
                &recent.month.usage.cost,
                recent.month.projected_cost(now, &periods.budget),
            )),
        ),
    ]
}
