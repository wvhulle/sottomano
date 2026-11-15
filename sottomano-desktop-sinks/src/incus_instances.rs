use serde_json::json;
use sottomano_desktop_sources::{
    container_run_state::RunChange,
    incus_instances::{INCUS_API_ROOT, InstanceDaemon},
    system_protocol_clients::http_json_client,
};

use crate::sink_write_errors::GuestPowerError;

const STATE_CHANGE_TIMEOUT_SECONDS: u32 = 30;

const fn incus_state_action(run_change: RunChange) -> &'static str {
    match run_change {
        RunChange::Start => "start",
        RunChange::Stop => "stop",
    }
}

pub async fn change_instance_state(
    instance_daemon: InstanceDaemon,
    project_name: &str,
    instance_name: &str,
    run_change: RunChange,
) -> Result<(), GuestPowerError> {
    let state_url =
        format!("{INCUS_API_ROOT}/instances/{instance_name}/state?project={project_name}");
    http_json_client::put_over_unix_socket(
        &instance_daemon.socket_path(),
        &state_url,
        Some(&json!({
            "action": incus_state_action(run_change),
            "timeout": STATE_CHANGE_TIMEOUT_SECONDS,
        })),
    )
    .await
    .map(|_| ())
    .map_err(|source| GuestPowerError::Http {
        target: instance_name.to_owned(),
        source,
    })
}
