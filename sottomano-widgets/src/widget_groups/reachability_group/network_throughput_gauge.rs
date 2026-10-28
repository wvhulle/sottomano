use sottomano_config::severity::Severity;
use sottomano_sources::network_connectivity::interface_throughput::{
    InterfaceThroughput, NetworkTrafficDirection, SCALE_END_MB_PER_S, Throughput,
};
use uom::si::{
    f64::{InformationRate, Ratio},
    information_rate::{kilobyte_per_second, megabyte_per_second},
    ratio::ratio,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::TooltipBody,
        value_cells::LabeledValue,
        widget_state::WidgetState,
    },
};

const HI_RES_AUDIO_STREAM_KB_PER_S: f64 = 1_152.0;
const HI_RES_VIDEO_STREAM_KB_PER_S: f64 = 3_125.0;

fn severity(throughput: &Throughput) -> Severity {
    if throughput.unused() {
        Severity::Normal
    } else if throughput.peak_rate
        < InformationRate::new::<kilobyte_per_second>(HI_RES_AUDIO_STREAM_KB_PER_S)
    {
        Severity::Critical
    } else if throughput.peak_rate
        < InformationRate::new::<kilobyte_per_second>(HI_RES_VIDEO_STREAM_KB_PER_S)
    {
        Severity::Warning
    } else {
        Severity::Normal
    }
}

impl BarWidget for Throughput {
    fn name(&self) -> &'static str {
        match self.direction {
            NetworkTrafficDirection::Upload => "Upload",
            NetworkTrafficDirection::Download => "Download",
        }
    }

    fn icon(&self) -> Glyph {
        let direction_icon = match self.direction {
            NetworkTrafficDirection::Upload => Phosphor::UploadSimple(Weight::Regular),
            NetworkTrafficDirection::Download => Phosphor::DownloadSimple(Weight::Regular),
        };
        direction_icon.with_weight(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        let severity = severity(self);
        if self.interfaces.is_empty() || (self.unused() && severity == Severity::Normal) {
            WidgetState::Off
        } else {
            severity.into()
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(Ratio::new::<ratio>(f64::from(self.peak_fraction())))
    }

    fn live_gauge(&self) -> Option<Ratio> {
        Some(Ratio::new::<ratio>(f64::from(self.current_fraction())))
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        if self.interfaces.is_empty() {
            return TooltipBody::dictionary([LabeledValue::new("Interfaces", "none")]);
        }
        let rate_label = match self.direction {
            NetworkTrafficDirection::Upload => "Up",
            NetworkTrafficDirection::Download => "Down",
        };
        TooltipBody::dictionary(
            [
                LabeledValue::new(rate_label, Quantity::transfer_rate(self.rate)),
                LabeledValue::new("Peak", Quantity::transfer_rate(self.peak_rate)),
                LabeledValue::new("Interfaces", self.interfaces.join(", ")),
            ]
            .into_iter()
            .chain([LabeledValue::new(
                "Capacity",
                Quantity::transfer_rate(InformationRate::new::<megabyte_per_second>(
                    SCALE_END_MB_PER_S,
                )),
            )]),
        )
    }
}

impl BarWidget for InterfaceThroughput {
    fn name(&self) -> &'static str {
        "Network throughput"
    }

    fn icon(&self) -> Glyph {
        Phosphor::ArrowsDownUp(Weight::Regular).into()
    }

    fn visible_in_bar(&self) -> bool {
        false
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([
            LabeledValue::new("Down", Quantity::transfer_rate(self.down.rate)),
            LabeledValue::new("Up", Quantity::transfer_rate(self.up.rate)),
        ])
    }
}
