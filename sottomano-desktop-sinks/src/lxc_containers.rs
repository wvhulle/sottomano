use sottomano_desktop_sources::lxc_containers::DEFAULT_LXC_PATH;

use crate::finished_commands::run_to_success;

pub async fn stop_lxc_container(container_name: &str) -> Result<(), String> {
    run_to_success("lxc-stop", ["-P", DEFAULT_LXC_PATH, "-n", container_name]).await
}

pub async fn start_lxc_container(container_name: &str) -> Result<(), String> {
    run_to_success("lxc-start", ["-P", DEFAULT_LXC_PATH, "-n", container_name]).await
}
