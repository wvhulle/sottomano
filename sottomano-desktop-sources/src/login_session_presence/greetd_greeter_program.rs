use std::{fs, path::Path};

use procfs::process::Process as Proc;
use serde::Deserialize;

const GREETD_DEFAULT_CONFIG: &str = "/etc/greetd/config.toml";

#[derive(Deserialize)]
struct GreetdConfig {
    default_session: GreeterSession,
}

#[derive(Deserialize)]
struct GreeterSession {
    command: String,
}

pub fn greeter_program(session_worker_pid: u32) -> Option<String> {
    let worker_process_id = i32::try_from(session_worker_pid)
        .inspect_err(|range_failure| {
            tracing::warn!(
                "greeter program: session worker pid {session_worker_pid} out of range: \
                 {range_failure}"
            );
        })
        .ok()?;
    let worker = Proc::new(worker_process_id)
        .inspect_err(|proc_failure| {
            tracing::debug!(
                "greeter program: session worker {session_worker_pid} unreadable: {proc_failure}"
            );
        })
        .ok()?;
    let worker_parent_id = worker
        .stat()
        .inspect_err(|stat_failure| {
            tracing::debug!(
                "greeter program: session worker {session_worker_pid} stat unreadable: \
                 {stat_failure}"
            );
        })
        .ok()?
        .ppid;
    let greetd = Proc::new(worker_parent_id)
        .inspect_err(|proc_failure| {
            tracing::debug!(
                "greeter program: parent {worker_parent_id} unreadable: {proc_failure}"
            );
        })
        .ok()?;
    let greetd_stat = greetd
        .stat()
        .inspect_err(|stat_failure| {
            tracing::debug!(
                "greeter program: parent {worker_parent_id} stat unreadable: {stat_failure}"
            );
        })
        .ok()?;
    if greetd_stat.comm != "greetd" {
        return None;
    }
    let arguments = greetd
        .cmdline()
        .inspect_err(|cmdline_failure| {
            tracing::warn!(
                "greeter program: greetd {worker_parent_id} command line unreadable: \
                 {cmdline_failure}"
            );
        })
        .ok()?;
    let config_path = arguments
        .iter()
        .skip_while(|argument| *argument != "--config")
        .nth(1)
        .map_or(GREETD_DEFAULT_CONFIG, String::as_str);
    let config_text = fs::read_to_string(config_path)
        .inspect_err(|read_failure| {
            tracing::warn!(
                "greeter program: cannot read greetd config {config_path}: {read_failure}"
            );
        })
        .ok()?;
    let config: GreetdConfig = toml::from_str(&config_text)
        .inspect_err(|parse_failure| {
            tracing::warn!("greeter program: invalid greetd config {config_path}: {parse_failure}");
        })
        .ok()?;
    let program = config.default_session.command.split_whitespace().next()?;
    Some(Path::new(program).file_name()?.to_str()?.to_owned())
}
