use std::{ffi::OsStr, io::ErrorKind};

use sottomano_desktop_sources::system_protocol_clients::shell_command_output::{
    self, shown_process_text,
};
use tokio::process::Command;

pub(crate) async fn run_to_success(
    program: &str,
    arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> Result<(), shell_command_output::Error> {
    let finished = match Command::new(program).args(arguments).output().await {
        Err(launch_failure) if launch_failure.kind() == ErrorKind::NotFound => {
            return Err(shell_command_output::Error::NotInstalled {
                program: program.to_owned(),
            });
        }
        Err(launch_failure) => {
            return Err(shell_command_output::Error::Spawn {
                program: program.to_owned(),
                detail: launch_failure.to_string(),
            });
        }
        Ok(finished) => finished,
    };
    if finished.status.success() {
        Ok(())
    } else {
        Err(shell_command_output::Error::Failed {
            program: program.to_owned(),
            stderr: shown_process_text(&finished.stderr).trim().to_owned(),
        })
    }
}
