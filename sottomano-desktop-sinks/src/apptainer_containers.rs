use rustix::process::{Pid, Signal, kill_process};

pub fn stop_apptainer_container(runtime_pid: i32) -> Result<(), String> {
    let Some(runtime_process) = Pid::from_raw(runtime_pid) else {
        return Err(format!("apptainer: no process {runtime_pid}"));
    };
    kill_process(runtime_process, Signal::TERM)
        .map_err(|signal_failure| format!("apptainer runtime {runtime_pid}: {signal_failure}"))
}
