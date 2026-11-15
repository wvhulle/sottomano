use crate::{finished_commands::run_to_success, sink_write_errors::GuestPowerError};

pub async fn stop_flatpak_instance(instance_id: &str) -> Result<(), GuestPowerError> {
    Ok(run_to_success("flatpak", ["kill", instance_id]).await?)
}
