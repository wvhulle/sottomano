use std::{
    env,
    fmt::{Display, Formatter, Result as FmtResult},
    path::PathBuf,
};

use crate::systemd_units_journal::{
    ScopedSystemdUnit, SystemdUnitScope,
    service_commands::{ServiceAccount, ServiceCommand},
};

const PODMAN_PROGRAM: &str = "podman";
const SERVICE_SUBCOMMAND: [&str; 2] = ["system", "service"];
const UNIX_SCHEME: &str = "unix:";
const ROOTFUL_DEFAULT_SOCKET: &str = "/run/podman/podman.sock";
const ROOTLESS_DEFAULT_SOCKET: &str = "podman/podman.sock";
const RUNTIME_DIRECTORY_VARIABLE: &str = "XDG_RUNTIME_DIR";
const ROOT_ACCOUNT: &str = "root";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EngineOwner {
    YourUser,
    Root,
    ServiceUser(String),
}

impl Display for EngineOwner {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            EngineOwner::YourUser => f.write_str("Your user"),
            EngineOwner::Root => f.write_str("Root"),
            EngineOwner::ServiceUser(user_name) => f.write_str(user_name),
        }
    }
}

impl EngineOwner {
    fn of_service(scope: SystemdUnitScope, account: &ServiceAccount) -> Self {
        match (scope, account) {
            (SystemdUnitScope::UserSession, _) => EngineOwner::YourUser,
            (SystemdUnitScope::SystemWide, ServiceAccount::ManagerOwner) => EngineOwner::Root,
            (SystemdUnitScope::SystemWide, ServiceAccount::NamedUser(user_name))
                if user_name == ROOT_ACCOUNT =>
            {
                EngineOwner::Root
            }
            (SystemdUnitScope::SystemWide, ServiceAccount::NamedUser(user_name)) => {
                EngineOwner::ServiceUser(user_name.clone())
            }
        }
    }

    fn default_socket(&self) -> Option<PathBuf> {
        match self {
            EngineOwner::YourUser => {
                env::var_os(RUNTIME_DIRECTORY_VARIABLE).map(|runtime_directory| {
                    PathBuf::from(runtime_directory).join(ROOTLESS_DEFAULT_SOCKET)
                })
            }
            EngineOwner::Root => Some(PathBuf::from(ROOTFUL_DEFAULT_SOCKET)),
            EngineOwner::ServiceUser(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiscoveredPodmanEngine {
    pub unit: ScopedSystemdUnit,
    pub owner: EngineOwner,
    pub socket_path: PathBuf,
}

impl DiscoveredPodmanEngine {
    #[must_use]
    pub fn discovered(scope: SystemdUnitScope, command: &ServiceCommand) -> Option<Self> {
        if !command.runs(PODMAN_PROGRAM) {
            return None;
        }
        let service_arguments: Vec<&str> = command.arguments_after(&SERVICE_SUBCOMMAND)?.collect();
        let owner = EngineOwner::of_service(scope, &command.account);
        let socket_path = match service_arguments
            .iter()
            .find(|argument| argument.contains(':'))
        {
            Some(listen_uri) => unix_socket_of_uri(listen_uri)?,
            None => owner.default_socket()?,
        };
        Some(Self {
            unit: ScopedSystemdUnit {
                scope,
                unit_name: command.unit.clone(),
            },
            owner,
            socket_path,
        })
    }
}

#[must_use]
pub fn unix_socket_of_uri(uri: &str) -> Option<PathBuf> {
    let slashed_path = uri.strip_prefix(UNIX_SCHEME)?;
    Some(PathBuf::from("/").join(slashed_path.trim_start_matches('/')))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn podman_command(arguments: &[&str], account: ServiceAccount) -> ServiceCommand {
        ServiceCommand {
            unit: "forgejo-runner-codeberg-podman.service".to_owned(),
            program: PathBuf::from("/nix/store/podman-6.1.4/bin/podman"),
            arguments: arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect(),
            environment: BTreeMap::new(),
            account,
        }
    }

    #[test]
    fn a_ci_users_engine_is_found_at_its_listen_uri() {
        let engine = DiscoveredPodmanEngine::discovered(
            SystemdUnitScope::SystemWide,
            &podman_command(
                &[
                    "--cgroup-manager=cgroupfs",
                    "system",
                    "service",
                    "--time=0",
                    "unix:/run/forgejo-runner-codeberg/podman.sock",
                ],
                ServiceAccount::NamedUser("forgejo-runner-codeberg".to_owned()),
            ),
        )
        .expect("a podman service is an engine");
        assert_eq!(
            engine.socket_path,
            PathBuf::from("/run/forgejo-runner-codeberg/podman.sock")
        );
        assert_eq!(engine.owner.to_string(), "forgejo-runner-codeberg");
    }

    #[test]
    fn a_socket_activated_root_engine_listens_on_the_rootful_default() {
        let engine = DiscoveredPodmanEngine::discovered(
            SystemdUnitScope::SystemWide,
            &podman_command(
                &["$LOGGING", "system", "service"],
                ServiceAccount::ManagerOwner,
            ),
        )
        .expect("a podman service is an engine");
        assert_eq!(engine.socket_path, PathBuf::from(ROOTFUL_DEFAULT_SOCKET));
        assert_eq!(engine.owner, EngineOwner::Root);
    }

    #[test]
    fn other_podman_commands_and_tcp_listeners_are_not_engines() {
        assert!(
            DiscoveredPodmanEngine::discovered(
                SystemdUnitScope::SystemWide,
                &podman_command(&["auto-update"], ServiceAccount::ManagerOwner),
            )
            .is_none()
        );
        assert!(
            DiscoveredPodmanEngine::discovered(
                SystemdUnitScope::SystemWide,
                &podman_command(
                    &["system", "service", "tcp://localhost:8080"],
                    ServiceAccount::ManagerOwner
                ),
            )
            .is_none()
        );
    }

    #[test]
    fn both_unix_uri_spellings_name_the_same_socket() {
        assert_eq!(
            unix_socket_of_uri("unix:///run/podman/podman.sock"),
            unix_socket_of_uri("unix:/run/podman/podman.sock")
        );
    }
}
