use tokio::process::Command;

const VIRTUALBOX_MANAGER_PROGRAM: &str = "VBoxManage";

pub async fn press_virtualbox_power_button(machine_reference: &str) -> Result<(), String> {
    let pressed = Command::new(VIRTUALBOX_MANAGER_PROGRAM)
        .args(["controlvm", machine_reference, "acpipowerbutton"])
        .output()
        .await
        .map_err(|launch_failure| format!("{VIRTUALBOX_MANAGER_PROGRAM}: {launch_failure}"))?;
    if pressed.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{VIRTUALBOX_MANAGER_PROGRAM} controlvm {machine_reference}: {}",
            String::from_utf8_lossy(&pressed.stderr).trim()
        ))
    }
}
