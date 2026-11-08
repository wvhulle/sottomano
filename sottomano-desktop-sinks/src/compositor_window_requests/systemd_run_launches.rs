use std::{env, process::Command};

use crate::compositor_window_requests::compositor_requests::CompositorRequestFailure;

pub fn launch_in_user_service(command: &[String]) -> Result<(), CompositorRequestFailure> {
    let request = format!("launch {}", command.join(" "));
    let mut systemd_run = Command::new("systemd-run");
    systemd_run.args(["--user", "--quiet", "--collect", "--slice=app.slice"]);
    if let Some(home) = env::home_dir() {
        systemd_run.arg(format!("--working-directory={}", home.display()));
    }
    let output = systemd_run
        .arg("--")
        .args(command)
        .output()
        .map_err(|launch_error| CompositorRequestFailure::Unreachable {
            request: request.clone(),
            reason: format!("systemd-run did not start: {launch_error}"),
        })?;
    if output.status.success() {
        return Ok(());
    }
    Err(CompositorRequestFailure::Rejected {
        request,
        reason: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}
