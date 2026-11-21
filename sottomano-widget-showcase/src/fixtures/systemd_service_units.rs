use std::{collections::BTreeMap, path::PathBuf};

use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::DeliveryCadence,
    systemd_units_journal::{
        SystemdUnitScope,
        service_commands::{ServiceAccount, ServiceCommand, ServiceCommands},
    },
};
use sottomano_widgets::widgets::discovered_systemd_services::ScopedServiceCommands;

pub struct ShownService<'fixture> {
    pub unit: &'fixture str,
    pub program_name: &'fixture str,
    pub arguments: &'fixture [&'fixture str],
    pub environment: &'fixture [(&'fixture str, &'fixture str)],
    pub account: ServiceAccount,
}

impl ShownService<'_> {
    fn command(&self) -> ServiceCommand {
        ServiceCommand {
            unit: self.unit.to_owned(),
            program: PathBuf::from("/run/current-system/sw/bin").join(self.program_name),
            arguments: self
                .arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect(),
            environment: self
                .environment
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect::<BTreeMap<_, _>>(),
            account: self.account.clone(),
        }
    }
}

pub fn services_delivered<'fixture>(
    scope: SystemdUnitScope,
    shown_services: impl IntoIterator<Item = ShownService<'fixture>>,
) -> ScopedServiceCommands {
    ScopedServiceCommands {
        scope,
        delivery: SnapshotDelivery::fixture_delivered(
            Ok(ServiceCommands {
                services: shown_services
                    .into_iter()
                    .map(|shown| shown.command())
                    .collect(),
            }),
            DeliveryCadence::LivePush,
        ),
    }
}
