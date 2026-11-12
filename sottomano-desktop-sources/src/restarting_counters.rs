#[must_use]
pub fn count_since_counter_restart(
    counter_name: &str,
    current_count: u64,
    earlier_count: u64,
) -> u64 {
    if let Some(counted_since) = current_count.checked_sub(earlier_count) {
        counted_since
    } else {
        tracing::debug!(
            "{counter_name}: counter restarted from {earlier_count} at {current_count}"
        );
        current_count
    }
}
