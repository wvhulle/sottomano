use std::{path::PathBuf, sync::LazyLock, time::Duration};

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    podman_container_engine::libpod_running_containers::{
        ContainerCiIdentity, ContainerCpuLoad, EngineContainers, EngineReadError, EngineReadout,
        RunningContainer,
    },
};
use sottomano_toml_config::widget_groups::podman_engines::{
    PodmanEngineSocket, PodmanEnginesConfig,
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::podman_engines_group::{PodmanEnginesGroup, PodmanEnginesMessage},
};

use crate::showcased_group::ShowcasedGroup;

const SESSION_ENGINE: &str = "Session";
const WOODPECKER_ENGINE: &str = "Woodpecker";
const FORGEJO_ENGINE: &str = "Forgejo";

pub static SHOWN_ENGINES: LazyLock<PodmanEnginesConfig> = LazyLock::new(|| PodmanEnginesConfig {
    engines: [SESSION_ENGINE, WOODPECKER_ENGINE, FORGEJO_ENGINE]
        .map(|label| PodmanEngineSocket {
            label: label.to_owned(),
            socket: PathBuf::from("podman.sock"),
        })
        .into(),
});

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
        cpu_load: ContainerCpuLoad::BusyCores(busy_cores),
        resident_memory,
    }
}

fn delivered(engine_label: &str, readout: EngineReadout) -> PodmanEnginesMessage {
    PodmanEnginesMessage::Engine(
        engine_label.to_owned(),
        SnapshotDelivery::fixture_delivered(
            Ok(readout),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        ),
    )
}

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = PodmanEnginesGroup::default();
    [
        delivered(
            SESSION_ENGINE,
            EngineReadout::Containers(EngineContainers {
                containers: Vec::new(),
            }),
        ),
        delivered(
            WOODPECKER_ENGINE,
            EngineReadout::Containers(EngineContainers {
                containers: vec![
                    step_container("run-tests", 6.2, ByteSize::gib(4), clock),
                    step_container("clone", 0.4, ByteSize::mib(120), clock),
                ],
            }),
        ),
        delivered(
            FORGEJO_ENGINE,
            EngineReadout::Unreadable(EngineReadError::SocketDenied),
        ),
    ]
    .into_iter()
    .for_each(|message| group.update(message, clock));
    ShowcasedGroup::PodmanEngines(Box::new(group))
}
