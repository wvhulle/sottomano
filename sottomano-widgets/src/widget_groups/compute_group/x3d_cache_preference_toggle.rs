use sottomano_sources::{
    cpu_memory_compute::{
        amd_x3d_cache_preference::X3dCachePreference,
        cpu_topology::{CoreRole, CpuTopology},
    },
    system_protocol_clients::sysfs_attribute_reads::Error,
};

use super::core_grid_sections::topology_facts;
use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    phosphor::{Phosphor, Weight},
    toggle_control::ToggleableSource,
    tooltip_body::{KeyValue, Section, TooltipBody},
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
        TooltipBody::sections([])
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

pub struct ChipletRoles<'a> {
    pub topology: &'a CpuTopology,
}

impl ChipletRoles<'_> {
    pub fn sections(&self) -> Vec<Section> {
        vec![
            self.chiplet_section(X3dCachePreference::Cache).titled((
                CoreRole::StackedCacheCores.title(),
                "New work goes first to the favoured cores while they have idle capacity; the \
                 choice resets at reboot.",
            )),
            self.chiplet_section(X3dCachePreference::Frequency)
                .titled(CoreRole::HighClockCores.title()),
        ]
    }

    fn chiplet_section(&self, preference: X3dCachePreference) -> Section {
        let role = favoured_role(preference);
        Section::dictionary(
            self.topology
                .core_groups()
                .iter()
                .find(|group| group.role() == role)
                .map(topology_facts)
                .into_iter()
                .flatten()
                .chain([KeyValue::new("Suits", suited_work(preference))]),
        )
    }
}

impl ToggleableSource for X3dCachePreference {
    fn toggled_value(&self) -> Self {
        self.other()
    }

    fn apply_toggle(&self) -> Result<(), Error> {
        self.other().write()
    }

    fn press_action_text(&self) -> &'static str {
        match self {
            X3dCachePreference::Cache => "favour the high-clock cores instead",
            X3dCachePreference::Frequency => "favour the stacked-cache cores instead",
        }
    }
}
