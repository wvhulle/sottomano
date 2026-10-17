use uom::si::{
    f64::{InformationRate, Ratio},
    information_rate::{gibibyte_per_second, kibibyte_per_second},
    ratio::{percent, ratio},
};

use crate::{
    data::source::disk::activity::{DiskActivity, DiskTransfer, DiskTransferDirection},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        rate_text::per_second_text,
        tooltip_body::{KeyValue, TooltipBody},
        widget_state::WidgetState,
    },
};

const BACKGROUND_TRANSFER_KIB_PER_S: f64 = 64.0;
const SCALE_END_GIB_PER_S: f64 = 4.0;

impl DiskTransferDirection {
    const fn widget_name(self) -> &'static str {
        match self {
            DiskTransferDirection::Read => "Disk reads",
            DiskTransferDirection::Write => "Disk writes",
        }
    }

    const fn rate_label(self) -> &'static str {
        match self {
            DiskTransferDirection::Read => "Read",
            DiskTransferDirection::Write => "Write",
        }
    }

    const fn direction_icon(self) -> Phosphor {
        match self {
            DiskTransferDirection::Read => Phosphor::TrayArrowUp(Weight::Regular),
            DiskTransferDirection::Write => Phosphor::TrayArrowDown(Weight::Regular),
        }
    }
}

fn background_rate() -> InformationRate {
    InformationRate::new::<kibibyte_per_second>(BACKGROUND_TRANSFER_KIB_PER_S)
}

impl DiskTransfer {
    fn quiet(&self) -> bool {
        self.rate <= background_rate()
    }

    fn fraction_of_scale(&self) -> Ratio {
        let scale_start = background_rate();
        let scale_end = InformationRate::new::<gibibyte_per_second>(SCALE_END_GIB_PER_S);
        if self.rate <= scale_start {
            return Ratio::new::<ratio>(0.0);
        }
        let scale_width = (scale_end / scale_start).get::<ratio>().ln();
        let position = (self.rate / scale_start).get::<ratio>().ln();
        Ratio::new::<ratio>((position / scale_width).clamp(0.0, 1.0))
    }
}

impl BarWidget for DiskTransfer {
    fn name(&self) -> &'static str {
        self.direction.widget_name()
    }

    fn icon(&self) -> Glyph {
        self.direction
            .direction_icon()
            .with_weight(self.state().weight())
            .into()
    }

    fn state(&self) -> WidgetState {
        if self.quiet() {
            WidgetState::Off
        } else {
            WidgetState::Idle
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.fraction_of_scale())
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::dictionary([
            KeyValue::new("Disk", self.disk.clone()),
            KeyValue::new(self.direction.rate_label(), per_second_text(self.rate)),
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
