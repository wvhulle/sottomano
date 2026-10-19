use sottomano_config::severity::Severity;
use sottomano_sources::network_connectivity::{
    interface_throughput::{
        InterfaceThroughput, NetworkTrafficDirection, SCALE_END_MB_PER_S, Throughput,
    },
    network_manager_proxies::Connectivity,
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
        phosphor::{Phosphor, Weight},
        rate_text::per_second_text,
        tooltip_body::{KeyValue, TooltipBody},
        widget_state::WidgetState,
    },
};

const HI_RES_AUDIO_STREAM_KB_PER_S: f64 = 1_152.0;
const HI_RES_VIDEO_STREAM_KB_PER_S: f64 = 3_125.0;

fn severity(throughput: &Throughput) -> Severity {
    if throughput.interfaces.is_empty() {
        return Severity::Critical;
    }
    let link_level = if throughput.unused() {
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
    };
    connectivity_level(throughput.connectivity).max(link_level)
}

impl BarWidget for Throughput {
    fn name(&self) -> &'static str {
        match self.direction {
            NetworkTrafficDirection::Upload => "Upload",
            NetworkTrafficDirection::Download => "Download",
            NetworkTrafficDirection::Combined => "Network throughput",
        }
    }

    fn icon(&self) -> Glyph {
        let direction_icon = match self.direction {
            NetworkTrafficDirection::Upload => Phosphor::UploadSimple(Weight::Regular),
            NetworkTrafficDirection::Download => Phosphor::DownloadSimple(Weight::Regular),
            NetworkTrafficDirection::Combined => Phosphor::ArrowsDownUp(Weight::Regular),
        };
        direction_icon.with_weight(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        let severity = severity(self);
        if self.unused() && severity == Severity::Normal {
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
            return TooltipBody::dictionary([KeyValue::new("Connectivity", "none")]);
        }
        let connectivity = self
            .connectivity
            .map(|connectivity| KeyValue::new("Connectivity", connectivity.label()));
        let rate_label = match self.direction {
            NetworkTrafficDirection::Upload => "Up",
            NetworkTrafficDirection::Download => "Down",
            NetworkTrafficDirection::Combined => "Total",
        };
        TooltipBody::dictionary(
            [
                KeyValue::new(rate_label, per_second_text(self.rate)),
                KeyValue::new("Peak", per_second_text(self.peak_rate)),
                KeyValue::new("Interfaces", self.interfaces.join(", ")),
            ]
            .into_iter()
            .chain(connectivity)
            .chain([KeyValue::new(
                "Capacity",
                per_second_text(InformationRate::new::<megabyte_per_second>(
                    SCALE_END_MB_PER_S,
                )),
            )]),
        )
    }
}

impl BarWidget for InterfaceThroughput {
    fn name(&self) -> &'static str {
        self.total.name()
    }

    fn icon(&self) -> Glyph {
        self.total.icon()
    }

    fn visible_in_bar(&self) -> bool {
        false
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        self.total.tooltip(clock)
    }
}

fn connectivity_level(connectivity: Option<Connectivity>) -> Severity {
    match connectivity {
        Some(Connectivity::Full) => Severity::Normal,
        Some(Connectivity::Limited) | None => Severity::Warning,
        Some(Connectivity::Offline) => Severity::Critical,
    }
}
