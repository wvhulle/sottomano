use sottomano_desktop_sources::{
    container_run_state::ContainerRunState,
    guest_machine_sizing::{GuestCoreCount, GuestMemorySize},
    microvm_guests::{
        EnclosingUnit, MicrovmGuest, MicrovmGuestListing, MicrovmUnit,
        hypervisor_command_lines::{GuestKernelImage, HypervisorApiSocket},
    },
    systemd_units_journal::unit_active_state::UnitActivity,
};

use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        unit_switch::activity_text,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::process_usage_cells::{approximate_memory_text, uptime_entry},
    widgets::value_cells::LabeledValue,
};

pub(super) struct MicrovmUnitIndicator<'view> {
    pub(super) unit: &'view MicrovmUnit,
    pub(super) activity: UnitActivity,
}

impl<'view> MicrovmUnitIndicator<'view> {
    pub(super) fn indicator(self) -> GroupIndicator<'view> {
        let state = match self.activity {
            UnitActivity::Running => IndicatorState::On,
            UnitActivity::Starting | UnitActivity::Stopping => IndicatorState::Pending,
            UnitActivity::Stopped => IndicatorState::Off,
        };
        GroupIndicator::glyph_only(
            self.unit.instance_name.clone(),
            Phosphor::Lightning(Weight::Regular).into(),
            state,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(activity_text(self.activity).to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([
                LabeledValue::new("State", activity_text(self.activity)),
                LabeledValue::new("Unit", self.unit.unit_name.clone()),
            ]),
        )]
    }
}

pub(super) struct MicrovmGuestListingIndicator<'view> {
    pub(super) listing: &'view MicrovmGuestListing,
}

impl<'view> MicrovmGuestListingIndicator<'view> {
    pub(super) fn indicator(self) -> GroupIndicator<'view> {
        GroupIndicator::glyph_only(
            self.listing.machine_name.clone(),
            Phosphor::Lightning(Weight::Regular).into(),
            ContainerRunState::Running.into(),
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(ContainerRunState::Running.label().to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(|_| Self::hover_card()))
    }

    pub(super) fn hover_card() -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([LabeledValue::new(
                "State",
                ContainerRunState::Running.label(),
            )]),
        )]
    }
}

pub(super) struct MicrovmGuestIndicator<'view> {
    pub(super) machine: &'view MicrovmGuest,
}

impl MicrovmGuestIndicator<'_> {
    fn detail_entries(&self, clock: Clock) -> Vec<LabeledValue> {
        let machine = self.machine;
        let mut detail_entries = vec![
            LabeledValue::new("Hypervisor", machine.hypervisor.product_name()),
            LabeledValue::new("Memory use", approximate_memory_text(machine.memory_in_use)),
        ];
        if let GuestMemorySize::Configured(guest_memory) = machine.launch.guest_memory {
            detail_entries.push(LabeledValue::new("Guest memory", guest_memory.to_string()));
        }
        if let GuestCoreCount::Configured(guest_cores) = machine.launch.guest_cores {
            detail_entries.push(LabeledValue::new("Cores", guest_cores.to_string()));
        }
        detail_entries.extend(uptime_entry(clock, machine.started_at));
        if let EnclosingUnit::Service(unit_name) = &machine.enclosing_unit {
            detail_entries.push(LabeledValue::new("Unit", unit_name.clone()));
        }
        if let GuestKernelImage::Booted(kernel) = &machine.launch.kernel {
            detail_entries.push(LabeledValue::new("Kernel", kernel.display().to_string()));
        }
        if let HypervisorApiSocket::Listening(api_socket) = &machine.launch.api_socket {
            detail_entries.push(LabeledValue::new(
                "API socket",
                api_socket.display().to_string(),
            ));
        }
        detail_entries.push(LabeledValue::new("PID", machine.listing.pid.to_string()));
        detail_entries
    }
}

impl<'view> MicrovmGuestIndicator<'view> {
    pub(super) fn indicator(self) -> GroupIndicator<'view> {
        GroupIndicator::glyph_only(
            self.machine.listing.machine_name.clone(),
            Phosphor::Lightning(Weight::Regular).into(),
            IndicatorState::On,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(approximate_memory_text(self.machine.memory_in_use)),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |card_clock| self.hover_card(card_clock)))
    }

    pub(super) fn hover_card(&self, clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Machine",
            dictionary_table(self.detail_entries(clock)),
        )]
    }
}
