use chrono::{TimeDelta, Utc};
use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    config::SecretPaths,
    data::source::{
        Delivery,
        ai::claude::{ClaudeUsageWindows, ExtraStatus, ExtraUsage, Money, WindowUsage},
    },
    desktop_shell::Message,
    readme_showcase::showcased_group::ShowcasedGroup,
    widget_groups::ai_group::{AiGroup, AiMessage},
};

pub fn group() -> ShowcasedGroup {
    let mut group = AiGroup::new(SecretPaths::default());
    let _ = group.update(usage_windows_delivered(0));
    ShowcasedGroup::Ai(Box::new(group))
}

pub fn live(tick: u32) -> Vec<Message> {
    vec![Message::Ai(usage_windows_delivered(tick))]
}

fn usage_windows_delivered(tick: u32) -> AiMessage {
    AiMessage::ClaudeUsageWindows(Box::new(Delivery::Reading(Ok(usage_windows(f64::from(
        tick,
    ))))))
}

fn window(utilization_percent: f64, resets_in: TimeDelta) -> WindowUsage {
    WindowUsage {
        utilization: Ratio::new::<percent>(utilization_percent),
        resets_at: Some(Utc::now() + resets_in),
    }
}

fn euros(amount: f64) -> Money {
    Money {
        amount,
        currency: "EUR".to_owned(),
        decimal_places: 2,
    }
}

fn usage_windows(elapsed_ticks: f64) -> ClaudeUsageWindows {
    let spent = 19.81 + 0.37 * elapsed_ticks;
    ClaudeUsageWindows {
        plan: Some("Max".to_owned()),
        session: window(42.0 + 2.0 * elapsed_ticks, TimeDelta::minutes(95)),
        weekly: window(31.0 + 0.5 * elapsed_ticks, TimeDelta::days(3)),
        sonnet: Some(window(12.0 + 0.5 * elapsed_ticks, TimeDelta::days(3))),
        extra: Some(ExtraUsage {
            status: ExtraStatus::Enabled,
            spent: Some(euros(spent)),
            monthly_limit: Some(euros(30.0)),
            used_share: Some(Ratio::new::<percent>(spent / 30.0 * 100.0)),
        }),
    }
}
