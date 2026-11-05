use std::{
    env,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

const RUNTIME_DIRECTORY_VARIABLE: &str = "XDG_RUNTIME_DIR";

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PodmanEngineSocket {
    pub label: String,
    pub socket: PathBuf,
}

impl PodmanEngineSocket {
    #[must_use]
    pub fn resolved_socket_path(&self) -> PathBuf {
        resolved_runtime_path(&self.socket)
    }
}

#[must_use]
pub fn resolved_runtime_path(socket: &Path) -> PathBuf {
    if socket.is_absolute() {
        return socket.to_path_buf();
    }
    env::var_os(RUNTIME_DIRECTORY_VARIABLE).map_or_else(
        || socket.to_path_buf(),
        |runtime_directory| PathBuf::from(runtime_directory).join(socket),
    )
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PodmanEnginesConfig {
    pub engines: Vec<PodmanEngineSocket>,
}

impl Default for PodmanEnginesConfig {
    fn default() -> Self {
        Self {
            engines: vec![
                PodmanEngineSocket {
                    label: "Session".to_owned(),
                    socket: PathBuf::from("podman/podman.sock"),
                },
                PodmanEngineSocket {
                    label: "System".to_owned(),
                    socket: PathBuf::from("/run/podman/podman.sock"),
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absolute_socket_is_used_as_written() {
        assert_eq!(
            resolved_runtime_path(Path::new("/run/podman/podman.sock")),
            PathBuf::from("/run/podman/podman.sock")
        );
    }
}
