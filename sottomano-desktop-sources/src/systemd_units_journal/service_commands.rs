use std::{collections::BTreeMap, path::PathBuf};

use futures::{Stream, StreamExt, future::join_all};
use futures_rx::RxExt;
use zbus::{proxy::CacheProperties, zvariant::OwnedObjectPath};

use super::{ManagerProxy, ServiceProxy, SystemdOnBus, manager_signal};
use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::ChangeTriggerStreams,
    system_protocol_clients::{
        dbus_change_signals::change_triggers, protocol_peer_reconnects::subscribed,
    },
};

const SERVICE_SUFFIX: &str = ".service";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ServiceAccount {
    ManagerOwner,
    NamedUser(String),
}

impl From<String> for ServiceAccount {
    fn from(user: String) -> Self {
        if user.is_empty() {
            ServiceAccount::ManagerOwner
        } else {
            ServiceAccount::NamedUser(user)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServiceCommand {
    pub unit: String,
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub account: ServiceAccount,
}

impl ServiceCommand {
    #[must_use]
    pub fn program_name(&self) -> Option<&str> {
        self.program.file_name().and_then(|name| name.to_str())
    }

    pub fn argument(&self, flag: &str) -> Option<&str> {
        let position = self
            .arguments
            .iter()
            .position(|argument| argument == flag)?;
        self.arguments.get(position + 1).map(String::as_str)
    }

    pub fn arguments_after<'a>(
        &'a self,
        subcommand: &[&str],
    ) -> Option<impl Iterator<Item = &'a str>> {
        let position = self
            .arguments
            .windows(subcommand.len())
            .position(|window| {
                window
                    .iter()
                    .zip(subcommand)
                    .all(|(argument, expected)| argument == expected)
            })?;
        Some(
            self.arguments[position + subcommand.len()..]
                .iter()
                .map(String::as_str),
        )
    }

    pub fn variable(&self, name: &str) -> Option<&str> {
        self.environment.get(name).map(String::as_str)
    }

    #[must_use]
    pub fn unit_stem(&self) -> &str {
        self.unit
            .strip_suffix(SERVICE_SUFFIX)
            .unwrap_or(self.unit.as_str())
    }
}

fn environment_assignments(assignments: Vec<String>) -> BTreeMap<String, String> {
    assignments
        .into_iter()
        .filter_map(|assignment| {
            assignment
                .split_once('=')
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct ServiceCommands {
    pub services: Vec<ServiceCommand>,
}

impl SnapshotSource for ServiceCommands {
    type Error = zbus::Error;
    type StreamInputs = SystemdOnBus;

    const NAME: &'static str = "Systemd service commands";

    fn delivery_stream(systemd: SystemdOnBus) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        subscribed(systemd.connection(), watch_service_commands)
            .map(|listed| listed.map(|services| Self { services }))
            .map(SnapshotDelivery::live_push)
    }
}

async fn watch_service_commands(
    connection: zbus::Connection,
) -> zbus::Result<impl Stream<Item = zbus::Result<Vec<ServiceCommand>>> + Send> {
    let manager = ManagerProxy::builder(&connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    manager.ensure_subscribed().await?;
    let signals = change_triggers(
        &connection,
        [
            manager_signal("UnitNew")?,
            manager_signal("UnitRemoved")?,
            manager_signal("Reloading")?,
        ],
    )
    .await?;
    Ok(signals
        .triggered_rereads(move || {
            let manager = manager.clone();
            async move {
                all_service_commands(&manager)
                    .await
                    .map(|services| services.into_values().collect())
            }
        })
        .distinct_until_changed())
}

async fn all_service_commands(
    manager: &ManagerProxy<'static>,
) -> zbus::Result<BTreeMap<String, ServiceCommand>> {
    let rows = manager.list_units().await?;
    Ok(join_all(
        rows.into_iter()
            .filter(|row| row.name.ends_with(SERVICE_SUFFIX))
            .map(|row| service_command(manager, row.name, row.unit_path)),
    )
    .await
    .into_iter()
    .flatten()
    .map(|command| (command.unit.clone(), command))
    .collect())
}

async fn service_command(
    manager: &ManagerProxy<'static>,
    unit: String,
    path: OwnedObjectPath,
) -> Option<ServiceCommand> {
    let service = ServiceProxy::builder(manager.inner().connection())
        .path(path)
        .inspect_err(|path_failure| {
            tracing::warn!("service command of {unit}: invalid object path: {path_failure}");
        })
        .ok()?
        .cache_properties(CacheProperties::No)
        .build()
        .await
        .inspect_err(|proxy_failure| {
            tracing::warn!("service command of {unit}: service proxy: {proxy_failure}");
        })
        .ok()?;
    let exec_start = service
        .exec_start()
        .await
        .inspect_err(|exec_failure| {
            tracing::debug!("service command of {unit}: ExecStart unreadable: {exec_failure}");
        })
        .ok()?;
    let (program, argv, ..) = exec_start.into_iter().next()?;
    let environment = service
        .environment()
        .await
        .unwrap_or_else(|environment_failure| {
            tracing::debug!(
                "service command of {unit}: Environment unreadable: {environment_failure}"
            );
            Vec::new()
        });
    let account = service.user().await.unwrap_or_else(|user_failure| {
        tracing::debug!("service command of {unit}: User unreadable: {user_failure}");
        String::new()
    });
    Some(ServiceCommand {
        unit,
        program: PathBuf::from(program),
        arguments: argv.into_iter().skip(1).collect(),
        environment: environment_assignments(environment),
        account: ServiceAccount::from(account),
    })
}
