use uom::si::{
    f64::{InformationRate, Ratio},
    information_rate::{kilobyte_per_second, megabyte_per_second},
    ratio::ratio,
};

use crate::{
    data::{
        severity::Severity,
        source::{
            net_interfaces::nm::Connectivity,
            throughput::{Direction, Download, SCALE_END_MB_PER_S, Throughput, Total, Upload},
        },
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        rate_text::RateText,
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

const HI_RES_AUDIO_STREAM_KB_PER_S: f64 = 1_152.0;
const HI_RES_VIDEO_STREAM_KB_PER_S: f64 = 3_125.0;

pub trait DirectionIcon: Direction {
    const NAME: &'static str;
    const ICON: Phosphor;
}

impl DirectionIcon for Upload {
    const NAME: &'static str = "Upload";
    const ICON: Phosphor = Phosphor::UploadSimple(Weight::Regular);
}

impl DirectionIcon for Download {
    const NAME: &'static str = "Download";
    const ICON: Phosphor = Phosphor::DownloadSimple(Weight::Regular);
}

impl DirectionIcon for Total {
    const NAME: &'static str = "Network throughput";
    const ICON: Phosphor = Phosphor::ArrowsDownUp(Weight::Regular);
}

fn severity<D: Direction>(throughput: &Throughput<D>) -> Severity {
    if throughput.interfaces.is_empty() {
        return Severity::Critical;
    }
    connectivity_level(throughput.connectivity).max(throughput.link_level())
}

impl<D: DirectionIcon> BarWidget for Throughput<D> {
    fn name(&self) -> &'static str {
        D::NAME
    }

    fn icon(&self) -> Glyph {
        D::ICON.with_weight(self.state().weight()).into()
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
            return TooltipBody::sections(&[Section::untitled_dictionary([KeyValue::new(
                "Connectivity",
                "none",
            )])]);
        }
        let connectivity = self
            .connectivity
            .map(|connectivity| KeyValue::new("Connectivity", connectivity.label()));
        TooltipBody::sections(&[Section::untitled_dictionary(
            [
                KeyValue::new(D::LABEL, self.rate.per_second_text()),
                KeyValue::new("Peak", self.peak_rate.per_second_text()),
                KeyValue::new("Interfaces", self.interfaces.join(", ")),
            ]
            .into_iter()
            .chain(connectivity)
            .chain([KeyValue::new(
                "Capacity",
                InformationRate::new::<megabyte_per_second>(SCALE_END_MB_PER_S).per_second_text(),
            )]),
        )])
    }
}

impl<D: Direction> Throughput<D> {
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

fn connectivity_level(connectivity: Option<Connectivity>) -> Severity {
    match connectivity {
        Some(Connectivity::Full) => Severity::Normal,
        Some(Connectivity::Limited) | None => Severity::Warning,
        Some(Connectivity::Offline) => Severity::Critical,
    }
}
