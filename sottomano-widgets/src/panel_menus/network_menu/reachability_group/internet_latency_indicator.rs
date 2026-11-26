use std::time::Duration;

use sottomano_desktop_sources::{
    SnapshotSource, network_connectivity::icmp_internet_latency::InternetLatency,
};
use uom::si::{f64::Ratio, ratio::ratio};

use super::{ReachabilityBusRequest, ReachabilityGroup, ReachabilityInteraction};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_drawing::ReadoutGauge,
        indicator_group_content::IndicatorListing,
    },
    hover_cards::hover_card_body::{Section, SectionTitle},
    panel_menus::network_menu::echo_probe_window_rows::{
        probe_window_section, probe_window_severity,
    },
};

const INTERNET_LATENCY_GLYPH: Phosphor = Phosphor::Timer(Weight::Regular);

const SCALE_START: Duration = Duration::from_millis(5);
const SCALE_END: Duration = Duration::from_millis(500);

fn fraction_of_scale(round_trip: Duration) -> f64 {
    let log_width_of_scale = (SCALE_END.as_secs_f64() / SCALE_START.as_secs_f64()).ln();
    let log_position = (round_trip.max(SCALE_START).as_secs_f64() / SCALE_START.as_secs_f64()).ln();
    (log_position / log_width_of_scale).clamp(0.0, 1.0)
}

pub(super) fn internet_latency_hover_card(latency: &InternetLatency) -> Vec<Section> {
    vec![probe_window_section(
        SectionTitle::captioned(
            "Ping",
            "Echo round trips every five seconds to a host on the internet, through whichever \
             connection carries the traffic.",
        ),
        &latency.window,
    )]
}

pub(super) fn internet_latency_indicator(latency: &InternetLatency) -> GroupIndicator<'_> {
    let base = GroupIndicator::glyph_only(
        InternetLatency::NAME,
        Phosphor::Timer(Weight::Regular).into(),
        probe_window_severity(&latency.window).into(),
    )
    .with_hover_card(Box::new(move |_| internet_latency_hover_card(latency)));
    let gauge = match latency.window.median_round_trip() {
        Some(round_trip) => {
            let fill = Ratio::new::<ratio>(fraction_of_scale(round_trip));
            ReadoutGauge::Gauged { fill, live: fill }
        }
        None => ReadoutGauge::Ungauged,
    };
    base.with_gauge(gauge)
}

impl ReachabilityGroup {
    pub(super) fn internet_latency_listing(&self) -> IndicatorListing<'_> {
        self.latency.indicator_listing(
            |held_snapshot| {
                self.wired_internet_latency_indicator(internet_latency_indicator(held_snapshot))
                    .when_shown()
            },
            INTERNET_LATENCY_GLYPH.into(),
        )
    }

    fn wired_internet_latency_indicator<'view>(
        &'view self,
        latency_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        latency_indicator
            .with_hover_card(Box::new(move |_| {
                self.latency
                    .sections_described_by(internet_latency_hover_card)
            }))
            .refreshed_by(ReachabilityInteraction::OnBuses(
                ReachabilityBusRequest::Refresh,
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_is_logarithmic_between_its_ends() {
        assert!(fraction_of_scale(Duration::from_millis(1)).abs() < f64::EPSILON);
        assert!((fraction_of_scale(Duration::from_millis(50)) - 0.5).abs() < 1e-9);
        assert!((fraction_of_scale(Duration::from_secs(2)) - 1.0).abs() < f64::EPSILON);
    }
}
