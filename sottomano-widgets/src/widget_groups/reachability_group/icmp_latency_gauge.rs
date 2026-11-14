use std::{borrow::Cow, time::Duration};

use sottomano_desktop_sources::{
    SnapshotSource, network_connectivity::icmp_internet_latency::InternetLatency,
};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::HoverCardBody,
    widget_groups::echo_probe_window_rows::{probe_window_section, probe_window_severity},
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
};

const SCALE_START: Duration = Duration::from_millis(5);
const SCALE_END: Duration = Duration::from_millis(500);

fn fraction_of_scale(round_trip: Duration) -> f64 {
    let log_width_of_scale = (SCALE_END.as_secs_f64() / SCALE_START.as_secs_f64()).ln();
    let log_position = (round_trip.max(SCALE_START).as_secs_f64() / SCALE_START.as_secs_f64()).ln();
    (log_position / log_width_of_scale).clamp(0.0, 1.0)
}

impl GroupItemReading for InternetLatency {
    fn name(&self) -> Cow<'static, str> {
        Self::NAME.into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Timer(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        probe_window_severity(&self.window).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        let round_trip = self.window.median_round_trip()?;
        Some(Ratio::new::<ratio>(fraction_of_scale(round_trip)))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        probe_window_section(&self.window).into()
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
