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

pub struct ShownService<'a> {
    pub unit: &'a str,
    pub program_name: &'a str,
    pub arguments: &'a [&'a str],
    pub environment: &'a [(&'a str, &'a str)],
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

pub fn services_delivered<'a>(
    scope: SystemdUnitScope,
    shown_services: impl IntoIterator<Item = ShownService<'a>>,
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
