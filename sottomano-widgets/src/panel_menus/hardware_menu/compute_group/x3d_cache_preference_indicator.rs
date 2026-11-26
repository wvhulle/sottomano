use sottomano_desktop_sinks::cpu_memory_compute::amd_x3d_mode_writes::write_x3d_cache_preference;
use sottomano_desktop_sources::{
    SnapshotSource,
    cpu_memory_compute::{
        amd_x3d_cache_preference::X3dCachePreference,
        cpu_topology::{CoreRole, CpuTopology},
    },
    system_protocol_clients::sysfs_attribute_reads::FileReadError,
};

use super::{ComputeGroup, ComputeInteraction, ComputeSource, core_grid_sections::topology_facts};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
        toggle_control::{SwitchPosition, ToggleableSource},
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::value_cells::LabeledValue,
};

const X3D_CACHE_PREFERENCE_GLYPH: Phosphor = Phosphor::Warehouse(Weight::Regular);

pub(crate) fn x3d_cache_preference_indicator(
    preference: &X3dCachePreference,
) -> GroupIndicator<'_> {
    let glyph = match preference {
        X3dCachePreference::Cache => Phosphor::Warehouse(Weight::Regular).into(),
        X3dCachePreference::Frequency => Phosphor::Speedometer(Weight::Regular).into(),
    };
    let label = match preference {
        X3dCachePreference::Cache => "cache first",
        X3dCachePreference::Frequency => "clock first",
    };
    GroupIndicator::glyph_only(X3dCachePreference::NAME, glyph, IndicatorState::Idle)
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(label.to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(|_| Vec::new()))
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

pub struct ChipletRoles<'snapshot> {
    pub topology: &'snapshot CpuTopology,
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
                    .core_groups
                    .iter()
                    .find(|group| group.role == role)
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

impl ComputeGroup {
    pub(super) fn x3d_cache_preference_listing(&self) -> IndicatorListing<'_> {
        self.x3d_cache_preference
            .sample
            .indicator_listing(
                |held_snapshot| {
                    x3d_cache_preference_indicator(held_snapshot)
                        .on_switch_press(
                            held_snapshot.switch_position(),
                            held_snapshot.press_action_text(),
                            ComputeInteraction::X3dCachePreferencePressed,
                        )
                        .when_shown()
                },
                X3D_CACHE_PREFERENCE_GLYPH.into(),
            )
            .map_indicators(|preference_row| {
                self.wired_x3d_cache_preference_indicator(preference_row)
            })
    }
    fn wired_x3d_cache_preference_indicator<'view>(
        &'view self,
        preference_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        preference_indicator
            .with_hover_card(Box::new(move |_| self.x3d_cache_preference_card()))
            .refreshed_by(ComputeInteraction::Refresh(
                ComputeSource::X3dCachePreference,
            ))
    }
    fn x3d_cache_preference_card(&self) -> Vec<Section> {
        let mut sections = self.x3d_cache_preference.hover_card();
        sections.extend(self.cpu_clock.sections_described_by(|cpu_clock| {
            ChipletRoles {
                topology: &cpu_clock.topology,
            }
            .sections()
        }));
        sections
    }
}
