use std::path::Path;

use futures::{StreamExt, stream};

use crate::{
    container_power::{ContainerEngineApi, stop_container},
    sink_write_errors::GuestPowerError,
};

const CONTAINER_STOPS_IN_FLIGHT: usize = 8;

pub async fn stop_task_containers(
    engine_socket: &Path,
    container_ids: &[String],
) -> Result<(), GuestPowerError> {
    let stop_outcomes: Vec<_> = stream::iter(container_ids.to_vec())
        .map(|container_id| {
            let engine_socket = engine_socket.to_path_buf();
            async move {
                stop_container(ContainerEngineApi::Libpod, &engine_socket, &container_id).await
            }
        })
        .buffer_unordered(CONTAINER_STOPS_IN_FLIGHT)
        .collect()
        .await;
    let refusals: Vec<GuestPowerError> =
        stop_outcomes.into_iter().filter_map(Result::err).collect();
    if refusals.is_empty() {
        Ok(())
    } else {
        Err(GuestPowerError::SeveralRefused(refusals))
    }
}
