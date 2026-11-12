use std::{
    io::{self, ErrorKind},
    process::Output,
};

use tokio::process::Command;

use crate::backing_service_availability::Availability;

const TRANSIENT_SCOPE_PROGRAM: &str = "systemd-run";
const BACKGROUND_SCOPE_ARGS: [&str; 6] = [
    "--user",
    "--scope",
    "--quiet",
    "--collect",
    "--slice=background.slice",
    "--",
];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("{program} is not installed")]
    NotInstalled { program: String },
    #[error("cannot run {program}: {detail}")]
    Spawn { program: String, detail: String },
    #[error("{program} failed: {stderr}")]
    Failed { program: String, stderr: String },
    #[error("{program} exited with {exit_status}")]
    Exited {
        program: String,
        exit_status: String,
    },
}

fn spawn_failure(program: &str, error: &io::Error) -> Error {
    if error.kind() == ErrorKind::NotFound {
        Error::NotInstalled {
            program: program.to_owned(),
        }
    } else {
        Error::Spawn {
            program: program.to_owned(),
            detail: error.to_string(),
        }
    }
}

fn captured(program: &str, output: &Output) -> Result<String, Error> {
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(Error::Failed {
        program: program.to_owned(),
        stderr: stderr.trim().chars().take(200).collect(),
    })
}

pub mod blocking {
    use std::process::Command;

    use super::{Error, captured, spawn_failure};

    pub fn output(program: &str, args: &[&str]) -> Result<String, Error> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|error| spawn_failure(program, &error))?;
        captured(program, &output)
    }

    pub fn run(program: &str, args: &[&str]) -> Result<(), Error> {
        let exit_status = Command::new(program)
            .args(args)
            .status()
            .map_err(|error| spawn_failure(program, &error))?;
        if exit_status.success() {
            Ok(())
        } else {
            Err(Error::Exited {
                program: program.to_owned(),
                exit_status: exit_status.to_string(),
            })
        }
    }
}

pub async fn output(program: &str, args: &[&str]) -> Result<String, Error> {
    let output = Command::new(program)
        .args(args)
        .output()
        .await
        .map_err(|error| spawn_failure(program, &error))?;
    captured(program, &output)
}

pub async fn background_output(program: &str, args: &[&str]) -> Result<String, Error> {
    let output = Command::new(TRANSIENT_SCOPE_PROGRAM)
        .args(BACKGROUND_SCOPE_ARGS)
        .arg(program)
        .args(args)
        .output()
        .await
        .map_err(|error| spawn_failure(TRANSIENT_SCOPE_PROGRAM, &error))?;
    captured(program, &output)
}

pub async fn program_availability(program: &str) -> Availability {
    match output("sh", &["-c", &format!("command -v {program}")]).await {
        Ok(_) => Availability::Available,
        Err(_) => Availability::Unavailable,
    }
}
