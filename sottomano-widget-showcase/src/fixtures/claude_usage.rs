use std::time::Duration;

use chrono::TimeDelta;
use rust_decimal::Decimal;
use rusty_money::iso;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    ai_usage_spend::{
        ccusage_cli_reports::{BillingBlock, Period, RecentUsage, Usage},
        claude_usage::{
            ClaudeCcusageBlock, ClaudeCcusageModels, ClaudeCcusagePeriods, ClaudeUsageWindows,
            ExtraStatus, ExtraUsage, WindowUsage,
        },
        model_spend_ranking::{ModelRanking, ModelSpend},
        spend_money::{Money, usd},
        working_time_forecast::{CalendarSpan, SpendBudget, SpendPacing},
    },
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::claude_usage_group::{ClaudeUsageGroup, ClaudeUsageMessage, ClaudeUsageTarget},
};
use uom::si::{f64::Ratio, ratio::percent};

use crate::{fixtures::sample, showcase_message::ShowcaseMessage, showcased_group::ShowcasedGroup};

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = ClaudeUsageGroup::new(None);
    [
        usage_windows_delivered(0, clock),
        ClaudeUsageMessage::CcusagePeriods(SnapshotDelivery::live_push(Ok(local_log_periods(
            clock,
        )))),
        ClaudeUsageMessage::CcusageBlock(SnapshotDelivery::live_push(Ok(active_billing_block(
            clock,
        )))),
        ClaudeUsageMessage::CcusageModels(SnapshotDelivery::live_push(Ok(
            subscription_model_ranking(),
        ))),
    ]
    .into_iter()
    .for_each(|reading| {
        let _ = group.update(reading, clock);
    });
    ShowcasedGroup::ClaudeUsage(Box::new(group))
}

pub fn live(clock: Clock) -> Vec<ShowcaseMessage> {
    let tick = sample(clock);
    vec![ShowcaseMessage::ClaudeUsage(usage_windows_delivered(
        tick, clock,
    ))]
}

fn usage_windows_delivered(tick: u32, clock: Clock) -> ClaudeUsageMessage {
    ClaudeUsageMessage::UsageWindows(Box::new(SnapshotDelivery::fixture_delivered(
        Ok(usage_windows(tick, clock)),
        DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_mins(1))),
    )))
}

fn window(utilization_percent: f64, resets_in: TimeDelta, clock: Clock) -> WindowUsage {
    WindowUsage {
        utilization: Ratio::new::<percent>(utilization_percent),
        resets_at: Some(clock.utc() + resets_in),
    }
}

fn euro_cents(cents: u32) -> Money {
    Money::from_decimal(Decimal::new(i64::from(cents), 2), iso::EUR)
}

fn usage_windows(tick: u32, clock: Clock) -> ClaudeUsageWindows {
    let elapsed_ticks = f64::from(tick);
    let spent_cents = 1981 + 37 * tick;
    let limit_cents = 3000;
    ClaudeUsageWindows {
        plan: Some("Max".to_owned()),
        session: window(42.0 + 2.0 * elapsed_ticks, TimeDelta::minutes(95), clock),
        weekly: window(31.0 + 0.5 * elapsed_ticks, TimeDelta::days(3), clock),
        sonnet: Some(window(
            12.0 + 0.5 * elapsed_ticks,
            TimeDelta::days(3),
            clock,
        )),
        extra: Some(ExtraUsage {
            status: ExtraStatus::Enabled,
            spent: Some(euro_cents(spent_cents)),
            monthly_limit: Some(euro_cents(limit_cents)),
            used_share: Some(Ratio::new::<percent>(
                f64::from(spent_cents) / f64::from(limit_cents) * 100.0,
            )),
        }),
    }
}

pub fn on_hover(_target: ClaudeUsageTarget, _clock: Clock) -> Vec<ClaudeUsageMessage> {
    Vec::new()
}

const MONTHLY_BUDGET_USD: u32 = 200;
const OPUS_MODEL: &str = "claude-opus-4-5";
const SONNET_MODEL: &str = "claude-sonnet-4-5";
const HAIKU_MODEL: &str = "claude-haiku-4-5";

fn dollar_cents(cents: i64) -> Money {
    usd(Decimal::new(cents, 2))
}

fn ranked_models(model_cents: &[(&str, i64)]) -> ModelRanking {
    model_cents
        .iter()
        .map(|&(model, cents)| ModelSpend {
            model: model.to_owned(),
            cost: dollar_cents(cents),
        })
        .collect()
}

fn logged_period(span: CalendarSpan, tokens: u64, model_cents: &[(&str, i64)]) -> Period {
    let models = ranked_models(model_cents);
    Period {
        span,
        usage: Usage {
            tokens,
            cost: models.total(),
            models,
        },
    }
}

fn today_model_cents() -> [(&'static str, i64); 2] {
    [(OPUS_MODEL, 1_842), (SONNET_MODEL, 391)]
}

fn week_model_cents() -> [(&'static str, i64); 3] {
    [
        (OPUS_MODEL, 6_920),
        (SONNET_MODEL, 2_315),
        (HAIKU_MODEL, 148),
    ]
}

fn month_model_cents() -> [(&'static str, i64); 3] {
    [
        (OPUS_MODEL, 9_871),
        (SONNET_MODEL, 3_604),
        (HAIKU_MODEL, 262),
    ]
}

fn local_log_periods(clock: Clock) -> ClaudeCcusagePeriods {
    let today = clock.local().date_naive();
    ClaudeCcusagePeriods {
        recent: RecentUsage {
            today: logged_period(CalendarSpan::day(today), 4_812_000, &today_model_cents()),
            week: logged_period(
                CalendarSpan::week_containing(today),
                17_390_000,
                &week_model_cents(),
            ),
            month: logged_period(
                CalendarSpan::month_containing(today),
                26_045_000,
                &month_model_cents(),
            ),
        },
        budget: SpendBudget {
            pacing: SpendPacing::CalendarTime,
            monthly_limit_usd: MONTHLY_BUDGET_USD,
        },
    }
}

fn active_billing_block(clock: Clock) -> ClaudeCcusageBlock {
    ClaudeCcusageBlock {
        active: Some(BillingBlock {
            cost: dollar_cents(1_274),
            ends_at: clock.utc() + TimeDelta::minutes(95),
            cost_per_hour: Some(dollar_cents(397)),
            projected_cost: Some(dollar_cents(1_903)),
        }),
    }
}

fn subscription_model_ranking() -> ClaudeCcusageModels {
    ClaudeCcusageModels {
        covered_by_subscription: true,
        today: ranked_models(&today_model_cents()),
        week: ranked_models(&week_model_cents()),
        month: ranked_models(&month_model_cents()),
    }
}
