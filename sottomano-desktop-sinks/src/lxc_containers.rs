use sottomano_desktop_sources::{container_run_state::RunChange, lxc_containers::DEFAULT_LXC_PATH};

use crate::finished_commands::run_to_success;

pub async fn change_lxc_container(
    container_name: &str,
    run_change: RunChange,
) -> Result<(), String> {
    let lxc_program = match run_change {
        RunChange::Start => "lxc-start",
        RunChange::Stop => "lxc-stop",
    };
    run_to_success(lxc_program, ["-P", DEFAULT_LXC_PATH, "-n", container_name]).await
}
