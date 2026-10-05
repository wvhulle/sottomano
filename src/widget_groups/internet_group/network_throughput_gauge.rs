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
        tooltip_body::{TooltipBody, row},
        widget_state::WidgetState,
    },
};

const HI_RES_AUDIO_STREAM_KB_PER_S: f64 = 1_152.0;
const HI_RES_VIDEO_STREAM_KB_PER_S: f64 = 3_125.0;

pub trait DirectionIcon: Direction {
    const ICON: Phosphor;
}

impl DirectionIcon for Upload {
    const ICON: Phosphor = Phosphor::UploadSimple(Weight::Regular);
}

impl DirectionIcon for Download {
    const ICON: Phosphor = Phosphor::DownloadSimple(Weight::Regular);
}

impl DirectionIcon for Total {
    const ICON: Phosphor = Phosphor::ArrowsDownUp(Weight::Regular);
}

fn severity<D: Direction>(throughput: &Throughput<D>) -> Severity {
    if throughput.interfaces.is_empty() {
        return Severity::Critical;
    }
    connectivity_level(throughput.connectivity).max(throughput.link_level())
}

impl<D: DirectionIcon> BarWidget for Throughput<D> {
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
            return TooltipBody::rows(vec![row("Connectivity", "none")]);
        }
        TooltipBody::rows(vec![
            row(D::LABEL, self.rate.per_second_text()),
            row("Peak", self.peak_rate.per_second_text()),
            row("Interfaces", self.interfaces.join(", ")),
            row("Connectivity", self.connectivity.label()),
            row(
                "Capacity",
                InformationRate::new::<megabyte_per_second>(SCALE_END_MB_PER_S).per_second_text(),
            ),
        ])
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

fn connectivity_level(connectivity: Connectivity) -> Severity {
    match connectivity {
        Connectivity::Full => Severity::Normal,
        Connectivity::Limited | Connectivity::Unknown => Severity::Warning,
        Connectivity::None => Severity::Critical,
    }
}
