use std::{path::PathBuf, time::Duration};

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    podman_container_engine::libpod_running_containers::{
        ContainerCiIdentity, EngineContainers, EngineListing, EngineReadError, EngineReadout,
        RunningContainer, StoppedContainer,
    },
    system_protocol_clients::procfs_process_tree::ProcessTreeCpuLoad,
    systemd_units_journal::{SystemdUnitScope, service_commands::ServiceAccount},
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::podman_engines_group::{PodmanEnginesGroup, PodmanEnginesMessage},
};
use uom::si::{f64::Ratio, ratio::percent};

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
    busy_share_percent: f64,
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
        cpu_load: ProcessTreeCpuLoad::BusyShare(Ratio::new::<percent>(busy_share_percent)),
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
            EngineReadout::Containers(EngineListing {
                running: EngineContainers {
                    containers: Vec::new(),
                },
                stopped: vec![StoppedContainer {
                    id: "toolbox".to_owned(),
                    container_name: "fedora-toolbox-42".to_owned(),
                    shown_name: "fedora-toolbox-42".to_owned(),
                    image: "registry.fedoraproject.org/fedora-toolbox:42".to_owned(),
                    stopped_at: clock.wall - Duration::from_hours(5),
                }],
            }),
        ),
        delivered(
            WOODPECKER_SOCKET,
            EngineReadout::Containers(EngineListing {
                running: EngineContainers {
                    containers: vec![
                        step_container("run-tests", 19.0, ByteSize::gib(4), clock),
                        step_container("clone", 1.2, ByteSize::mib(120), clock),
                    ],
                },
                stopped: Vec::new(),
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
        .for_each(|message| {
            let _ = group.update(message, clock);
        });
    ShowcasedGroup::PodmanEngines(Box::new(group))
}
