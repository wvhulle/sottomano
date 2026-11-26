use super::{
    ComputeGroup,
    cpu_clock_label::cpu_clock_hover_card,
    cpu_clock_limit_causes::ClockLimitCauses,
    cpu_load_gauge::cpu_usage_hover_card,
    cpu_model_identity::{cpu_model_hover_card, cpu_model_panel_visibility},
    cpu_temperature_label::{hardware_throttling_sections, temperature_hover_card},
    x3d_cache_preference_toggle::ChipletRoles,
};
use crate::{
    group_indicators::group_indicator::PanelVisibility, hover_cards::hover_card_body::Section,
    panel_menus::hardware_menu::compute_group::cpu_load_gauge::cpu_busy_processes_sections,
};

impl ComputeGroup {
    pub(super) fn cpu_load_card(&self) -> Vec<Section> {
        let live = self.live();
        let mut sections = self.cpu_load.sections_described_by(cpu_usage_hover_card);
        sections.extend(
            self.cpu_busy_processes
                .sections_described_by(cpu_busy_processes_sections),
        );
        sections.extend(
            self.core_loads
                .sections_described_by(|loads| live.core_load_sections(loads)),
        );
        sections
    }

    pub(super) fn cpu_clock_card(&self) -> Vec<Section> {
        let live = self.live();
        let mut sections = self.cpu_clock.sections_described_by(cpu_clock_hover_card);
        sections.extend(
            self.cpu_clock.sections_described_by(|cpu_clock| {
                live.clock_sections(cpu_clock, &self.core_loads)
            }),
        );
        sections
    }

    pub(super) fn cpu_clock_limits_card(&self) -> Vec<Section> {
        let Some(cpu_clock) = self.cpu_clock.get() else {
            return Vec::new();
        };
        let limits = ClockLimitCauses { clock: cpu_clock };
        if limits.panel_visibility() != PanelVisibility::Shown {
            return Vec::new();
        }
        self.cpu_clock.hover_card_with(limits.hover_card())
    }

    pub(super) fn cpu_temperature_card(&self) -> Vec<Section> {
        let live = self.live();
        let mut sections = self
            .cpu_temperature
            .sections_described_by(temperature_hover_card);
        sections.extend(self.cpu_clock.sections_described_by(|cpu_clock| {
            hardware_throttling_sections(&cpu_clock.thermal_throttle_activity)
        }));
        sections.extend(
            self.core_temperatures
                .sections_described_by(|temperatures| live.core_temperature_sections(temperatures)),
        );
        sections
    }

    pub(super) fn cpu_model_card(&self) -> Vec<Section> {
        let Some(host_hardware) = self.host_hardware.get() else {
            return Vec::new();
        };
        let model = &host_hardware.cpu;
        if cpu_model_panel_visibility(model) != PanelVisibility::Shown {
            return Vec::new();
        }
        self.host_hardware
            .hover_card_with(cpu_model_hover_card(model))
    }

    pub(super) fn x3d_cache_preference_card(&self) -> Vec<Section> {
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
