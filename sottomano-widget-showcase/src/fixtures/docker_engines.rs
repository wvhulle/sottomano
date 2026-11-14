use std::{path::PathBuf, time::Duration};

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    docker_container_engine::{DockerEngineReadout, DockerEngineSocket},
    podman_container_engine::{
        libpod_running_containers::{
            ContainerCiIdentity, EngineContainers, EngineListing, EngineReadout, RunningContainer,
            StoppedContainer,
        },
        systemd_service_engine_discovery::EngineOwner,
    },
    system_protocol_clients::procfs_process_tree::ProcessTreeCpuLoad,
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::docker_engines_group::{DockerEnginesGroup, DockerEnginesMessage},
};
use uom::si::{f64::Ratio, ratio::percent};

use crate::showcased_group::ShowcasedGroup;

pub const ROOTFUL_DOCKER_SOCKET: &str = "/run/docker.sock";
pub const HOVERED_CONTAINER_ID: &str = "postgres";
const ROOTLESS_DOCKER_SOCKET: &str = "/run/user/1000/docker.sock";

fn service_container(
    container_name: &str,
    image: &str,
    busy_share_percent: f64,
    resident_memory: ByteSize,
    clock: Clock,
) -> RunningContainer {
    RunningContainer {
        id: container_name.to_owned(),
        container_name: container_name.to_owned(),
        shown_name: container_name.to_owned(),
        image: image.to_owned(),
        started_at: clock.wall - Duration::from_hours(30),
        ci_identity: ContainerCiIdentity::NotWoodpecker,
        cpu_load: ProcessTreeCpuLoad::BusyShare(Ratio::new::<percent>(busy_share_percent)),
        resident_memory,
    }
}

fn delivered(socket_path: &str, readout: EngineReadout) -> DockerEnginesMessage {
    DockerEnginesMessage::Engine(
        PathBuf::from(socket_path),
        SnapshotDelivery::fixture_delivered(
            Ok(DockerEngineReadout(readout)),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        ),
    )
}

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = DockerEnginesGroup::watching(vec![
        DockerEngineSocket {
            owner: EngineOwner::Root,
            socket_path: PathBuf::from(ROOTFUL_DOCKER_SOCKET),
        },
        DockerEngineSocket {
            owner: EngineOwner::YourUser,
            socket_path: PathBuf::from(ROOTLESS_DOCKER_SOCKET),
        },
    ]);
    let readouts = [
        delivered(
            ROOTFUL_DOCKER_SOCKET,
            EngineReadout::Containers(EngineListing {
                running: EngineContainers {
                    containers: vec![
                        service_container(
                            "grafana",
                            "grafana/grafana:11.3.0",
                            0.8,
                            ByteSize::mib(184),
                            clock,
                        ),
                        service_container(
                            HOVERED_CONTAINER_ID,
                            "postgres:17",
                            2.4,
                            ByteSize::mib(412),
                            clock,
                        ),
                    ],
                },
                stopped: vec![StoppedContainer {
                    id: "migrations".to_owned(),
                    container_name: "migrations".to_owned(),
                    shown_name: "migrations".to_owned(),
                    image: "flyway/flyway:11".to_owned(),
                    stopped_at: clock.wall - Duration::from_hours(2),
                }],
            }),
        ),
        delivered(
            ROOTLESS_DOCKER_SOCKET,
            EngineReadout::Containers(EngineListing {
                running: EngineContainers {
                    containers: Vec::new(),
                },
                stopped: Vec::new(),
            }),
        ),
    ];
    for message in readouts {
        let _ = group.update(message, clock);
    }
    ShowcasedGroup::DockerEngines(Box::new(group))
}
