use std::borrow::Cow;

use sottomano_desktop_sources::qemu_virtual_machines::{
    qemu_command_line_arguments::{ForwardedGuestPort, NixosSystemClosure, QmpSocketAccess},
    qmp_monitor_events::GuestRunState,
    running_virtual_machines::{
        GuestPowerControl, LaunchContext, RunningVirtualMachine, VirtualMachineOrigin,
    },
};

use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::process_usage_cells::{
        approximate_memory_text, cpu_share_quantity, uptime_entry,
    },
    widgets::value_cells::LabeledValue,
};

fn origin_text(origin: &VirtualMachineOrigin) -> Cow<'static, str> {
    match origin {
        VirtualMachineOrigin::SessionUser => "started by you".into(),
        VirtualMachineOrigin::NixosTestBuild => "NixOS test in a nix build".into(),
        VirtualMachineOrigin::LibvirtDomain => "libvirt domain".into(),
        VirtualMachineOrigin::OtherAccount(account_name) => {
            format!("started by {account_name}").into()
        }
    }
}

fn run_state_text(run_state: GuestRunState) -> Option<&'static str> {
    match run_state {
        GuestRunState::Running | GuestRunState::Unreported => None,
        GuestRunState::Paused => Some("paused"),
        GuestRunState::ShuttingDown => Some("shutting down"),
    }
}

fn forwarded_port_text(forwarded_port: &ForwardedGuestPort) -> String {
    format!(
        "{} {} → {}",
        forwarded_port.protocol, forwarded_port.host_port, forwarded_port.guest_port
    )
}

fn power_control_text(power_control: &GuestPowerControl) -> &'static str {
    match power_control {
        GuestPowerControl::QmpSocket(_) => "QMP",
        GuestPowerControl::LibvirtDomain(_) => "libvirt",
        GuestPowerControl::Unavailable => "unavailable",
    }
}

fn qmp_socket_text(machine: &RunningVirtualMachine) -> Option<String> {
    match (&machine.launch.qmp_socket, &machine.power_control) {
        (QmpSocketAccess::UnixSocket(socket_path), GuestPowerControl::QmpSocket(_)) => {
            Some(socket_path.display().to_string())
        }
        (QmpSocketAccess::UnixSocket(socket_path), _) => {
            Some(format!("{} (not reachable)", socket_path.display()))
        }
        (QmpSocketAccess::NotConfigured, _) => None,
    }
}

pub(super) struct VirtualMachineIndicator<'view> {
    pub(super) machine: &'view RunningVirtualMachine,
}

impl VirtualMachineIndicator<'_> {
    fn configuration_entries(&self, clock: Clock) -> Vec<LabeledValue> {
        let machine = self.machine;
        let mut entries = vec![LabeledValue::new(
            "Origin",
            origin_text(&machine.origin).into_owned(),
        )];
        if let LaunchContext::Named(launch_context) = &machine.launch_context {
            entries.push(LabeledValue::new("Test or project", launch_context.clone()));
        }
        if let Some(cpu_share) = cpu_share_quantity(machine.cpu_load) {
            entries.push(LabeledValue::new("CPU load", cpu_share));
        }
        entries.extend([
            LabeledValue::new(
                "Memory use",
                approximate_memory_text(machine.resident_memory),
            ),
            LabeledValue::new("Cores", machine.launch.guest_cores.to_string()),
            LabeledValue::new("Guest memory", machine.launch.guest_memory.to_string()),
        ]);
        if let NixosSystemClosure::Named(system_name) = &machine.launch.nixos_system {
            entries.push(LabeledValue::new("NixOS system", system_name.clone()));
        }
        entries.extend(
            machine
                .launch
                .disk_images
                .iter()
                .map(|disk_image| LabeledValue::new("Disk", disk_image.display().to_string())),
        );
        entries.extend(
            machine.launch.forwarded_ports.iter().map(|forwarded_port| {
                LabeledValue::new("Port", forwarded_port_text(forwarded_port))
            }),
        );
        if let Some(socket_text) = qmp_socket_text(machine) {
            entries.push(LabeledValue::new("QMP", socket_text));
        }
        entries.push(LabeledValue::new(
            "Power off",
            power_control_text(&machine.power_control),
        ));
        entries.extend(uptime_entry(clock, machine.started_at));
        entries
    }
}

pub(super) struct InactiveDomainIndicator<'view> {
    pub(super) domain_name: &'view str,
}

impl<'view> InactiveDomainIndicator<'view> {
    pub(super) fn indicator(self) -> GroupIndicator<'view> {
        GroupIndicator::glyph_only(
            self.domain_name.to_owned(),
            Phosphor::DesktopTower(Weight::Regular).into(),
            IndicatorState::Off,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled("shut off".to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(|_| Self::hover_card()))
    }

    pub(super) fn hover_card() -> Vec<Section> {
        vec![Section::new(
            "Machine",
            dictionary_table([
                LabeledValue::new(
                    "Origin",
                    origin_text(&VirtualMachineOrigin::LibvirtDomain).into_owned(),
                ),
                LabeledValue::new("State", "shut off"),
            ]),
        )]
    }
}

impl<'view> VirtualMachineIndicator<'view> {
    pub(super) fn indicator(self) -> GroupIndicator<'view> {
        let machine = self.machine;
        let name = match &machine.launch_context {
            LaunchContext::Named(launch_context) => format!(
                "{} from {launch_context} pid {}",
                machine.launch.machine_name, machine.pid
            ),
            LaunchContext::Unidentified => {
                format!("{} pid {}", machine.launch.machine_name, machine.pid)
            }
        };
        let label = match run_state_text(machine.run_state) {
            Some(text) => ReadoutLabel::Labeled(text.to_owned()),
            None => ReadoutLabel::Unlabeled,
        };
        let state = match machine.run_state {
            GuestRunState::Running | GuestRunState::Unreported => IndicatorState::Idle,
            GuestRunState::Paused => IndicatorState::Off,
            GuestRunState::ShuttingDown => IndicatorState::Pending,
        };
        GroupIndicator::glyph_only(name, Phosphor::DesktopTower(Weight::Regular).into(), state)
            .with_readout(IndicatorReadout {
                label,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            })
            .with_hover_card(Box::new(move |card_clock| self.hover_card(card_clock)))
    }

    pub(super) fn hover_card(&self, clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Machine",
            dictionary_table(self.configuration_entries(clock)),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_states_other_than_running_are_spelled_out() {
        assert_eq!(run_state_text(GuestRunState::Running), None);
        assert_eq!(run_state_text(GuestRunState::Unreported), None);
        assert_eq!(run_state_text(GuestRunState::Paused), Some("paused"));
    }

    #[test]
    fn a_forwarded_port_reads_host_to_guest() {
        assert_eq!(
            forwarded_port_text(&ForwardedGuestPort {
                protocol: "tcp".to_owned(),
                host_port: 2222,
                guest_port: 22,
            }),
            "tcp 2222 → 22"
        );
    }
}
