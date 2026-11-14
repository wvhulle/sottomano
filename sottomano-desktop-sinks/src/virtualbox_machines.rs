use crate::finished_commands::run_to_success;

pub async fn press_virtualbox_power_button(machine_reference: &str) -> Result<(), String> {
    run_to_success(
        "VBoxManage",
        ["controlvm", machine_reference, "acpipowerbutton"],
    )
    .await
}
