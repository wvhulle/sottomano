use crate::{
    data::source::sysfs_sensors::{cpu_clock::CpuClock, favg, fmax, fmin},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        font_awesome::Solid,
        tooltip_body::{TooltipBody, row},
    },
};

pub fn ghz(value: f64) -> String {
    format!("{value:.2} GHz")
}

impl BarWidget for CpuClock {
    fn icon(&self) -> Glyph {
        Solid::GaugeHigh.into()
    }

    fn label(&self) -> Option<String> {
        Some(ghz(favg(&self.core_ghz())))
    }

    fn tooltip(&self) -> TooltipBody {
        let per_core = self.core_ghz();
        let mut rows = vec![
            row("Average", ghz(favg(&per_core))),
            row("Min", ghz(fmin(&per_core))),
            row("Max", ghz(fmax(&per_core))),
        ];
        rows.extend(
            self.cores
                .iter()
                .map(|core| row(format!("Core {}", core.core), ghz(core.ghz))),
        );
        TooltipBody::rows(rows)
    }
}
