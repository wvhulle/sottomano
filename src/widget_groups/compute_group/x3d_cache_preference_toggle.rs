use super::core_grid_sections::topology_facts;
use crate::{
    data::source::sysfs_sensors::{
        cpu_topology::{CoreRole, CpuTopology},
        x3d_vcache::X3dCachePreference,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        toggle_control::Toggle,
        tooltip_body::{KeyValue, Section, TooltipBody},
    },
};

impl BarWidget for X3dCachePreference {
    fn name(&self) -> &'static str {
        "X3D scheduling preference"
    }

    fn icon(&self) -> Glyph {
        match self {
            X3dCachePreference::Cache => Phosphor::Warehouse(Weight::Regular).into(),
            X3dCachePreference::Frequency => Phosphor::Speedometer(Weight::Regular).into(),
        }
    }

    fn label(&self) -> Option<String> {
        Some(
            match self {
                X3dCachePreference::Cache => "cache first",
                X3dCachePreference::Frequency => "clock first",
            }
            .to_owned(),
        )
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([
            Section::dictionary([
                KeyValue::new("Now", favoured_chiplet(*self)),
                KeyValue::new("Click", favoured_chiplet(self.other())),
                KeyValue::new(
                    "How",
                    "the amd_x3d_vcache driver raises the favoured chiplet's preferred-core \
                     ranking, and the scheduler places new work on the highest-ranked idle cores",
                ),
                KeyValue::new(
                    "Matters",
                    "only while the busy threads fit on one chiplet; with both full, both run",
                ),
                KeyValue::new("Resets", "at reboot"),
            ])
            .titled("Which chiplet gets new work first"),
            Section::dictionary([
                KeyValue::new(
                    "Cache first",
                    "games and simulations whose working data exceeds the smaller L3",
                ),
                KeyValue::new(
                    "Clock first",
                    "lightly threaded work that fits in cache: the desktop, editors, \
                     single-threaded tools",
                ),
            ])
            .titled("Which to choose"),
        ])
    }
}

const fn favoured_chiplet(preference: X3dCachePreference) -> &'static str {
    match preference {
        X3dCachePreference::Cache => "the stacked-cache chiplet",
        X3dCachePreference::Frequency => "the high-clock chiplet",
    }
}

const fn favoured_role(preference: X3dCachePreference) -> CoreRole {
    match preference {
        X3dCachePreference::Cache => CoreRole::StackedCacheCores,
        X3dCachePreference::Frequency => CoreRole::HighClockCores,
    }
}

pub struct FavouredChiplets<'a> {
    pub preference: X3dCachePreference,
    pub topology: &'a CpuTopology,
}

impl FavouredChiplets<'_> {
    pub fn sections(&self) -> Vec<Section> {
        [
            self.section("Favoured now", self.preference),
            self.section("Favoured after a click", self.preference.other()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    fn section(&self, moment: &str, preference: X3dCachePreference) -> Option<Section> {
        let group = self
            .topology
            .core_groups()
            .iter()
            .find(|group| group.role() == favoured_role(preference))?;
        Some(Section::dictionary(topology_facts(group)).titled(format!("{moment}: {}", group.title())))
    }
}

impl Toggle for X3dCachePreference {
    fn toggled(&self) -> Self {
        self.other()
    }

    fn toggle(&self) -> bool {
        self.other().write()
    }

    fn press_does(&self) -> &'static str {
        match self {
            X3dCachePreference::Cache => "give new work to the high-clock chiplet first",
            X3dCachePreference::Frequency => "give new work to the stacked-cache chiplet first",
        }
    }
}
