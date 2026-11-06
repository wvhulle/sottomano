use std::{path::PathBuf, time::Duration};

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    podman_container_engine::libpod_running_containers::{
        ContainerCiIdentity, EngineContainers, EngineReadError, EngineReadout, RunningContainer,
    },
    system_protocol_clients::procfs_process_tree::ProcessTreeCpuLoad,
    systemd_units_journal::{SystemdUnitScope, service_commands::ServiceAccount},
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::podman_engines_group::{PodmanEnginesGroup, PodmanEnginesMessage},
};

use crate::{
    fixtures::systemd_service_units::{ShownService, services_delivered},
    showcased_group::ShowcasedGroup,
};

const ROOTLESS_SOCKET: &str = "/run/user/1000/podman/podman.sock";
const WOODPECKER_SOCKET: &str = "/run/woodpecker-agent-codeberg/podman.sock";
const FORGEJO_SOCKET: &str = "/run/forgejo-runner-codeberg/podman.sock";

fn podman_service<'a>(
    unit: &'a str,
    arguments: &'a [&'a str],
    account: ServiceAccount,
) -> ShownService<'a> {
    ShownService {
        unit,
        program_name: "podman",
        arguments,
        environment: &[],
        account,
    }
}

fn discovered_engines() -> [PodmanEnginesMessage; 2] {
    [
        PodmanEnginesMessage::Services(services_delivered(
            SystemdUnitScope::UserSession,
            [podman_service(
                "podman.service",
                &[
                    "system",
                    "service",
                    "unix:///run/user/1000/podman/podman.sock",
                ],
                ServiceAccount::ManagerOwner,
            )],
        )),
        PodmanEnginesMessage::Services(services_delivered(
            SystemdUnitScope::SystemWide,
            [
                podman_service(
                    "woodpecker-agent-codeberg-podman.service",
                    &[
                        "system",
                        "service",
                        "--time=0",
                        "unix:/run/woodpecker-agent-codeberg/podman.sock",
                    ],
                    ServiceAccount::NamedUser("woodpecker-agent-codeberg".to_owned()),
                ),
                podman_service(
                    "forgejo-runner-codeberg-podman.service",
                    &[
                        "system",
                        "service",
                        "--time=0",
                        "unix:/run/forgejo-runner-codeberg/podman.sock",
                    ],
                    ServiceAccount::NamedUser("forgejo-runner-codeberg".to_owned()),
                ),
            ],
        )),
    ]
}

fn step_container(
    shown_name: &str,
    busy_cores: f64,
    resident_memory: ByteSize,
    clock: Clock,
) -> RunningContainer {
    RunningContainer {
        id: shown_name.to_owned(),
        container_name: format!("wp_{shown_name}"),
        shown_name: shown_name.to_owned(),
        image: "docker.io/nixos/nix:latest".to_owned(),
        started_at: clock.wall - Duration::from_mins(4),
        ci_identity: ContainerCiIdentity::NotWoodpecker,
        cpu_load: ProcessTreeCpuLoad::BusyCores(busy_cores),
        resident_memory,
    }
}

fn delivered(socket_path: &str, readout: EngineReadout) -> PodmanEnginesMessage {
    PodmanEnginesMessage::Engine(
        PathBuf::from(socket_path),
        SnapshotDelivery::fixture_delivered(
            Ok(readout),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        ),
    )
}

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = PodmanEnginesGroup::default();
    let readouts = [
        delivered(
            ROOTLESS_SOCKET,
            EngineReadout::Containers(EngineContainers {
                containers: Vec::new(),
            }),
        ),
        delivered(
            WOODPECKER_SOCKET,
            EngineReadout::Containers(EngineContainers {
                containers: vec![
                    step_container("run-tests", 6.2, ByteSize::gib(4), clock),
                    step_container("clone", 0.4, ByteSize::mib(120), clock),
                ],
            }),
        ),
        delivered(
            FORGEJO_SOCKET,
            EngineReadout::Unreadable(EngineReadError::SocketDenied),
        ),
    ];
    discovered_engines()
        .into_iter()
        .chain(readouts)
        .for_each(|message| group.update(message, clock));
    ShowcasedGroup::PodmanEngines(Box::new(group))
}
