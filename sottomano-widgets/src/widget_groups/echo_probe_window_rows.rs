use std::time::Duration;

use sottomano_config::severity::Severity;
use sottomano_sources::network_connectivity::icmp_echo_probes::{
    LatencyProbeError, ProbeOutcome, ProbeWindow,
};
use uom::si::ratio::percent;

use crate::widgets::{
    localized_quantities::Quantity, tooltip_body::Section, value_cells::LabeledValue,
};

const WARNING_ROUND_TRIP: Duration = Duration::from_millis(100);
const CRITICAL_ROUND_TRIP: Duration = Duration::from_millis(300);
const WARNING_LOSS_PERCENT: f64 = 5.0;
const CRITICAL_LOSS_PERCENT: f64 = 20.0;

impl From<LatencyProbeError> for Section {
    fn from(error: LatencyProbeError) -> Self {
        Section::error(error)
    }
}

#[must_use]
pub fn probe_window_severity(window: &ProbeWindow) -> Severity {
    let loss_percent = window.loss_share().get::<percent>();
    let round_trip_level = match window.median_round_trip() {
        Some(round_trip) if round_trip >= CRITICAL_ROUND_TRIP => Severity::Critical,
        Some(round_trip) if round_trip >= WARNING_ROUND_TRIP => Severity::Warning,
        Some(_) => Severity::Normal,
        None if window.outcomes.is_empty() => Severity::Low,
        None => Severity::Critical,
    };
    let loss_level = if loss_percent >= CRITICAL_LOSS_PERCENT {
        Severity::Critical
    } else if loss_percent >= WARNING_LOSS_PERCENT {
        Severity::Warning
    } else {
        Severity::Low
    };
    round_trip_level.max(loss_level)
}

#[must_use]
pub fn probe_window_section(window: &ProbeWindow) -> Section {
    let latest = match window.latest_outcome() {
        Some(ProbeOutcome::Answered(round_trip)) => Some(LabeledValue::new(
            "Latest",
            Quantity::round_trip(round_trip),
        )),
        Some(ProbeOutcome::Lost) => Some(LabeledValue::new("Latest", "no reply")),
        None => None,
    };
    let median = window
        .median_round_trip()
        .map(|round_trip| LabeledValue::new("Median", Quantity::round_trip(round_trip)));
    Section::dictionary(
        [LabeledValue::new("Host", window.host.to_string())]
            .into_iter()
            .chain(latest)
            .chain(median)
            .chain([LabeledValue::new(
                format!("Loss of last {}", window.outcomes.len()),
                Quantity::share(window.loss_share()),
            )
            .level(probe_window_severity(window))]),
    )
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::*;

    fn window_of(outcomes: &[ProbeOutcome]) -> ProbeWindow {
        outcomes.iter().fold(
            ProbeWindow::empty(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            |window, outcome| window.recorded(*outcome),
        )
    }

    #[test]
    fn fast_answers_without_loss_are_quiet() {
        let window = window_of(&[ProbeOutcome::Answered(Duration::from_millis(12)); 10]);
        assert_eq!(probe_window_severity(&window), Severity::Normal);
    }

    #[test]
    fn heavy_loss_is_critical_even_when_answers_are_fast() {
        let window = window_of(&[
            ProbeOutcome::Answered(Duration::from_millis(12)),
            ProbeOutcome::Lost,
            ProbeOutcome::Lost,
            ProbeOutcome::Answered(Duration::from_millis(12)),
        ]);
        assert_eq!(probe_window_severity(&window), Severity::Critical);
    }

    #[test]
    fn no_answers_at_all_is_critical() {
        let window = window_of(&[ProbeOutcome::Lost; 3]);
        assert_eq!(probe_window_severity(&window), Severity::Critical);
    }
}
