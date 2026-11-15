use rustix::process::{Pid, Signal, kill_process};

use crate::sink_write_errors::GuestPowerError;

pub fn stop_apptainer_container(runtime_pid: i32) -> Result<(), GuestPowerError> {
    let Some(runtime_process) = Pid::from_raw(runtime_pid) else {
        return Err(GuestPowerError::NoSuchProcess(runtime_pid));
    };
    kill_process(runtime_process, Signal::TERM)
        .map_err(|errno| GuestPowerError::Signal { runtime_pid, errno })
}
