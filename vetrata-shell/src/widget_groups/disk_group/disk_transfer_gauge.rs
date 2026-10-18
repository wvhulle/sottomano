use uom::si::{
    f64::{InformationRate, Ratio},
    information_rate::{gibibyte_per_second, kibibyte_per_second},
    ratio::{percent, ratio},
};
use vetrata_sources::disk::activity::{DiskActivity, DiskTransfer, DiskTransferDirection};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    phosphor::{Phosphor, Weight},
    rate_text::per_second_text,
    tooltip_body::{KeyValue, TooltipBody},
    widget_state::WidgetState,
};

const BACKGROUND_TRANSFER_KIB_PER_S: f64 = 64.0;
const SCALE_END_GIB_PER_S: f64 = 4.0;

fn background_rate() -> InformationRate {
    InformationRate::new::<kibibyte_per_second>(BACKGROUND_TRANSFER_KIB_PER_S)
}

impl BarWidget for DiskTransfer {
    fn name(&self) -> &'static str {
        match self.direction {
            DiskTransferDirection::Read => "Disk reads",
            DiskTransferDirection::Write => "Disk writes",
        }
    }

    fn icon(&self) -> Glyph {
        let direction_icon = match self.direction {
            DiskTransferDirection::Read => Phosphor::TrayArrowUp(Weight::Regular),
            DiskTransferDirection::Write => Phosphor::TrayArrowDown(Weight::Regular),
        };
        direction_icon.with_weight(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        if self.rate <= background_rate() {
            WidgetState::Off
        } else {
            WidgetState::Idle
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

    fn tooltip(&self) -> TooltipBody {
        let rate_label = match self.direction {
            DiskTransferDirection::Read => "Read",
            DiskTransferDirection::Write => "Write",
        };
        TooltipBody::dictionary([
            KeyValue::new("Disk", self.disk.clone()),
            KeyValue::new(rate_label, per_second_text(self.rate)),
            KeyValue::new(
                "Disk busy",
                format!("{:.0}%", self.disk_busy.get::<percent>()),
            ),
        ])
    }
}

impl BarWidget for DiskActivity {
    fn name(&self) -> &'static str {
        "Disk activity"
    }

    fn icon(&self) -> Glyph {
        self.read.icon()
    }

    fn visible_in_bar(&self) -> bool {
        false
    }

    fn tooltip(&self) -> TooltipBody {
        self.read.tooltip()
    }
}
