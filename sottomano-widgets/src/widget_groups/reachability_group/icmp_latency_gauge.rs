use std::time::Duration;

use sottomano_config::severity::Severity;
use sottomano_sources::network_connectivity::icmp_uplink_latency::{
    GatewayProbe, LatencyProbeError, ProbeOutcome, ProbeWindow, UplinkLatency,
};
use uom::si::{
    f64::Ratio,
    ratio::{percent, ratio},
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

const SCALE_START: Duration = Duration::from_millis(5);
const SCALE_END: Duration = Duration::from_millis(500);
const WARNING_ROUND_TRIP: Duration = Duration::from_millis(100);
const CRITICAL_ROUND_TRIP: Duration = Duration::from_millis(300);
const WARNING_LOSS_PERCENT: f64 = 5.0;
const CRITICAL_LOSS_PERCENT: f64 = 20.0;

impl From<LatencyProbeError> for Section {
    fn from(error: LatencyProbeError) -> Self {
        Section::error(error)
    }
}

fn window_severity(window: &ProbeWindow) -> Severity {
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

fn severity(latency: &UplinkLatency) -> Severity {
    let gateway_level = match &latency.gateway {
        GatewayProbe::Probed(window) => window_severity(window),
        GatewayProbe::NoGateway => Severity::Low,
    };
    window_severity(&latency.internet).max(gateway_level)
}

fn fraction_of_scale(round_trip: Duration) -> f64 {
    let log_width_of_scale = (SCALE_END.as_secs_f64() / SCALE_START.as_secs_f64()).ln();
    let log_position = (round_trip.max(SCALE_START).as_secs_f64() / SCALE_START.as_secs_f64()).ln();
    (log_position / log_width_of_scale).clamp(0.0, 1.0)
}

fn window_entries(window: &ProbeWindow) -> Vec<KeyValue> {
    let latest = match window.latest_outcome() {
        Some(ProbeOutcome::Answered(round_trip)) => {
            Some(KeyValue::new("Latest", Quantity::round_trip(round_trip)))
        }
        Some(ProbeOutcome::Lost) => Some(KeyValue::new("Latest", "no reply")),
        None => None,
    };
    let median = window
        .median_round_trip()
        .map(|round_trip| KeyValue::new("Median", Quantity::round_trip(round_trip)));
    [KeyValue::new("Host", window.host.to_string())]
        .into_iter()
        .chain(latest)
        .chain(median)
        .chain([KeyValue::new(
            format!("Loss of last {}", window.outcomes.len()),
            Quantity::share(window.loss_share()),
        )
        .level(window_severity(window))])
        .collect()
}

impl BarWidget for UplinkLatency {
    fn name(&self) -> &'static str {
        "Latency"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Timer(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        severity(self).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        let round_trip = self.internet.median_round_trip()?;
        Some(Ratio::new::<ratio>(fraction_of_scale(round_trip)))
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let gateway_section = match &self.gateway {
            GatewayProbe::Probed(window) => Section::dictionary(window_entries(window)),
            GatewayProbe::NoGateway => Section::dictionary([KeyValue::new("Host", "none")]),
        };
        TooltipBody::sections([
            gateway_section.titled("Gateway (Router)"),
            Section::dictionary(window_entries(&self.internet)).titled("Internet"),
        ])
    }
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
    fn scale_is_logarithmic_between_its_ends() {
        assert!(fraction_of_scale(Duration::from_millis(1)).abs() < f64::EPSILON);
        assert!((fraction_of_scale(Duration::from_millis(50)) - 0.5).abs() < 1e-9);
        assert!((fraction_of_scale(Duration::from_secs(2)) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fast_answers_without_loss_are_quiet() {
        let window = window_of(&[ProbeOutcome::Answered(Duration::from_millis(12)); 10]);
        assert_eq!(window_severity(&window), Severity::Normal);
    }

    #[test]
    fn heavy_loss_is_critical_even_when_answers_are_fast() {
        let window = window_of(&[
            ProbeOutcome::Answered(Duration::from_millis(12)),
            ProbeOutcome::Lost,
            ProbeOutcome::Lost,
            ProbeOutcome::Answered(Duration::from_millis(12)),
        ]);
        assert_eq!(window_severity(&window), Severity::Critical);
    }

    #[test]
    fn no_answers_at_all_is_critical() {
        let window = window_of(&[ProbeOutcome::Lost; 3]);
        assert_eq!(window_severity(&window), Severity::Critical);
    }
}
