use chrono::{TimeDelta, Utc};
use rust_decimal::Decimal;
use rusty_money::iso;
use sottomano_sources::{
    SourceDelivery,
    ai::claude::{ClaudeUsageWindows, ExtraStatus, ExtraUsage, WindowUsage},
    money::Money,
};
use sottomano_widgets::{
    shell_message::Message,
    widget_groups::claude_usage_group::{ClaudeUsageGroup, ClaudeUsageMessage},
};
use uom::si::{f64::Ratio, ratio::percent};

use crate::showcased_group::ShowcasedGroup;

pub fn group() -> ShowcasedGroup {
    let mut group = ClaudeUsageGroup::new(None);
    let _ = group.update(usage_windows_delivered(0));
    ShowcasedGroup::ClaudeUsage(Box::new(group))
}

pub fn live(tick: u32) -> Vec<Message> {
    vec![Message::ClaudeUsage(usage_windows_delivered(tick))]
}

fn usage_windows_delivered(tick: u32) -> ClaudeUsageMessage {
    ClaudeUsageMessage::UsageWindows(Box::new(SourceDelivery::Reading(Ok(usage_windows(tick)))))
}

fn window(utilization_percent: f64, resets_in: TimeDelta) -> WindowUsage {
    WindowUsage {
        utilization: Ratio::new::<percent>(utilization_percent),
        resets_at: Some(Utc::now() + resets_in),
    }
}

fn euro_cents(cents: u32) -> Money {
    Money::from_decimal(Decimal::new(i64::from(cents), 2), iso::EUR)
}

fn usage_windows(tick: u32) -> ClaudeUsageWindows {
    let elapsed_ticks = f64::from(tick);
    let spent_cents = 1981 + 37 * tick;
    let limit_cents = 3000;
    ClaudeUsageWindows {
        plan: Some("Max".to_owned()),
        session: window(42.0 + 2.0 * elapsed_ticks, TimeDelta::minutes(95)),
        weekly: window(31.0 + 0.5 * elapsed_ticks, TimeDelta::days(3)),
        sonnet: Some(window(12.0 + 0.5 * elapsed_ticks, TimeDelta::days(3))),
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
