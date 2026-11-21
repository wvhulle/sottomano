use futures::executor::block_on;

use super::*;
use crate::delivery_cadence_drivers::DeliveryCadence;

fn sample_critical_since(critical_since: CriticalOnset) -> Temperature {
    Temperature {
        samples: Vec::new(),
        throttle_limit: ThrottleLimit::Unreported,
        overrides: ThresholdOverrides::default(),
        critical_since,
    }
}

#[test]
fn holding_earliest_critical_keeps_the_first_stamp_until_a_gap() {
    let first = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let second = first + Duration::from_secs(5);
    let third = first + Duration::from_secs(15);
    let deliveries = stream::iter(
        [
            CriticalOnset::CriticalSince(first),
            CriticalOnset::CriticalSince(second),
            CriticalOnset::BelowCritical,
            CriticalOnset::CriticalSince(third),
        ]
        .map(|since| {
            SnapshotDelivery::fixture_delivered(
                Ok(sample_critical_since(since)),
                DeliveryCadence::PeriodicPolling(TEMPERATURE_POLLING),
            )
        }),
    );
    let held: Vec<CriticalOnset> = block_on(
        holding_earliest_critical(deliveries)
            .map(|delivery| match delivery {
                SnapshotDelivery::SnapshotDelivered {
                    snapshot_fetch: Ok(sample),
                    ..
                } => sample.critical_since,
                SnapshotDelivery::SnapshotDelivered {
                    snapshot_fetch: Err(_),
                    ..
                }
                | SnapshotDelivery::SourceUnavailable
                | SnapshotDelivery::SnapshotUnchanged => CriticalOnset::BelowCritical,
            })
            .collect(),
    );
    assert_eq!(
        held,
        [
            CriticalOnset::CriticalSince(first),
            CriticalOnset::CriticalSince(first),
            CriticalOnset::BelowCritical,
            CriticalOnset::CriticalSince(third),
        ]
    );
}

#[test]
fn reported_limit_sets_thresholds_below_throttling() {
    let thresholds = cpu_thresholds(
        ThrottleLimit::ThrottlesAt(celsius(100.0)),
        ThresholdOverrides::default(),
    );
    assert_eq!(
        thresholds.basis,
        ThresholdBasis::BelowThrottle(celsius(100.0))
    );
    assert_eq!(thresholds.level(celsius(79.0)), Severity::Normal);
    assert_eq!(thresholds.level(celsius(81.0)), Severity::Warning);
    assert_eq!(thresholds.level(celsius(96.0)), Severity::Critical);
}

#[test]
fn unreported_limit_falls_back_to_fixed_thresholds() {
    let thresholds = cpu_thresholds(ThrottleLimit::Unreported, ThresholdOverrides::default());
    assert_eq!(thresholds.basis, ThresholdBasis::Assumed);
    assert_eq!(thresholds.level(celsius(69.0)), Severity::Normal);
    assert_eq!(thresholds.level(celsius(71.0)), Severity::Warning);
    assert_eq!(thresholds.level(celsius(86.0)), Severity::Critical);
}
