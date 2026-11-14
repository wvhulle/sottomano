use std::borrow::Cow;

use chrono::Locale;
use iced::{Subscription, Task};
use sottomano_desktop_sinks::qemu_virtual_machines::libvirt_domain_shutdown::{
    request_libvirt_shutdown, request_libvirt_start, request_qmp_powerdown,
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
            GuestPowerControl, RunningVirtualMachine, VirtualMachineOrigin, VirtualMachinesReadout,
        },
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use tokio::task::spawn_blocking;

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{
        group_item::GroupItem, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState, toggle_control::SwitchPosition,
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, Section, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        process_usage_cells::{approximate_memory_text, cpu_share_quantity, uptime_entry},
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        app_window_focus::AppWindowFocus,
        icon_glyph::IconGlyph,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const FOCUS_WINDOW_HINT: &str = "Focus its window";
const POWER_OFF_HINT: &str = "Power off";
const POWER_ON_HINT: &str = "Power on";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtualMachineTarget {
    RunningProcess { pid: i32 },
    InactiveDomain { domain_name: Cow<'static, str> },
}

impl VirtualMachineTarget {
    #[must_use]
    pub const fn of_pid(pid: i32) -> Self {
        Self::RunningProcess { pid }
    }
}

impl From<VirtualMachineTarget> for HoverTarget {
    fn from(target: VirtualMachineTarget) -> Self {
        HoverTarget::VirtualMachine(target)
    }
}

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

struct VirtualMachineRow<'a> {
    machine: &'a RunningVirtualMachine,
}

impl VirtualMachineRow<'_> {
    fn configuration_entries(&self, clock: Clock) -> Vec<LabeledValue> {
        let machine = self.machine;
        let mut entries = vec![LabeledValue::new(
            "Origin",
            origin_text(&machine.origin).into_owned(),
        )];
        if let Some(launch_context) = &machine.launch_context {
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

struct InactiveDomainRow<'a> {
    domain_name: &'a str,
}

impl GroupItemReading for InactiveDomainRow<'_> {
    fn name(&self) -> &'static str {
        "Virtual machine"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.domain_name.to_owned().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::DesktopTower(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some("shut off".to_owned())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Off
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new(
                "Origin",
                origin_text(&VirtualMachineOrigin::LibvirtDomain).into_owned(),
            ),
            LabeledValue::new("State", "shut off"),
        ])
    }
}

impl GroupItemReading for VirtualMachineRow<'_> {
    fn name(&self) -> &'static str {
        "Virtual machine"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        match &self.machine.launch_context {
            Some(launch_context) => {
                format!("{} · {launch_context}", self.machine.launch.machine_name).into()
            }
            None => self.machine.launch.machine_name.clone().into(),
        }
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::DesktopTower(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        run_state_text(self.machine.run_state).map(str::to_owned)
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.machine.run_state {
            GuestRunState::Running | GuestRunState::Unreported => ReadingDisplayState::Idle,
            GuestRunState::Paused => ReadingDisplayState::Off,
            GuestRunState::ShuttingDown => ReadingDisplayState::Pending,
        }
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([Section::dictionary(self.configuration_entries(clock))])
    }
}

#[derive(Default)]
pub struct VirtualMachinesGroup {
    readout: LatestSnapshot<VirtualMachinesReadout, HidesFetchFailures>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum VirtualMachinesMessage {
    Machines(SnapshotDelivery<VirtualMachinesReadout>),
    WindowNotFound(i32),
    PowerChangeAccepted(String),
    PowerChangeRefused(String),
    Interaction(VirtualMachinesInteraction),
}

#[derive(Debug, Clone)]
pub enum VirtualMachinesInteraction {
    Expansion(WidgetGroupExpansionMessage),
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
                    .map_err(|failure| failure.to_string())
            },
            |shutdown| match shutdown {
                Ok(target_label) => VirtualMachinesMessage::PowerChangeAccepted(target_label),
                Err(failure) => VirtualMachinesMessage::PowerChangeRefused(failure),
            },
        ),
        GuestPowerControl::LibvirtDomain(domain_name) => Task::perform(
            async move {
                spawn_blocking(move || request_libvirt_shutdown(&domain_name).map(|()| domain_name))
                    .await
                    .unwrap_or_else(|join_failure| Err(join_failure.to_string()))
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
            spawn_blocking(move || request_libvirt_start(&domain_name).map(|()| domain_name))
                .await
                .unwrap_or_else(|join_failure| Err(join_failure.to_string()))
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
            VirtualMachinesMessage::Interaction(VirtualMachinesInteraction::Expansion(
                expansion_message,
            )) => {
                self.expansion.update(expansion_message);
                Action::Run(Task::none())
            }
            VirtualMachinesMessage::Interaction(VirtualMachinesInteraction::FocusWindow(pid)) => {
                Action::FocusAppWindow(AppWindowFocus {
                    owner: WindowOwner::Process(pid.unsigned_abs()),
                    unmatched: VirtualMachinesMessage::WindowNotFound(pid),
                })
            }
            VirtualMachinesMessage::Interaction(VirtualMachinesInteraction::PowerOff(
                power_control,
            )) => Action::Run(power_off_task(power_control)),
            VirtualMachinesMessage::Interaction(VirtualMachinesInteraction::PowerOn(
                domain_name,
            )) => Action::Run(power_on_task(domain_name)),
        }
    }

    fn machine_with_pid(&self, pid: i32) -> Option<&RunningVirtualMachine> {
        self.readout
            .get()?
            .machines
            .iter()
            .find(|machine| machine.pid == pid)
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &VirtualMachineTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            VirtualMachineTarget::RunningProcess { pid } => {
                let machine = self.machine_with_pid(*pid)?;
                self.readout
                    .hover_card_with(VirtualMachineRow { machine }.hover_card(clock))
            }
            VirtualMachineTarget::InactiveDomain { domain_name } => {
                let readout = self.readout.get()?;
                let domain_name = readout
                    .inactive_domains
                    .iter()
                    .find(|inactive| inactive.as_str() == domain_name.as_ref())?;
                self.readout
                    .hover_card_with(InactiveDomainRow { domain_name }.hover_card(clock))
            }
        }
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, VirtualMachinesInteraction> {
        let machines = self
            .readout
            .get()
            .map(|readout| readout.machines.as_slice())
            .unwrap_or_default();
        let rows = machines.iter().map(|machine| {
            let row = GroupItem::reading(
                VirtualMachineRow { machine },
                VirtualMachineTarget::of_pid(machine.pid),
            );
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
        let inactive_rows = inactive_domains.iter().map(|domain_name| {
            GroupItem::reading(
                InactiveDomainRow { domain_name },
                VirtualMachineTarget::InactiveDomain {
                    domain_name: Cow::Owned(domain_name.clone()),
                },
            )
            .on_switch_press(
                SwitchPosition::Off,
                POWER_ON_HINT,
                VirtualMachinesInteraction::PowerOn(domain_name.clone()),
            )
        });
        WidgetGroupContent::expandable(
            WidgetGroupFamily::VirtualMachines,
            &self.expansion,
            VirtualMachinesInteraction::Expansion,
            rows.chain(inactive_rows),
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::DesktopTower(Weight::Regular).into(),
        })
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
        let target_of = |domain_name: &'static str| VirtualMachineTarget::InactiveDomain {
            domain_name: Cow::Borrowed(domain_name),
        };
        assert!(
            group
                .hover_card_for(&target_of("windows-11"), clock)
                .is_some()
        );
        assert!(group.hover_card_for(&target_of("debian"), clock).is_none());
    }
}
