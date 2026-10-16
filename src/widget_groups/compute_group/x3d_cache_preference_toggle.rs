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
        TooltipBody::sections(Self::facts())
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
    pub topology: Option<&'a CpuTopology>,
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

    fn facts() -> Vec<Section> {
        ChipletRoles { topology: None }.sections()
    }
}
