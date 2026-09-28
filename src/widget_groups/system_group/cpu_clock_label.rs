use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Frequency, frequency::gigahertz},
};

use crate::{
    data::source::sysfs_sensors::cpu_clock::CpuClock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{Row, TooltipBody, row},
    },
};

pub fn frequency_text(frequency: Frequency) -> String {
    format!("{:.2}", frequency.into_format_args(gigahertz, Abbreviation))
}

impl CpuClock {
    pub fn summary_rows(&self, prefix: &str) -> Vec<Row> {
        [
            self.lowest()
                .map(|lowest| row(format!("{prefix} min"), frequency_text(lowest))),
            self.average()
                .map(|average| row(format!("{prefix} avg"), frequency_text(average))),
            self.highest()
                .map(|highest| row(format!("{prefix} max"), frequency_text(highest))),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl BarWidget for CpuClock {
    fn icon(&self) -> Glyph {
        Phosphor::Gauge(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        self.average().map(frequency_text)
    }

    fn tooltip(&self) -> TooltipBody {
        let per_core = self.cores.iter().map(|core| {
            row(
                format!("Core {}", core.core),
                frequency_text(core.frequency),
            )
        });
        TooltipBody::rows(
            self.summary_rows("Frequency")
                .into_iter()
                .chain(per_core)
                .collect(),
        )
    }
}
