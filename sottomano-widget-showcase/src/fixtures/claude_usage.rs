use std::time::Duration;

use chrono::TimeDelta;
use rust_decimal::Decimal;
use rusty_money::iso;
use sottomano_sources::{
    SnapshotDelivery,
    ai_usage_spend::{
        claude_usage::{ClaudeUsageWindows, ExtraStatus, ExtraUsage, WindowUsage},
        spend_money::Money,
    },
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::claude_usage_group::{ClaudeUsageGroup, ClaudeUsageMessage},
};
use uom::si::{f64::Ratio, ratio::percent};

use crate::{fixtures::sample, showcase_message::ShowcaseMessage, showcased_group::ShowcasedGroup};

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = ClaudeUsageGroup::new(None);
    let _ = group.update(usage_windows_delivered(0, clock), clock);
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
