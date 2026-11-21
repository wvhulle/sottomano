use std::borrow::Cow;

use iced::{Subscription, Task};
use sottomano_desktop_sinks::{
    qemu_virtual_machines::libvirt_domain_shutdown::{
        request_libvirt_shutdown, request_libvirt_start, request_qmp_powerdown,
    },
    sink_write_errors::GuestPowerError,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    desktop_windows_apps::window_edge_placement::WindowOwner,
    qemu_virtual_machines::{
        qemu_command_line_arguments::{
            ForwardedGuestPort, GuestDisplay, NixosSystemClosure, QmpSocketAccess,
        },
        qmp_monitor_events::GuestRunState,
        running_virtual_machines::{
            GuestPowerControl, LaunchContext, RunningVirtualMachine, VirtualMachineOrigin,
            VirtualMachinesReadout,
        },
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
        process_usage_cells::{approximate_memory_text, cpu_share_quantity, uptime_entry},
    },
    widgets::{
        app_window_focus::AppWindowFocus, subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
    },
};

const FOCUS_WINDOW_HINT: &str = "Focus its window";
const POWER_OFF_HINT: &str = "Power off";
const POWER_ON_HINT: &str = "Power on";

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

struct VirtualMachineIndicator<'view> {
    machine: &'view RunningVirtualMachine,
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

struct InactiveDomainIndicator<'view> {
    domain_name: &'view str,
}

impl<'view> InactiveDomainIndicator<'view> {
    fn indicator(self) -> GroupIndicator<'view> {
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

    fn hover_card() -> Vec<Section> {
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
    fn indicator(self) -> GroupIndicator<'view> {
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

    fn hover_card(&self, clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Machine",
            dictionary_table(self.configuration_entries(clock)),
        )]
    }
}

#[derive(Default)]
pub struct VirtualMachinesGroup {
    readout: SubscribedSnapshot<VirtualMachinesReadout>,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum VirtualMachinesMessage {
    Machines(SnapshotDelivery<VirtualMachinesReadout>),
    WindowNotFound(i32),
    PowerChangeAccepted(String),
    PowerChangeRefused(GuestPowerError),
}

#[derive(Debug, Clone)]
pub enum VirtualMachinesInteraction {
    Expansion(IndicatorGroupExpansionMessage),
    FocusWindow(i32),
    PowerOff(GuestPowerControl),
    PowerOn(String),
}

pub enum Action {
    Run(Task<VirtualMachinesMessage>),
    FocusAppWindow(AppWindowFocus<VirtualMachinesMessage>),
}

fn power_off_task(power_control: GuestPowerControl) -> Task<VirtualMachinesMessage> {
    match power_control {
        GuestPowerControl::QmpSocket(socket_path) => Task::perform(
            async move {
                let target_label = socket_path.display().to_string();
                request_qmp_powerdown(&socket_path)
                    .await
                    .map(|()| target_label)
            },
            |shutdown| match shutdown {
                Ok(target_label) => VirtualMachinesMessage::PowerChangeAccepted(target_label),
                Err(failure) => VirtualMachinesMessage::PowerChangeRefused(failure),
            },
        ),
        GuestPowerControl::LibvirtDomain(domain_name) => Task::perform(
            async move {
                request_libvirt_shutdown(domain_name.clone())
                    .await
                    .map(|()| domain_name)
            },
            |shutdown| match shutdown {
                Ok(domain_name) => VirtualMachinesMessage::PowerChangeAccepted(domain_name),
                Err(failure) => VirtualMachinesMessage::PowerChangeRefused(failure),
            },
        ),
        GuestPowerControl::Unavailable => Task::none(),
    }
}

fn power_on_task(domain_name: String) -> Task<VirtualMachinesMessage> {
    Task::perform(
        async move {
            request_libvirt_start(domain_name.clone())
                .await
                .map(|()| domain_name)
        },
        |start| match start {
            Ok(domain_name) => VirtualMachinesMessage::PowerChangeAccepted(domain_name),
            Err(failure) => VirtualMachinesMessage::PowerChangeRefused(failure),
        },
    )
}

impl VirtualMachinesGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(buses: &DesktopBuses) -> Subscription<VirtualMachinesMessage> {
        snapshot_subscription::<VirtualMachinesReadout>(buses.clone())
            .map(VirtualMachinesMessage::Machines)
    }

