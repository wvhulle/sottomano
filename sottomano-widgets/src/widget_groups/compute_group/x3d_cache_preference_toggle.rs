use sottomano_desktop_sinks::cpu_memory_compute::amd_x3d_mode_writes::write_x3d_cache_preference;
use sottomano_desktop_sources::{
    SnapshotSource,
    cpu_memory_compute::{
        amd_x3d_cache_preference::X3dCachePreference,
        cpu_topology::{CoreRole, CpuTopology},
    },
    system_protocol_clients::sysfs_attribute_reads::FileReadError,
};

use super::core_grid_sections::topology_facts;
use crate::{
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
        toggle_control::{SwitchPosition, ToggleableSource},
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::{
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub(crate) fn x3d_cache_preference_reading_row(preference: &X3dCachePreference) -> ReadingRow<'_> {
    let glyph = match preference {
        X3dCachePreference::Cache => Phosphor::Warehouse(Weight::Regular).into(),
        X3dCachePreference::Frequency => Phosphor::Speedometer(Weight::Regular).into(),
    };
    let label = match preference {
        X3dCachePreference::Cache => "cache first",
        X3dCachePreference::Frequency => "clock first",
    };
    ReadingRow {
        readout: RowReadout {
            label: ReadoutLabel::Labeled(label.to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        },
        ..ReadingRow::glyph_only(
            X3dCachePreference::NAME,
            glyph,
            ReadingDisplayState::Idle,
            Box::new(|_| Vec::new()),
        )
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
            self.chiplet_section(
                X3dCachePreference::Cache,
                SectionTitle::captioned(
                    CoreRole::StackedCacheCores.title(),
                    "New work goes first to the favoured cores while they have idle capacity; the \
                     choice resets at reboot.",
                ),
            ),
            self.chiplet_section(
                X3dCachePreference::Frequency,
                CoreRole::HighClockCores.title(),
            ),
        ]
    }

    fn chiplet_section(
        &self,
        preference: X3dCachePreference,
        title: impl Into<SectionTitle>,
    ) -> Section {
        let role = favoured_role(preference);
        Section::new(
            title,
            dictionary_table(
                self.topology
                    .core_groups()
                    .iter()
                    .find(|group| group.role() == role)
                    .map(topology_facts)
                    .into_iter()
                    .flatten()
                    .chain([LabeledValue::new("Suits", suited_work(preference))]),
            ),
        )
    }
}

impl ToggleableSource for X3dCachePreference {
    fn toggled_value(&self) -> Self {
        self.other()
    }

    fn apply_toggle(&self) -> impl Future<Output = Result<(), FileReadError>> + Send + 'static {
        let wanted_preference = self.other();
        async move { write_x3d_cache_preference(wanted_preference) }
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
