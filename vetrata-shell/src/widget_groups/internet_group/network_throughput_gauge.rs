use uom::si::{
    f64::{InformationRate, Ratio},
    information_rate::{kilobyte_per_second, megabyte_per_second},
    ratio::ratio,
};
use vetrata_config::severity::Severity;

use crate::{
    data::source::{
        net_interfaces::nm::Connectivity,
        throughput::{NetworkTrafficDirection, Readings, SCALE_END_MB_PER_S, Throughput},
    },
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

impl NetworkTrafficDirection {
    const fn widget_name(self) -> &'static str {
        match self {
            NetworkTrafficDirection::Upload => "Upload",
            NetworkTrafficDirection::Download => "Download",
            NetworkTrafficDirection::Combined => "Network throughput",
        }
    }

    const fn rate_label(self) -> &'static str {
        match self {
            NetworkTrafficDirection::Upload => "Up",
            NetworkTrafficDirection::Download => "Down",
            NetworkTrafficDirection::Combined => "Total",
        }
    }

    const fn direction_icon(self) -> Phosphor {
        match self {
            NetworkTrafficDirection::Upload => Phosphor::UploadSimple(Weight::Regular),
            NetworkTrafficDirection::Download => Phosphor::DownloadSimple(Weight::Regular),
            NetworkTrafficDirection::Combined => Phosphor::ArrowsDownUp(Weight::Regular),
        }
    }
}

fn severity(throughput: &Throughput) -> Severity {
    if throughput.interfaces.is_empty() {
        return Severity::Critical;
    }
    connectivity_level(throughput.connectivity).max(throughput.link_level())
}

impl BarWidget for Throughput {
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

    fn tooltip(&self) -> TooltipBody {
        if self.interfaces.is_empty() {
            return TooltipBody::dictionary([KeyValue::new("Connectivity", "none")]);
        }
        let connectivity = self
            .connectivity
            .map(|connectivity| KeyValue::new("Connectivity", connectivity.label()));
        TooltipBody::dictionary(
            [
                KeyValue::new(self.direction.rate_label(), per_second_text(self.rate)),
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

impl Throughput {
    fn link_level(&self) -> Severity {
        if self.unused() {
            return Severity::Normal;
        }
        if self.peak_rate
            < InformationRate::new::<kilobyte_per_second>(HI_RES_AUDIO_STREAM_KB_PER_S)
        {
            Severity::Critical
        } else if self.peak_rate
            < InformationRate::new::<kilobyte_per_second>(HI_RES_VIDEO_STREAM_KB_PER_S)
        {
            Severity::Warning
        } else {
            Severity::Normal
        }
    }
}

impl BarWidget for Readings {
    fn name(&self) -> &'static str {
        self.total.name()
    }

    fn icon(&self) -> Glyph {
        self.total.icon()
    }

    fn visible_in_bar(&self) -> bool {
        false
    }

    fn tooltip(&self) -> TooltipBody {
        self.total.tooltip()
    }
}

fn connectivity_level(connectivity: Option<Connectivity>) -> Severity {
    match connectivity {
        Some(Connectivity::Full) => Severity::Normal,
        Some(Connectivity::Limited) | None => Severity::Warning,
        Some(Connectivity::Offline) => Severity::Critical,
    }
}