    pub fn update(&mut self, message: VirtualMachinesMessage, clock: Clock) -> Action {
        match message {
            VirtualMachinesMessage::Machines(delivery) => {
                self.readout.receive(delivery, clock);
                Action::Run(Task::none())
            }
            VirtualMachinesMessage::WindowNotFound(pid) => {
                tracing::debug!("virtual machine {pid} has no window");
                Action::Run(Task::none())
            }
            VirtualMachinesMessage::PowerChangeAccepted(machine_label) => {
                tracing::info!(%machine_label, "virtual machine power change accepted");
                Action::Run(Task::none())
            }
            VirtualMachinesMessage::PowerChangeRefused(failure) => {
                tracing::warn!(%failure, "virtual machine power change refused");
                Action::Run(Task::none())
            }
        }
    }

    pub fn interact(&mut self, interaction: VirtualMachinesInteraction) -> Action {
        match interaction {
            VirtualMachinesInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Action::Run(Task::none())
            }
            VirtualMachinesInteraction::FocusWindow(pid) => {
                Action::FocusAppWindow(AppWindowFocus {
                    owner: WindowOwner::Process(pid.unsigned_abs()),
                    unmatched: VirtualMachinesMessage::WindowNotFound(pid),
                })
            }
            VirtualMachinesInteraction::PowerOff(power_control) => {
                Action::Run(power_off_task(power_control))
            }
            VirtualMachinesInteraction::PowerOn(domain_name) => {
                Action::Run(power_on_task(domain_name))
            }
        }
    }

    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        let machines = self
            .readout
            .get()
            .map(|readout| readout.machines.as_slice())
            .unwrap_or_default();
        let rows = machines.iter().map(|machine| {
            let row = VirtualMachineIndicator { machine }
                .indicator()
                .with_hover_card(Box::new(move |clock| {
                    self.readout
                        .hover_card_with(VirtualMachineIndicator { machine }.hover_card(clock))
                }));
            let row = match machine.launch.display {
                GuestDisplay::Windowed => row.on_right_press(
                    FOCUS_WINDOW_HINT,
                    VirtualMachinesInteraction::FocusWindow(machine.pid),
                ),
                GuestDisplay::Headless => row,
            };
            match &machine.power_control {
                GuestPowerControl::Unavailable => row.inert_switch(SwitchPosition::On),
                power_control @ GuestPowerControl::LibvirtDomain(_) => row.on_switch_press(
                    SwitchPosition::On,
                    POWER_OFF_HINT,
                    VirtualMachinesInteraction::PowerOff(power_control.clone()),
                ),
                power_control @ GuestPowerControl::QmpSocket(_) => row.on_stop_press(
                    POWER_OFF_HINT,
                    VirtualMachinesInteraction::PowerOff(power_control.clone()),
                ),
            }
        });
        let inactive_domains = self
            .readout
            .get()
            .map(|readout| readout.inactive_domains.as_slice())
            .unwrap_or_default();
        let inactive_indicators = inactive_domains.iter().map(|domain_name| {
            InactiveDomainIndicator { domain_name }
                .indicator()
                .with_hover_card(Box::new(|_| {
                    self.readout
                        .hover_card_with(InactiveDomainIndicator::hover_card())
                }))
                .on_switch_press(
                    SwitchPosition::Off,
                    POWER_ON_HINT,
                    VirtualMachinesInteraction::PowerOn(domain_name.clone()),
                )
        });
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::VirtualMachines,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::VirtualMachines.title(),
                Phosphor::Browsers(Weight::Regular).into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            VirtualMachinesInteraction::Expansion,
            rows.chain(inactive_indicators),
        )
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

    #[test]
    fn a_shut_off_libvirt_domain_has_its_own_hover_card() {
        use std::time::Duration;

        use sottomano_desktop_sources::delivery_cadence_drivers::{
            DeliveryCadence, PeriodicPolling,
        };

        use crate::{
            hover_cards::{HoverTarget, IndicatorKey},
            indicator_groups::indicator_group_identity::IndicatorGroupIdentity,
        };

        let clock = Clock::real();
        let mut group = VirtualMachinesGroup::default();
        let _ = group.update(
            VirtualMachinesMessage::Machines(SnapshotDelivery::fixture_delivered(
                Ok(VirtualMachinesReadout {
                    machines: Vec::new(),
                    inactive_domains: vec!["windows-11".to_owned()],
                }),
                DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
            )),
            clock,
        );
        let target_of = |domain_name: &'static str| {
            HoverTarget::Indicator(IndicatorKey {
                group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::VirtualMachines),
                name: domain_name.into(),
            })
        };
        let content = group.group_content();
        assert!(
            content
                .hover_card(&target_of("windows-11"), clock)
                .is_some_and(|card| !card.sections.is_empty())
        );
        assert!(content.hover_card(&target_of("debian"), clock).is_none());
    }
}
