use std::path::Path;

use sottomano_desktop_sources::{
    docker_container_engine::DOCKER_API_ROOT,
    podman_container_engine::libpod_running_containers::LIBPOD_API_ROOT,
    system_protocol_clients::http_json_client,
};

const NOT_FOUND_STATUS: u16 = 404;
const STOP_GRACE_SECONDS: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerEngineApi {
    Libpod,
    Docker,
}

impl ContainerEngineApi {
    const fn api_root(self) -> &'static str {
        match self {
            ContainerEngineApi::Libpod => LIBPOD_API_ROOT,
            ContainerEngineApi::Docker => DOCKER_API_ROOT,
        }
    }

    const fn grace_parameter(self) -> &'static str {
        match self {
            ContainerEngineApi::Libpod => "timeout",
            ContainerEngineApi::Docker => "t",
        }
    }
}

pub async fn stop_container(
    engine_api: ContainerEngineApi,
    engine_socket: &Path,
    container_id: &str,
) -> Result<(), http_json_client::Error> {
    let stop_url = format!(
        "{}/containers/{container_id}/stop?{}={STOP_GRACE_SECONDS}",
        engine_api.api_root(),
        engine_api.grace_parameter()
    );
    match http_json_client::post_over_unix_socket(engine_socket, &stop_url).await {
        Ok(_)
        | Err(http_json_client::Error::Status {
            status: NOT_FOUND_STATUS,
            ..
        }) => Ok(()),
        Err(failure) => Err(failure),
    }
}

pub async fn start_container(
    engine_api: ContainerEngineApi,
    engine_socket: &Path,
    container_id: &str,
) -> Result<(), http_json_client::Error> {
    let start_url = format!("{}/containers/{container_id}/start", engine_api.api_root());
    http_json_client::post_over_unix_socket(engine_socket, &start_url)
        .await
        .map(drop)
}
