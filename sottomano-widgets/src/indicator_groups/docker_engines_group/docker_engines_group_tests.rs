use std::time::{Duration, UNIX_EPOCH};

use sottomano_desktop_sources::{
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    podman_container_engine::{
        libpod_running_containers::{
            EngineContainers, EngineListing, EngineReadError, StoppedContainer,
        },
        systemd_service_engine_discovery::EngineOwner,
    },
};

use super::*;
use crate::{
    hover_cards::{HoverTarget, IndicatorKey},
    indicator_groups::indicator_group_identity::IndicatorGroupIdentity,
};

fn delivered(
    engine_read: Result<EngineListing, EngineReadError>,
) -> SnapshotDelivery<DockerEngineReadout> {
    SnapshotDelivery::fixture_delivered(
        engine_read.map(|engine_listing| DockerEngineReadout { engine_listing }),
        DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
    )
}

const ROOTFUL_SOCKET_PATH: &str = "/run/docker.sock";

fn rootful_engine_group() -> DockerEnginesGroup {
    DockerEnginesGroup::watching(vec![DockerEngineSocket {
        owner: EngineOwner::Root,
        socket_path: PathBuf::from(ROOTFUL_SOCKET_PATH),
    }])
}

#[test]
fn a_denied_docker_socket_shows_a_failure_indicator() {
    let rootful_socket = PathBuf::from(ROOTFUL_SOCKET_PATH);
    let mut group = rootful_engine_group();
    let _ = group.update(
        DockerEnginesMessage::Engine(
            rootful_socket.clone(),
            delivered(Err(EngineReadError::SocketDenied)),
        ),
        Clock::real(),
    );
    assert_eq!(group.container_indicators().len(), 1);
    let failure_indicator = HoverTarget::Indicator(IndicatorKey {
        group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::DockerEngines),
        name: group.container_indicators()[0].indicator_name(),
    });
    assert!(
        group
            .group_content()
            .hover_card(&failure_indicator, Clock::real())
            .is_some_and(|card| !card.sections.is_empty())
    );
}

#[test]
fn a_missing_docker_socket_shows_no_indicator() {
    let rootful_socket = PathBuf::from(ROOTFUL_SOCKET_PATH);
    let mut group = rootful_engine_group();
    let _ = group.update(
        DockerEnginesMessage::Engine(rootful_socket.clone(), SnapshotDelivery::SourceUnavailable),
        Clock::real(),
    );
    assert!(group.group_content().listing.iter().next().is_none());
    let _ = group.update(
        DockerEnginesMessage::Engine(
            rootful_socket,
            delivered(Ok(EngineListing {
                running: EngineContainers {
                    containers: Vec::new(),
                },
                stopped: vec![StoppedContainer {
                    id: "exited".to_owned(),
                    container_name: "exited".to_owned(),
                    shown_name: "exited".to_owned(),
                    image: "alpine".to_owned(),
                    stopped_at: UNIX_EPOCH,
                }],
            })),
        ),
        Clock::real(),
    );
    assert!(group.group_content().listing.iter().next().is_some());
}
