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
    let worker = Proc::new(i32::try_from(session_worker_pid).ok()?).ok()?;
    let greetd = Proc::new(worker.stat().ok()?.ppid).ok()?;
    if greetd.stat().ok()?.comm != "greetd" {
        return None;
    }
    let arguments = greetd.cmdline().ok()?;
    let config_path = arguments
        .iter()
        .skip_while(|argument| *argument != "--config")
        .nth(1)
        .map_or(GREETD_DEFAULT_CONFIG, String::as_str);
    let config: GreetdConfig = toml::from_str(&fs::read_to_string(config_path).ok()?).ok()?;
    let program = config.default_session.command.split_whitespace().next()?;
    Some(Path::new(program).file_name()?.to_str()?.to_owned())
}
