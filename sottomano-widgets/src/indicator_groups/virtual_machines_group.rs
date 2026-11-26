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
        qemu_command_line_arguments::GuestDisplay,
        running_virtual_machines::{
            GuestPowerControl, RunningVirtualMachine, VirtualMachinesReadout,
        },
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::{app_window_focus::AppWindowFocus, subscribed_snapshot::SubscribedSnapshot},
};

mod virtual_machine_indicators;

use self::virtual_machine_indicators::{InactiveDomainIndicator, VirtualMachineIndicator};

const FOCUS_WINDOW_HINT: &str = "Focus its window";
const POWER_OFF_HINT: &str = "Power off";
const POWER_ON_HINT: &str = "Power on";

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
        let group_glyph = Phosphor::Browsers(Weight::Regular);
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::VirtualMachines,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::VirtualMachines.title(),
                group_glyph.into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            VirtualMachinesInteraction::Expansion,
            self.readout.indicator_listing(
                |readout| {
                    readout
                        .machines
                        .iter()
                        .map(|machine| self.running_machine_indicator(machine))
                        .chain(
                            readout
                                .inactive_domains
                                .iter()
                                .map(|domain_name| self.inactive_domain_indicator(domain_name)),
                        )
                        .collect()
                },
                group_glyph.into(),
            ),
        )
    }

    fn running_machine_indicator<'view>(
        &'view self,
        machine: &'view RunningVirtualMachine,
    ) -> GroupIndicator<'view> {
        let indicator = VirtualMachineIndicator { machine }
            .indicator()
            .with_hover_card(Box::new(move |clock| {
                self.readout
                    .hover_card_with(VirtualMachineIndicator { machine }.hover_card(clock))
            }));
        let indicator = match machine.launch.display {
            GuestDisplay::Windowed => indicator.on_right_press(
                FOCUS_WINDOW_HINT,
                VirtualMachinesInteraction::FocusWindow(machine.pid),
            ),
            GuestDisplay::Headless => indicator,
        };
        match &machine.power_control {
            GuestPowerControl::Unavailable => indicator.inert_switch(SwitchPosition::On),
            power_control @ GuestPowerControl::LibvirtDomain(_) => indicator.on_switch_press(
                SwitchPosition::On,
                POWER_OFF_HINT,
                VirtualMachinesInteraction::PowerOff(power_control.clone()),
            ),
            power_control @ GuestPowerControl::QmpSocket(_) => indicator.on_stop_press(
                POWER_OFF_HINT,
                VirtualMachinesInteraction::PowerOff(power_control.clone()),
            ),
        }
    }

    fn inactive_domain_indicator<'view>(
        &'view self,
        domain_name: &'view str,
    ) -> GroupIndicator<'view> {
        InactiveDomainIndicator { domain_name }
            .indicator()
            .with_hover_card(Box::new(|_| {
                self.readout
                    .hover_card_with(InactiveDomainIndicator::hover_card())
            }))
            .on_switch_press(
                SwitchPosition::Off,
                POWER_ON_HINT,
                VirtualMachinesInteraction::PowerOn(domain_name.to_owned()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
