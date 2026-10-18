use std::{collections::BTreeMap, path::PathBuf};

use futures::{Stream, StreamExt, future::join_all};
use zbus::{proxy, proxy::CacheProperties, zvariant::OwnedObjectPath};

use super::{ManagerProxy, SystemdOnBus, manager_signal, signals_matching};
use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    stream_operators::{StreamOperators, connected_until_failure, read_once_then_per_burst},
};

const SERVICE_SUFFIX: &str = ".service";

type ExecCommandRow = (String, Vec<String>, bool, u64, u64, u64, u64, u32, i32, i32);

#[proxy(
    interface = "org.freedesktop.systemd1.Service",
    default_service = "org.freedesktop.systemd1",
    gen_blocking = false
)]
trait ServiceExec {
    #[zbus(property)]
    fn exec_start(&self) -> zbus::Result<Vec<ExecCommandRow>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServiceCommand {
    pub unit: String,
    pub program: PathBuf,
    pub arguments: Vec<String>,
}

impl ServiceCommand {
    #[must_use]
    pub fn runs(&self, program_name: &str) -> bool {
        self.program
            .file_name()
            .is_some_and(|name| name == program_name)
    }

    pub fn argument(&self, flag: &str) -> Option<&str> {
        let position = self
            .arguments
            .iter()
            .position(|argument| argument == flag)?;
        self.arguments.get(position + 1).map(String::as_str)
    }
}

#[derive(Debug, Clone)]
pub struct ServiceCommands {
    pub services: Vec<ServiceCommand>,
}

impl ReadingSource for ServiceCommands {
    type Error = zbus::Error;
    type StreamInputs = SystemdOnBus;

    const NAME: &'static str = "systemd service commands";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(systemd: SystemdOnBus) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        connected_until_failure(watch_service_commands(systemd.connection()))
            .map(|listed| listed.map(|services| Self { services }))
            .map(SourceDelivery::Reading)
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
    let signals = signals_matching(
        &connection,
        [
            manager_signal("UnitNew")?,
            manager_signal("UnitRemoved")?,
            manager_signal("Reloading")?,
        ],
    )
    .await?
    .map(drop);
    Ok(read_once_then_per_burst(signals, move || {
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
    let service = ServiceExecProxy::builder(manager.inner().connection())
        .path(path)
        .ok()?
        .cache_properties(CacheProperties::No)
        .build()
        .await
        .ok()?;
    let (program, argv, ..) = service.exec_start().await.ok()?.into_iter().next()?;
    Some(ServiceCommand {
        unit,
        program: PathBuf::from(program),
        arguments: argv.into_iter().skip(1).collect(),
    })
}
