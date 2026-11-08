use std::path::Path;

use futures::future::join_all;
use sottomano_desktop_sources::{
    podman_container_engine::libpod_running_containers::LIBPOD_API_ROOT,
    system_protocol_clients::http_json_client,
};

const NOT_FOUND_STATUS: u16 = 404;
const STOP_GRACE_SECONDS: u32 = 10;

async fn stop_container(
    engine_socket: &Path,
    container_id: &str,
) -> Result<(), http_json_client::Error> {
    let stop_url =
        format!("{LIBPOD_API_ROOT}/containers/{container_id}/stop?timeout={STOP_GRACE_SECONDS}");
    match http_json_client::post_over_unix_socket(engine_socket, &stop_url).await {
        Ok(_)
        | Err(http_json_client::Error::Status {
            status: NOT_FOUND_STATUS,
            ..
        }) => Ok(()),
        Err(failure) => Err(failure),
    }
}

pub async fn stop_task_containers(
    engine_socket: &Path,
    container_ids: &[String],
) -> Result<(), String> {
    let refusals: Vec<String> = join_all(container_ids.iter().map(|container_id| async move {
        stop_container(engine_socket, container_id)
            .await
            .map_err(|failure| format!("{container_id}: {failure}"))
    }))
    .await
    .into_iter()
    .filter_map(Result::err)
    .collect();
    match refusals.as_slice() {
        [] => Ok(()),
        _ => Err(refusals.join("; ")),
    }
}
