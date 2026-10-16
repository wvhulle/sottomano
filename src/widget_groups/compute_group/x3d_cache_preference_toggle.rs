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
        FavouredChiplets {
            preference: *self,
            topology: None,
        }
        .body()
    }
}

const fn favoured_role(preference: X3dCachePreference) -> CoreRole {
    match preference {
        X3dCachePreference::Cache => CoreRole::StackedCacheCores,
        X3dCachePreference::Frequency => CoreRole::HighClockCores,
    }
}

const fn suited_work(preference: X3dCachePreference) -> &'static str {
    match preference {
        X3dCachePreference::Cache => {
            "games and simulations whose working data exceeds the smaller L3"
        }
        X3dCachePreference::Frequency => {
            "lightly threaded work that fits in cache: the desktop, editors, single-threaded tools"
        }
    }
}

pub struct FavouredChiplets<'a> {
    pub preference: X3dCachePreference,
    pub topology: Option<&'a CpuTopology>,
}

impl FavouredChiplets<'_> {
    pub fn body(&self) -> TooltipBody {
        TooltipBody::sections([
            self.chiplet_section("Favoured now", self.preference),
            self.chiplet_section("Not favoured", self.preference.other()),
            Section::dictionary([
                KeyValue::new(
                    "How",
                    "the amd_x3d_vcache driver ranks the favoured cores highest, and the \
                     scheduler places new work on the highest-ranked idle cores",
                ),
                KeyValue::new(
                    "Matters",
                    "only while the busy threads fit on one chiplet; with both full, both run",
                ),
                KeyValue::new("Resets", "at reboot"),
            ]),
        ])
    }

    fn chiplet_section(&self, standing: &str, preference: X3dCachePreference) -> Section {
        let role = favoured_role(preference);
        let facts = self
            .topology
            .and_then(|topology| {
                topology
                    .core_groups()
                    .iter()
                    .find(|group| group.role() == role)
            })
            .map(topology_facts)
            .unwrap_or_default();
        Section::dictionary(
            facts
                .into_iter()
                .chain([KeyValue::new("Suits", suited_work(preference))]),
        )
        .titled(format!("{standing}: {}", role.title()))
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
            X3dCachePreference::Cache => "favour the high-clock cores instead",
            X3dCachePreference::Frequency => "favour the stacked-cache cores instead",
        }
    }
}
