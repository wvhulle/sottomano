use std::time::Duration;

use chrono::Locale;
use sottomano_desktop_sources::{
    SnapshotSource, network_connectivity::icmp_internet_latency::InternetLatency,
};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    clock::Clock,
    group_items::{
        reading_row::ReadingRow,
        source_reading::{ReadoutGauge, RowReadout},
    },
    hover_cards::hover_card_body::{Section, SectionTitle},
    widget_groups::echo_probe_window_rows::{probe_window_section, probe_window_severity},
    widgets::phosphor::{Phosphor, Weight},
};

const SCALE_START: Duration = Duration::from_millis(5);
const SCALE_END: Duration = Duration::from_millis(500);

fn fraction_of_scale(round_trip: Duration) -> f64 {
    let log_width_of_scale = (SCALE_END.as_secs_f64() / SCALE_START.as_secs_f64()).ln();
    let log_position = (round_trip.max(SCALE_START).as_secs_f64() / SCALE_START.as_secs_f64()).ln();
    (log_position / log_width_of_scale).clamp(0.0, 1.0)
}

pub(super) fn internet_latency_hover_card(
    latency: &InternetLatency,
    _clock: Clock,
) -> Vec<Section> {
    vec![probe_window_section(
        SectionTitle::captioned(
            "Ping",
            "Echo round trips every five seconds to a host on the internet, through whichever \
             connection carries the traffic.",
        ),
        &latency.window,
    )]
}

pub(super) fn internet_latency_reading_row(
    latency: &InternetLatency,
    _clock: Clock,
    _locale: Locale,
) -> ReadingRow<'_> {
    let base = ReadingRow::glyph_only(
        InternetLatency::NAME,
        Phosphor::Timer(Weight::Regular).into(),
        probe_window_severity(&latency.window).into(),
        Box::new(move |card_clock| internet_latency_hover_card(latency, card_clock)),
    );
    let gauge = match latency.window.median_round_trip() {
        Some(round_trip) => {
            let fill = Ratio::new::<ratio>(fraction_of_scale(round_trip));
            ReadoutGauge::Gauged { fill, live: fill }
        }
        None => ReadoutGauge::Ungauged,
    };
    ReadingRow {
        readout: RowReadout {
            gauge,
            ..base.readout
        },
        ..base
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
