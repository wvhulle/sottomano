use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::{
        openrouter_credits::{OpenRouterSpend, SpanSpend},
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
        amount_text::spend_text,
        localized_quantities::Quantity,
        ongoing_toast::IndicatorAlerts,
        phosphor::{Phosphor, Weight},
        spend_forecast_basis::{MONTH_HEADING_FORMAT, WEEK_HEADING_FORMAT},
        value_cells::LabeledValue,
    },
};

pub(crate) fn spend_panel_visibility(spend: &OpenRouterSpend) -> PanelVisibility {
    if spend.month.models.total().is_positive() {
        PanelVisibility::Shown
    } else {
        PanelVisibility::Hidden
    }
}

pub(crate) fn spend_indicator(spend: &OpenRouterSpend, clock: Clock) -> ReadingRow<'_> {
    let today = spend.today.models.total();
    let label = if today.is_positive() {
        ReadoutLabel::Labeled(format!("{} today", spend_text(&today)))
    } else {
        ReadoutLabel::Unlabeled
    };
    let quotas = spend.quotas(clock.utc());
    let state = Quota::worst_severity(&quotas)
        .map_or(spend.daily_severity(), |monthly| {
            monthly.max(spend.daily_severity())
        })
        .into();
    ReadingRow {
        readout: IndicatorReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        },
        alerts: IndicatorAlerts {
            quota_windows: quotas,
            ..IndicatorAlerts::default()
        },
        panel_visibility: spend_panel_visibility(spend),
        ..ReadingRow::glyph_only(
            OpenRouterSpend::NAME,
            Phosphor::Coins(Weight::Regular).into(),
            state,
            Box::new(move |card_clock| spend_hover_card(spend, card_clock)),
        )
    }
}

pub(crate) fn spend_hover_card(spend: &OpenRouterSpend, clock: Clock) -> Vec<Section> {
    let now = clock.local();
    let daily_warning = LabeledValue::new("Daily Warning", Quantity::spend(spend.daily_warning()))
        .level(spend.daily_severity());
    let monthly_budget = LabeledValue::monthly_budget(
        &spend.budget,
        &spend.month.models.total(),
        spend.month.projected_cost(now, &spend.budget),
    );
    let span_section = |title: String, spent: &SpanSpend, threshold: Option<LabeledValue>| {
        let projected = spent
            .projected_cost(now, &spend.budget)
            .map(|cost| LabeledValue::new("Projected Cost", Quantity::spend(cost)));
        Section::new(
            title,
            dictionary_table(
                [
                    Some(LabeledValue::new(
                        "Cost",
                        Quantity::spend(spent.models.total()),
                    )),
                    projected,
                    threshold,
                ]
                .into_iter()
                .flatten(),
            ),
        )
    };
    vec![
        Section::spend_forecast_basis(
            [LabeledValue::new("Source", "OpenRouter analytics")],
            &spend.budget,
        ),
        span_section("Today".to_owned(), &spend.today, Some(daily_warning)),
        span_section(
            spend
                .week
                .span
                .since
                .format(WEEK_HEADING_FORMAT)
                .to_string(),
            &spend.week,
            None,
        ),
        span_section(
            spend
                .month
                .span
                .since
                .format(MONTH_HEADING_FORMAT)
                .to_string(),
            &spend.month,
            Some(monthly_budget),
        ),
    ]
}
