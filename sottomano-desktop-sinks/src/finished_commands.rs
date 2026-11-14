use std::ffi::OsStr;

use sottomano_desktop_sources::system_protocol_clients::shell_command_output::shown_process_text;
use tokio::process::Command;

pub(crate) async fn run_to_success(
    program: &str,
    arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> Result<(), String> {
    let finished = Command::new(program)
        .args(arguments)
        .output()
        .await
        .map_err(|launch_failure| format!("{program}: {launch_failure}"))?;
    if finished.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{program}: {}",
            shown_process_text(&finished.stderr).trim()
        ))
    }
}
