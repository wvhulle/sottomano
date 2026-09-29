use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    io::ErrorKind,
    process::Output,
};

use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Spawn { program: String, detail: String },
    Failed { program: String, stderr: String },
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Spawn { program, detail } => write!(f, "cannot run {program}: {detail}"),
            Error::Failed { program, stderr } => write!(f, "{program} failed: {stderr}"),
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
    use std::{io::ErrorKind, process::Command};

    use super::{Error, captured};

    pub fn output(program: &str, args: &[&str]) -> Result<Option<String>, Error> {
        match Command::new(program).args(args).output() {
            Ok(output) => captured(program, &output).map(Some),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(Error::Spawn {
                program: program.to_owned(),
                detail: error.to_string(),
            }),
        }
    }

    pub fn run(program: &str, args: &[&str]) -> bool {
        Command::new(program)
            .args(args)
            .status()
            .is_ok_and(|status| status.success())
    }
}

pub async fn output(program: &str, args: &[&str]) -> Result<Option<String>, Error> {
    match Command::new(program).args(args).output().await {
        Ok(output) => captured(program, &output).map(Some),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::Spawn {
            program: program.to_owned(),
            detail: error.to_string(),
        }),
    }
}

pub async fn require(program: &str, args: &[&str]) -> Result<String, Error> {
    output(program, args).await?.ok_or_else(|| Error::Spawn {
        program: program.to_owned(),
        detail: "not installed".to_owned(),
    })
}

pub async fn exists(program: &str) -> bool {
    matches!(
        output("sh", &["-c", &format!("command -v {program}")]).await,
        Ok(Some(_))
    )
}
