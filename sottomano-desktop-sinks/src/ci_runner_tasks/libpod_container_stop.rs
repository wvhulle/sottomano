use std::path::Path;

use futures::future::join_all;

use crate::container_power::{ContainerEngineApi, stop_container};

pub async fn stop_task_containers(
    engine_socket: &Path,
    container_ids: &[String],
) -> Result<(), String> {
    let refusals: Vec<String> = join_all(container_ids.iter().map(|container_id| async move {
        stop_container(ContainerEngineApi::Libpod, engine_socket, container_id)
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
