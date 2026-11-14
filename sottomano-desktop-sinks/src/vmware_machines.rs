use std::{ffi::OsStr, path::Path};

use crate::finished_commands::run_to_success;

pub async fn stop_vmware_machine(configuration_file: &Path) -> Result<(), String> {
    run_to_success(
        "vmrun",
        [
            OsStr::new("stop"),
            configuration_file.as_os_str(),
            OsStr::new("soft"),
        ],
    )
    .await
}
