use std::path::Path;

use serde_json::json;
use sottomano_desktop_sources::{
    microvm_guests::hypervisor_command_lines::MicrovmHypervisor,
    system_protocol_clients::http_json_client,
};

const CLOUD_HYPERVISOR_POWER_BUTTON_URL: &str = "http://localhost/api/v1/vm.power-button";
const FIRECRACKER_ACTIONS_URL: &str = "http://localhost/actions";

pub async fn press_guest_power_button(
    hypervisor: MicrovmHypervisor,
    api_socket: &Path,
) -> Result<(), String> {
    let pressed = match hypervisor {
        MicrovmHypervisor::CloudHypervisor => {
            http_json_client::put_over_unix_socket(
                api_socket,
                CLOUD_HYPERVISOR_POWER_BUTTON_URL,
                None,
            )
            .await
        }
        MicrovmHypervisor::Firecracker => {
            http_json_client::put_over_unix_socket(
                api_socket,
                FIRECRACKER_ACTIONS_URL,
                Some(&json!({"action_type": "SendCtrlAltDel"})),
            )
            .await
        }
        MicrovmHypervisor::Crosvm | MicrovmHypervisor::Kvmtool => {
            return Err(format!(
                "{} has no power button this shell can press",
                hypervisor.product_name()
            ));
        }
    };
    pressed
        .map(|_| ())
        .map_err(|failure| format!("{}: {failure}", api_socket.display()))
}
