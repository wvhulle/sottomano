use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{
        f64::{Frequency, Ratio},
        ratio::percent,
    },
};

use super::cpu_clock_label::frequency_text;
use crate::{
    data::source::sysfs_sensors::cpu_usage::CpuUsage,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{Cell, TableRow, TooltipBody, row, section, table_section, untitled},
        widget_state::{Level, WidgetState},
    },
};

fn busy_level(busy: Ratio) -> Level {
    if busy > Ratio::new::<percent>(90.0) {
        Level::Critical
    } else if busy > Ratio::new::<percent>(70.0) {
        Level::Warning
    } else {
        Level::Ok
    }
}

fn percent_text(fraction: Ratio) -> String {
    format!("{:.0}", fraction.into_format_args(percent, Abbreviation))
}

struct CoreReading {
    core: u32,
    busy: Ratio,
    frequency: Option<Frequency>,
}

impl CpuUsage {
    fn core_readings(&self) -> Vec<CoreReading> {
        self.cores
            .iter()
            .map(|load| CoreReading {
                core: load.core,
                busy: load.busy,
                frequency: self
                    .clock
                    .cores
                    .iter()
                    .find(|clock| clock.core == load.core)
                    .map(|clock| clock.frequency),
            })
            .collect()
    }
}

impl TableRow for CoreReading {
    const HEADINGS: &'static [&'static str] = &["Load", "Clock"];

    fn key(&self) -> Cell {
        format!("Core {}", self.core).into()
    }

    fn cells(&self) -> Vec<Cell> {
        vec![
            percent_text(self.busy).into(),
            self.frequency
                .map_or_else(|| "—".to_owned(), frequency_text)
                .into(),
        ]
    }

    fn level(&self) -> Level {
        busy_level(self.busy)
    }
}

impl BarWidget for CpuUsage {
    fn icon(&self) -> Glyph {
        Phosphor::Cpu(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        busy_level(self.busy).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.busy)
    }

    fn tooltip(&self) -> TooltipBody {
        let frequencies = self.clock.summary_rows();
        let frequencies = if frequencies.is_empty() {
            vec![row("Frequency", "unavailable")]
        } else {
            frequencies
        };
        TooltipBody::Sections(vec![
            untitled(vec![row("CPU", percent_text(self.busy))]),
            section("Frequency", frequencies),
            table_section("Cores", self.core_readings()),
        ])
    }
}
