use bytesize::ByteSize;
use uom::si::{
    f64::{Information, InformationRate, Ratio, Time},
    information::byte,
    information_rate::{kilobyte_per_second, megabyte_per_second},
    ratio::ratio,
    time::second,
};

use crate::{
    data::source::throughput::{
        Connectivity, Direction, Download, SCALE_END_MB_PER_S, Throughput, Upload,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{TooltipBody, row},
        widget_state::{Level, WidgetState},
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

fn severity<D: Direction>(throughput: &Throughput<D>) -> Level {
    if throughput.interfaces.is_empty() {
        return Level::Critical;
    }
    connectivity_level(throughput.connectivity).max(throughput.link_level())
}

impl<D: DirectionIcon> BarWidget for Throughput<D> {
    fn icon(&self) -> Glyph {
        D::ICON.with_weight(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        let severity = severity(self);
        if self.unused() && severity == Level::Ok {
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
            row(D::LABEL, format_rate(self.rate)),
            row("Peak", format_rate(self.peak_rate)),
            row("Interfaces", self.interfaces.join(", ")),
            row("Connectivity", self.connectivity.label()),
            row(
                "Capacity",
                format_rate(InformationRate::new::<megabyte_per_second>(
                    SCALE_END_MB_PER_S,
                )),
            ),
        ])
    }
}

impl<D: Direction> Throughput<D> {
    fn link_level(&self) -> Level {
        if self.unused() {
            return Level::Ok;
        }
        if self.peak_rate
            < InformationRate::new::<kilobyte_per_second>(HI_RES_AUDIO_STREAM_KB_PER_S)
        {
            Level::Critical
        } else if self.peak_rate
            < InformationRate::new::<kilobyte_per_second>(HI_RES_VIDEO_STREAM_KB_PER_S)
        {
            Level::Warning
        } else {
            Level::Ok
        }
    }
}

fn connectivity_level(connectivity: Connectivity) -> Level {
    match connectivity {
        Connectivity::Full => Level::Ok,
        Connectivity::Limited | Connectivity::Unknown => Level::Warning,
        Connectivity::None => Level::Critical,
    }
}

fn format_rate(rate: InformationRate) -> String {
    let in_one_second: Information = (rate * Time::new::<second>(1.0)).into();
    let bytes = ByteSize::b(in_one_second.get::<byte>().round() as u64);
    format!("{}/s", bytes.display().iec())
}

#[cfg(test)]
mod tests {
    use uom::si::information_rate::{byte_per_second, kibibyte_per_second, mebibyte_per_second};

    use super::*;

    #[test]
    fn small_rates_keep_their_precision() {
        assert_eq!(
            format_rate(InformationRate::new::<byte_per_second>(312.0)),
            "312 B/s"
        );
        assert_eq!(
            format_rate(InformationRate::new::<kibibyte_per_second>(845.2)),
            "845.2 KiB/s"
        );
    }

    #[test]
    fn large_rates_switch_prefix() {
        assert_eq!(
            format_rate(InformationRate::new::<mebibyte_per_second>(1.8)),
            "1.8 MiB/s"
        );
    }
}
