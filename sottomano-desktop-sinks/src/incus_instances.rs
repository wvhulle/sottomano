use serde_json::json;
use sottomano_desktop_sources::{
    incus_instances::{INCUS_API_ROOT, InstanceDaemon},
    system_protocol_clients::http_json_client,
};

const STATE_CHANGE_TIMEOUT_SECONDS: u32 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceStateChange {
    Start,
    Stop,
}

impl InstanceStateChange {
    const fn api_action(self) -> &'static str {
        match self {
            InstanceStateChange::Start => "start",
            InstanceStateChange::Stop => "stop",
        }
    }
}

pub async fn change_instance_state(
    instance_daemon: InstanceDaemon,
    project_name: &str,
    instance_name: &str,
    state_change: InstanceStateChange,
) -> Result<(), String> {
    let state_url =
        format!("{INCUS_API_ROOT}/instances/{instance_name}/state?project={project_name}");
    http_json_client::put_over_unix_socket(
        &instance_daemon.socket_path(),
        &state_url,
        Some(&json!({
            "action": state_change.api_action(),
            "timeout": STATE_CHANGE_TIMEOUT_SECONDS,
        })),
    )
    .await
    .map(|_| ())
    .map_err(|failure| format!("{instance_name}: {failure}"))
}
