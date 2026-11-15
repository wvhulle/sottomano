use std::path::Path;

use futures::future::join_all;

use crate::{
    container_power::{ContainerEngineApi, stop_container},
    sink_write_errors::GuestPowerError,
};

pub async fn stop_task_containers(
    engine_socket: &Path,
    container_ids: &[String],
) -> Result<(), GuestPowerError> {
    let refusals: Vec<GuestPowerError> = join_all(container_ids.iter().map(|container_id| {
        stop_container(ContainerEngineApi::Libpod, engine_socket, container_id)
    }))
    .await
    .into_iter()
    .filter_map(Result::err)
    .collect();
    if refusals.is_empty() {
        Ok(())
    } else {
        Err(GuestPowerError::SeveralRefused(refusals))
    }
}
