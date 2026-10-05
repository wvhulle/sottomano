use uom::si::{
    f64::{InformationRate, Ratio},
    information_rate::{gibibyte_per_second, kibibyte_per_second},
    ratio::{percent, ratio},
};

use crate::{
    data::source::disk::activity::{DiskTransfer, Reading, TransferDirection, Writing},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        rate_text::RateText,
        tooltip_body::{TooltipBody, row},
        widget_state::WidgetState,
    },
};

const BACKGROUND_TRANSFER_KIB_PER_S: f64 = 64.0;
const SCALE_END_GIB_PER_S: f64 = 4.0;

pub trait TransferIcon: TransferDirection {
    const ICON: Phosphor;
}

impl TransferIcon for Reading {
    const ICON: Phosphor = Phosphor::FileArrowUp(Weight::Regular);
}

impl TransferIcon for Writing {
    const ICON: Phosphor = Phosphor::FileArrowDown(Weight::Regular);
}

fn background_rate() -> InformationRate {
    InformationRate::new::<kibibyte_per_second>(BACKGROUND_TRANSFER_KIB_PER_S)
}

impl<D: TransferDirection> DiskTransfer<D> {
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

impl<D: TransferIcon> BarWidget for DiskTransfer<D> {
    fn icon(&self) -> Glyph {
        D::ICON.with_weight(self.state().weight()).into()
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
        TooltipBody::rows(vec![
            row("Disk", self.disk.clone()),
            row(D::LABEL, self.rate.per_second_text()),
            row(
                "Disk busy",
                format!("{:.0}%", self.disk_busy.get::<percent>()),
            ),
        ])
    }
}
