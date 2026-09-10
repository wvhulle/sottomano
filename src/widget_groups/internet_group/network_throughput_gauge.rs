use bytesize::ByteSize;

use crate::{
    data::source::throughput::{Connectivity, Direction, Download, SCALE_END, Throughput, Upload},
    widgets::{
        bar_widget::{BarWidget, Glyph, Level, Unit, Value},
        font_awesome::Solid,
        tooltip_body::{TooltipBody, row},
    },
};

const HI_RES_AUDIO_STREAM: ByteSize = ByteSize::kb(1_152);
const HI_RES_VIDEO_STREAM: ByteSize = ByteSize::kb(3_125);

pub trait DirectionIcon: Direction {
    const ICON: Solid;
}

impl DirectionIcon for Upload {
    const ICON: Solid = Solid::Upload;
}

impl DirectionIcon for Download {
    const ICON: Solid = Solid::Download;
}

impl<D: DirectionIcon> BarWidget for Throughput<D> {
    fn icon(&self) -> Glyph {
        D::ICON.into()
    }

    fn disabled(&self) -> bool {
        self.unused() && self.level() == Level::Ok
    }

    fn level(&self) -> Level {
        if self.interfaces.is_empty() {
            return Level::Critical;
        }
        connectivity_level(self.connectivity).max(self.link_level())
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: percent(self.peak_fraction()),
            unit: Unit::Percent,
        })
    }

    fn live_fraction(&self) -> f32 {
        let peak = self.peak_fraction();
        if peak <= 0.0 {
            return 1.0;
        }
        (self.current_fraction() / peak).clamp(0.0, 1.0)
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
            row("Capacity", format_rate(SCALE_END)),
        ])
    }
}

impl<D: Direction> Throughput<D> {
    fn link_level(&self) -> Level {
        if self.unused() {
            return Level::Ok;
        }
        match self.peak_rate {
            peak if peak < HI_RES_AUDIO_STREAM => Level::Critical,
            peak if peak < HI_RES_VIDEO_STREAM => Level::Warning,
            _ => Level::Ok,
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

fn format_rate(rate: ByteSize) -> String {
    format!("{}/s", rate.display().iec())
}

#[allow(clippy::cast_possible_truncation)]
fn percent(fraction: f32) -> i64 {
    (fraction * 100.0).round() as i64
}
