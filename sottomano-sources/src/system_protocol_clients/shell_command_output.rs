use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    io::{self, ErrorKind},
    process::Output,
};

use tokio::process::Command;

const TRANSIENT_SCOPE_PROGRAM: &str = "systemd-run";
const BACKGROUND_SCOPE_ARGS: [&str; 6] = [
    "--user",
    "--scope",
    "--quiet",
    "--collect",
    "--slice=background.slice",
    "--",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    NotInstalled { program: String },
    Spawn { program: String, detail: String },
    Failed { program: String, stderr: String },
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::NotInstalled { program } => write!(f, "{program} is not installed"),
            Error::Spawn { program, detail } => write!(f, "cannot run {program}: {detail}"),
            Error::Failed { program, stderr } => write!(f, "{program} failed: {stderr}"),
        }
    }
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

    #[must_use]
    pub fn run(program: &str, args: &[&str]) -> bool {
        Command::new(program)
            .args(args)
            .status()
            .is_ok_and(|status| status.success())
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

pub async fn exists(program: &str) -> bool {
    output("sh", &["-c", &format!("command -v {program}")])
        .await
        .is_ok()
}
