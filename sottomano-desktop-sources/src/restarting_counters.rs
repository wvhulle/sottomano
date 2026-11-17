use std::{fmt::Debug, time::Duration};

fn since_counter_restart<Total: Copy + Debug>(
    counter_name: &str,
    current_total: Total,
    earlier_total: Total,
    checked_difference: fn(Total, Total) -> Option<Total>,
) -> Total {
    if let Some(accumulated_since) = checked_difference(current_total, earlier_total) {
        accumulated_since
    } else {
        tracing::debug!(
            counter = counter_name,
            earlier = ?earlier_total,
            current = ?current_total,
            "counter restarted"
        );
        current_total
    }
}

#[must_use]
pub fn count_since_counter_restart(
    counter_name: &str,
    current_count: u64,
    earlier_count: u64,
) -> u64 {
    since_counter_restart(counter_name, current_count, earlier_count, u64::checked_sub)
}

#[must_use]
pub fn time_since_counter_restart(
    counter_name: &str,
    current_time: Duration,
    earlier_time: Duration,
) -> Duration {
    since_counter_restart(
        counter_name,
        current_time,
        earlier_time,
        Duration::checked_sub,
    )
}
