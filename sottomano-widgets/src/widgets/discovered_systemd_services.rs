use iced::Subscription;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{
        SystemdOnBus, SystemdUnitScope,
        service_commands::{ServiceCommand, ServiceCommands},
    },
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    widgets::latest_snapshot::{HidesFetchFailures, LatestSnapshot},
};

const DISCOVERED_SCOPES: [SystemdUnitScope; 2] =
    [SystemdUnitScope::UserSession, SystemdUnitScope::SystemWide];

#[derive(Debug)]
pub struct ScopedServiceCommands {
    pub scope: SystemdUnitScope,
    pub delivery: SnapshotDelivery<ServiceCommands>,
}

#[derive(Default)]
pub struct DiscoveredSystemdServices {
    user_services: LatestSnapshot<ServiceCommands, HidesFetchFailures>,
    system_services: LatestSnapshot<ServiceCommands, HidesFetchFailures>,
}

impl DiscoveredSystemdServices {
    pub fn subscription(buses: &DesktopBuses) -> Subscription<ScopedServiceCommands> {
        Subscription::batch(DISCOVERED_SCOPES.map(|scope| {
            snapshot_subscription::<ServiceCommands>(SystemdOnBus {
                buses: buses.clone(),
                scope,
            })
            .with(scope)
            .map(|(scope, delivery)| ScopedServiceCommands { scope, delivery })
        }))
    }

    pub fn receive(&mut self, scoped_commands: ScopedServiceCommands, clock: Clock) {
        self.services_in_mut(scoped_commands.scope)
            .receive(scoped_commands.delivery, clock);
    }

    fn services_in_mut(
        &mut self,
        scope: SystemdUnitScope,
    ) -> &mut LatestSnapshot<ServiceCommands, HidesFetchFailures> {
        match scope {
            SystemdUnitScope::UserSession => &mut self.user_services,
            SystemdUnitScope::SystemWide => &mut self.system_services,
        }
    }

    fn services_in(
        &self,
        scope: SystemdUnitScope,
    ) -> &LatestSnapshot<ServiceCommands, HidesFetchFailures> {
        match scope {
            SystemdUnitScope::UserSession => &self.user_services,
            SystemdUnitScope::SystemWide => &self.system_services,
        }
    }

    pub fn discovered<Discovered: 'static>(
        &self,
        discover: fn(SystemdUnitScope, &ServiceCommand) -> Option<Discovered>,
    ) -> impl Iterator<Item = Discovered> + '_ {
        DISCOVERED_SCOPES.into_iter().flat_map(move |scope| {
            self.services_in(scope)
                .get()
                .into_iter()
                .flat_map(|listed| listed.services.iter())
                .filter_map(move |command| discover(scope, command))
        })
    }
}
