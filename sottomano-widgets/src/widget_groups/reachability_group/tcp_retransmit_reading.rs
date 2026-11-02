use sottomano_config::severity::Severity;
use sottomano_sources::network_connectivity::proc_tcp_retransmissions::TcpRetransmissions;
use uom::si::ratio::percent;

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        hover_card_body::HoverCardBody,
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        reading_display_state::ReadingDisplayState,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub(super) const TCP_RETRANSMISSIONS_ITEM_NAME: &str = "TCP retransmissions";

const NOTICEABLE_SHARE_PERCENT: f64 = 0.5;
const WARNING_SHARE_PERCENT: f64 = 2.0;
const CRITICAL_SHARE_PERCENT: f64 = 5.0;
const IDLE_SEGMENTS_PER_SECOND: f64 = 5.0;

fn severity(retransmissions: &TcpRetransmissions) -> Severity {
    let share_percent = retransmissions.retransmitted_share.get::<percent>();
    if retransmissions.sent_per_second < IDLE_SEGMENTS_PER_SECOND
        || share_percent < NOTICEABLE_SHARE_PERCENT
    {
        Severity::Low
    } else if share_percent >= CRITICAL_SHARE_PERCENT {
        Severity::Critical
    } else if share_percent >= WARNING_SHARE_PERCENT {
        Severity::Warning
    } else {
        Severity::Normal
    }
}

impl GroupItemReading for TcpRetransmissions {
    fn name(&self) -> &'static str {
        TCP_RETRANSMISSIONS_ITEM_NAME
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::ArrowCounterClockwise(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        severity(self).into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new(
                "Retransmitted",
                Quantity::share(self.retransmitted_share).decimals(1),
            )
            .level(severity(self)),
            LabeledValue::new(
                "Retransmitted segments",
                Quantity::per_second(self.retransmitted_per_second),
            ),
            LabeledValue::new("Sent segments", Quantity::per_second(self.sent_per_second)),
            LabeledValue::new("Measured over", ValueCell::duration(self.window)),
        ])
    }
}
