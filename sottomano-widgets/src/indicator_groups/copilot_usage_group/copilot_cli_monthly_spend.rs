use rust_decimal::Decimal;
use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::{
        ccusage_cli_reports::UsageActivity, copilot_usage::CopilotCcusage, spend_money::usd,
        usage_quota::Quota,
    },
};

use crate::{
    clock::Clock,
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        amount_text::spend_text,
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        ongoing_toast::IndicatorAlerts,
        phosphor::{Phosphor, Weight},
        spend_forecast_basis::MONTH_HEADING_FORMAT,
        spend_to_level::level_from_cost,
        value_cells::LabeledValue,
    },
};

pub(crate) const COPILOT_CCUSAGE_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::TerminalWindow(Weight::Regular));

pub fn copilot_ccusage_panel_visibility(ccusage: &CopilotCcusage) -> PanelVisibility {
    if ccusage.period.usage.activity() == UsageActivity::Active {
        PanelVisibility::Shown
    } else {
        PanelVisibility::Hidden
    }
}

pub fn copilot_ccusage_indicator(ccusage: &CopilotCcusage, clock: Clock) -> GroupIndicator<'_> {
    let label = if ccusage.period.usage.cost.is_positive() {
        format!("{} cli", spend_text(&ccusage.period.usage.cost))
    } else {
        "cli".to_owned()
    };
    let quotas = ccusage.quotas(clock.utc());
    let state = Quota::worst_severity(&quotas)
        .unwrap_or_else(|| level_from_cost(&ccusage.period.usage.cost))
        .into();
    GroupIndicator::glyph_only(
        CopilotCcusage::NAME,
        Phosphor::TerminalWindow(Weight::Regular).into(),
        state,
    )
    .with_readout(IndicatorReadout {
        label: ReadoutLabel::Labeled(label),
        gauge: ReadoutGauge::Ungauged,
        badge: ReadoutBadge::Unbadged,
    })
    .with_alerts(IndicatorAlerts {
        quota_windows: quotas,
        ..IndicatorAlerts::default()
    })
    .with_panel_visibility(copilot_ccusage_panel_visibility(ccusage))
    .with_hover_card(Box::new(move |_| copilot_ccusage_hover_card(ccusage)))
}

pub fn copilot_ccusage_hover_card(ccusage: &CopilotCcusage) -> Vec<Section> {
    let usage = &ccusage.period.usage;
    vec![
        Section::new(
            "Subscription",
            dictionary_table(
                [
                    LabeledValue::new("Plan", "copilot-cli"),
                    LabeledValue::new("Source", "ccusage, local CLI session data"),
                ]
                .into_iter()
                .chain([LabeledValue::new(
                    "Monthly Budget",
                    Quantity::spend(usd(Decimal::from(ccusage.budget.monthly_limit_usd))),
                )]),
            ),
        ),
        Section::new(
            ccusage
                .period
                .span
                .since
                .format(MONTH_HEADING_FORMAT)
                .to_string(),
            dictionary_table([
                LabeledValue::new("Tokens", Quantity::count(usage.tokens)),
                LabeledValue::new("Cost", Quantity::spend(usage.cost)),
                LabeledValue::new("Models", usage.models.to_string()),
            ]),
        ),
    ]
}
