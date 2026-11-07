use chrono::Locale;
use sottomano_desktop_sinks::cpu_memory_compute::amd_x3d_mode_writes::write_x3d_cache_preference;
use sottomano_desktop_sources::{
    cpu_memory_compute::{
        amd_x3d_cache_preference::X3dCachePreference,
        cpu_topology::{CoreRole, CpuTopology},
    },
    system_protocol_clients::sysfs_attribute_reads::Error,
};

use super::core_grid_sections::topology_facts;
use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading,
        toggle_control::{SwitchPosition, ToggleableSource},
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

impl GroupItemReading for X3dCachePreference {
    fn name(&self) -> &'static str {
        "X3D scheduling preference"
    }

    fn icon(&self) -> IconGlyph {
        match self {
            X3dCachePreference::Cache => Phosphor::Warehouse(Weight::Regular).into(),
            X3dCachePreference::Frequency => Phosphor::Speedometer(Weight::Regular).into(),
        }
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(
            match self {
                X3dCachePreference::Cache => "cache first",
                X3dCachePreference::Frequency => "clock first",
            }
            .to_owned(),
        )
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
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
                .chain([LabeledValue::new("Suits", suited_work(preference))]),
        )
    }
}

impl ToggleableSource for X3dCachePreference {
    fn toggled_value(&self) -> Self {
        self.other()
    }

    fn apply_toggle(&self) -> Result<(), Error> {
        write_x3d_cache_preference(self.other())
    }

    fn switch_position(&self) -> SwitchPosition {
        match self {
            X3dCachePreference::Cache => SwitchPosition::On,
            X3dCachePreference::Frequency => SwitchPosition::Off,
        }
    }

    fn press_action_text(&self) -> &'static str {
        match self {
            X3dCachePreference::Cache => "favour the high-clock cores instead",
            X3dCachePreference::Frequency => "favour the stacked-cache cores instead",
        }
    }
}
