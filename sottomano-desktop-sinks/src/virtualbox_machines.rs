use crate::finished_commands::run_to_success;

pub async fn press_virtualbox_power_button(machine_reference: &str) -> Result<(), String> {
    run_to_success(
        "VBoxManage",
        ["controlvm", machine_reference, "acpipowerbutton"],
    )
    .await
}

pub async fn start_virtualbox_machine(machine_uuid: &str) -> Result<(), String> {
    run_to_success("VBoxManage", ["startvm", machine_uuid]).await
}
