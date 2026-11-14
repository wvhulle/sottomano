use crate::finished_commands::run_to_success;

pub async fn stop_flatpak_instance(instance_id: &str) -> Result<(), String> {
    run_to_success("flatpak", ["kill", instance_id]).await
}
