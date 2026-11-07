use sottomano_desktop_sources::disk_storage::disk_io_activity::{
    DiskActivity, DiskTransfer, DiskTransferDirection,
};
use uom::si::{
    f64::{InformationRate, Ratio},
    information_rate::{gibibyte_per_second, kibibyte_per_second},
    ratio::ratio,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::{GroupItemReading, PanelVisibility},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const BACKGROUND_TRANSFER_KIB_PER_S: f64 = 64.0;
const SCALE_END_GIB_PER_S: f64 = 4.0;

fn background_rate() -> InformationRate {
    InformationRate::new::<kibibyte_per_second>(BACKGROUND_TRANSFER_KIB_PER_S)
}

impl GroupItemReading for DiskTransfer {
    fn name(&self) -> &'static str {
        match self.direction {
            DiskTransferDirection::Read => "Disk reads",
            DiskTransferDirection::Write => "Disk writes",
        }
    }

    fn icon(&self) -> IconGlyph {
        let direction_icon = match self.direction {
            DiskTransferDirection::Read => Phosphor::TrayArrowUp(Weight::Regular),
            DiskTransferDirection::Write => Phosphor::TrayArrowDown(Weight::Regular),
        };
        direction_icon.with_weight(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.rate <= background_rate() {
            ReadingDisplayState::Off
        } else {
            ReadingDisplayState::Idle
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        let scale_start = background_rate();
        let scale_end = InformationRate::new::<gibibyte_per_second>(SCALE_END_GIB_PER_S);
        if self.rate <= scale_start {
            return Some(Ratio::new::<ratio>(0.0));
        }
        let scale_width = (scale_end / scale_start).get::<ratio>().ln();
        let position = (self.rate / scale_start).get::<ratio>().ln();
        Some(Ratio::new::<ratio>(
            (position / scale_width).clamp(0.0, 1.0),
        ))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let rate_label = match self.direction {
            DiskTransferDirection::Read => "Read",
            DiskTransferDirection::Write => "Write",
        };
        HoverCardBody::dictionary([
            LabeledValue::new("Disk", self.disk.clone()),
            LabeledValue::new(rate_label, Quantity::transfer_rate(self.rate)),
            LabeledValue::new("Disk busy", Quantity::share(self.disk_busy)),
        ])
    }
}

impl GroupItemReading for DiskActivity {
    fn name(&self) -> &'static str {
        "Disk activity"
    }

    fn icon(&self) -> IconGlyph {
        self.read.icon()
    }

    fn panel_visibility(&self) -> PanelVisibility {
        PanelVisibility::Hidden
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        self.read.hover_card(clock)
    }
}
