use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Ratio, power::watt, ratio::percent},
};
use vetrata_config::severity::Severity;
use vetrata_sources::sysfs_sensors::{
    cpu_power::CpuPower,
    cpu_usage::{CpuCoreLoads, CpuUsage, LoadSplit},
};

use super::{
    chiplet_live_state::ChipletLiveState,
    core_grid_sections::{CoreGridSection, PerCoreReading},
};
use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    gauge::percent_text,
    phosphor::Phosphor,
    tooltip_body::{KeyValue, ReadingSections, Section, ThreadCell, TooltipBody},
    widget_state::WidgetState,
};

fn busy_level(busy: Ratio) -> Severity {
    if busy > Ratio::new::<percent>(90.0) {
        Severity::Critical
    } else if busy > Ratio::new::<percent>(70.0) {
        Severity::Warning
    } else {
        Severity::Normal
    }
}

struct CoreReading {
    core: u32,
    busy: Ratio,
    split: LoadSplit,
    deep_idle: Option<Ratio>,
}

fn mean_share<'a>(shares: impl Iterator<Item = Ratio> + 'a) -> Option<Ratio> {
    let collected: Vec<Ratio> = shares.collect();
    let count = collected.len();
    (count > 0).then(|| collected.into_iter().sum::<Ratio>() / count as f64)
}

impl ChipletLiveState<'_> {
    pub fn core_load_sections(&self, loads: &CpuCoreLoads) -> Vec<Section> {
        let readings: Vec<CoreReading> = loads
            .cores
            .iter()
            .map(|load| CoreReading {
                core: load.core,
                busy: load.busy,
                split: load.split,
                deep_idle: loads
                    .deep_idle
                    .iter()
                    .find(|idle| idle.core == load.core)
                    .map(|idle| idle.share),
            })
            .collect();
        let grid = CoreGridSection {
            topology: &loads.topology,
            readings: &readings,
        };
        vec![
            grid.section(
                |group| self.title(group),
                |group, members| {
                    let mean = |share: fn(&CoreReading) -> Ratio| {
                        mean_share(members.iter().map(|reading| share(reading)))
                    };
                    let load = mean(|reading| reading.busy).map(|busy| {
                        KeyValue::new("Load", percent_text(busy)).level(busy_level(busy))
                    });
                    let user = mean(|reading| reading.split.user)
                        .map(|user| KeyValue::new("User", percent_text(user)));
                    let kernel = mean(|reading| reading.split.system)
                        .map(|system| KeyValue::new("Kernel", percent_text(system)));
                    let deep_idle =
                        mean_share(members.iter().filter_map(|reading| reading.deep_idle))
                            .map(|idle| KeyValue::new("Deep sleep", percent_text(idle)));
                    load.into_iter()
                        .chain(user)
                        .chain(kernel)
                        .chain(deep_idle)
                        .chain(self.facts(group))
                        .collect()
                },
            )
            .titled((
                "Chiplets".to_owned(),
                "Each square is one physical core in core-number order, not die position, split \
                 into its hardware threads; bar height shows how busy each thread was over the \
                 last sample."
                    .to_owned(),
            )),
        ]
    }
}

impl PerCoreReading for CoreReading {
    fn core_index(&self) -> u32 {
        self.core
    }

    fn thread_cell(&self) -> ThreadCell {
        ThreadCell {
            fill: self.busy,
            level: busy_level(self.busy),
        }
    }
}

impl BarWidget for CpuUsage {
    fn name(&self) -> &'static str {
        "CPU load"
    }

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
        TooltipBody::dictionary([KeyValue::new("CPU", percent_text(self.busy))])
    }
}

impl ReadingSections for CpuPower {
    fn sections(&self) -> Vec<Section> {
        vec![
            Section::dictionary(self.zones.iter().map(|zone| {
                KeyValue::new(
                    zone.name.clone(),
                    format!("{:.1}", zone.draw.into_format_args(watt, Abbreviation)),
                )
            }))
            .titled("Power"),
        ]
    }
}
