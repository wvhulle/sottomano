use std::borrow::Cow;

use sottomano_desktop_sources::{
    container_run_state::{ContainerRunState, RunChange},
    container_runtime_facts::ContainerMemoryUse,
    incus_instances::{
        DaemonInstance, DaemonInstanceListing, InstanceDaemon, InstanceImageDescription,
        InstanceKind,
    },
    podman_container_engine::libpod_running_containers::EngineReadError,
};

use super::IncusInstancesInteraction;
use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        process_usage_cells::approximate_memory_text,
        toggle_control::SwitchPosition,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    panel_menus::virtualization_menu::engine_container_indicators::engine_failure_label,
    widgets::value_cells::LabeledValue,
};

const START_HINT: &str = "start the instance";
const STOP_HINT: &str = "stop the instance";

fn kind_text(kind: InstanceKind) -> &'static str {
    match kind {
        InstanceKind::Container => "container",
        InstanceKind::VirtualMachine => "virtual machine",
    }
}

fn instance_icon(kind: InstanceKind) -> IconGlyph {
    match kind {
        InstanceKind::Container => Phosphor::Package(Weight::Regular).into(),
        InstanceKind::VirtualMachine => Phosphor::DesktopTower(Weight::Regular).into(),
    }
}

pub(super) fn instance_power_switch<'view>(
    row: GroupIndicator<'view>,
    instance_daemon: InstanceDaemon,
    listing: &DaemonInstanceListing,
) -> GroupIndicator<'view> {
    let (switch_position, hint, run_change) = match listing.run_state {
        ContainerRunState::Running => (SwitchPosition::On, STOP_HINT, RunChange::Stop),
        ContainerRunState::Stopped => (SwitchPosition::Off, START_HINT, RunChange::Start),
    };
    row.on_switch_press(
        switch_position,
        hint,
        IncusInstancesInteraction::ChangeState {
            instance_daemon,
            project_name: listing.project_name.clone(),
            instance_name: listing.instance_name.clone(),
            run_change,
        },
    )
}

fn daemon_instance_name(
    instance_daemon: InstanceDaemon,
    project_name: &str,
    instance_name: &str,
) -> Cow<'static, str> {
    let in_project = match project_name {
        "default" => instance_name.to_owned(),
        _ => format!("{instance_name} in {project_name}"),
    };
    match instance_daemon {
        InstanceDaemon::Incus => in_project.into(),
        InstanceDaemon::Lxd => format!("{in_project} on LXD").into(),
    }
}

pub(super) struct IncusInstanceIndicator<'view> {
    pub(super) instance_daemon: InstanceDaemon,
    pub(super) instance: &'view DaemonInstance,
}

impl<'view> IncusInstanceIndicator<'view> {
    pub(super) fn indicator(self) -> GroupIndicator<'view> {
        let label =
            if self.instance.listing.run_state == ContainerRunState::Stopped {
                Some(self.instance.listing.status_text.to_lowercase())
            } else {
                self.instance.global_addresses.first().cloned().or(
                    match self.instance.memory_in_use {
                        ContainerMemoryUse::Measured(memory_in_use) => {
                            Some(approximate_memory_text(memory_in_use))
                        }
                        ContainerMemoryUse::Unaccounted => None,
                    },
                )
            };
        GroupIndicator::glyph_only(
            daemon_instance_name(
                self.instance_daemon,
                &self.instance.listing.project_name,
                &self.instance.listing.instance_name,
            ),
            instance_icon(self.instance.listing.kind),
            self.instance.listing.run_state.into(),
        )
        .with_readout(IndicatorReadout {
            label: label.map_or(ReadoutLabel::Unlabeled, ReadoutLabel::Labeled),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        let instance = self.instance;
        let mut detail_entries = vec![
            LabeledValue::new("Kind", kind_text(instance.listing.kind)),
            LabeledValue::new("Status", instance.listing.status_text.clone()),
            LabeledValue::new("Daemon", self.instance_daemon.product_name()),
            LabeledValue::new("Project", instance.listing.project_name.clone()),
        ];
        if let ContainerMemoryUse::Measured(memory_in_use) = instance.memory_in_use {
            detail_entries.push(LabeledValue::new(
                "Memory use",
                approximate_memory_text(memory_in_use),
            ));
        }
        detail_entries.extend(
            instance
                .global_addresses
                .iter()
                .map(|global_address| LabeledValue::new("Address", global_address.clone())),
        );
        if let InstanceImageDescription::Described(image_description) = &instance.image_description
        {
            detail_entries.push(LabeledValue::new("Image", image_description.clone()));
        }
        vec![Section::new("Instance", dictionary_table(detail_entries))]
    }
}

pub(super) struct IncusInstanceListingIndicator<'view> {
    pub(super) instance_daemon: InstanceDaemon,
    pub(super) listing: &'view DaemonInstanceListing,
}

impl<'view> IncusInstanceListingIndicator<'view> {
    pub(super) fn indicator(self) -> GroupIndicator<'view> {
        GroupIndicator::glyph_only(
            daemon_instance_name(
                self.instance_daemon,
                &self.listing.project_name,
                &self.listing.instance_name,
            ),
            instance_icon(self.listing.kind),
            self.listing.run_state.into(),
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(self.listing.status_text.to_lowercase()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Instance",
            dictionary_table([
                LabeledValue::new("Kind", kind_text(self.listing.kind)),
                LabeledValue::new("Status", self.listing.status_text.clone()),
                LabeledValue::new("Daemon", self.instance_daemon.product_name()),
                LabeledValue::new("Project", self.listing.project_name.clone()),
            ]),
        )]
    }
}

pub(super) struct UnreadableDaemonIndicator<'view> {
    pub(super) instance_daemon: InstanceDaemon,
    pub(super) read_failure: &'view EngineReadError,
}

impl<'view> UnreadableDaemonIndicator<'view> {
    pub(super) fn indicator(self) -> GroupIndicator<'view> {
        GroupIndicator::glyph_only(
            self.instance_daemon.product_name(),
            Phosphor::Package(Weight::Regular).into(),
            IndicatorState::Failed,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(engine_failure_label(self.read_failure).to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Error",
            dictionary_table([
                LabeledValue::new("Problem", self.read_failure.to_string()),
                LabeledValue::new(
                    "Socket",
                    self.instance_daemon.socket_path().display().to_string(),
                ),
            ]),
        )]
    }
}
