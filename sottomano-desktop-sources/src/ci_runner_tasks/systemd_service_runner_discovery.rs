use std::path::PathBuf;

use crate::{
    ci_runner_tasks::{
        forgejo_runner_tasks::ForgejoRunnerWatch, woodpecker_agent_tasks::WoodpeckerAgentWatch,
    },
    podman_container_engine::systemd_service_engine_discovery::unix_socket_of_uri,
    systemd_units_journal::{
        ScopedSystemdUnit, SystemdUnitScope, service_commands::ServiceCommand,
    },
};

const WOODPECKER_AGENT_PROGRAM: &str = "woodpecker-agent";
const FORGEJO_RUNNER_PROGRAM: &str = "forgejo-runner";
const FORGEJO_DAEMON_SUBCOMMAND: [&str; 1] = ["daemon"];
const DOCKER_HOST_VARIABLE: &str = "DOCKER_HOST";
const DOCKER_DEFAULT_SOCKET: &str = "/var/run/docker.sock";
const HEALTHCHECK_ADDRESS_VARIABLE: &str = "WOODPECKER_HEALTHCHECK_ADDR";
const HEALTHCHECK_DEFAULT_ADDRESS: &str = ":3000";
const UNSPECIFIED_HOSTS: [&str; 3] = ["", "0.0.0.0", "[::]"];
const LOOPBACK_HOST: &str = "localhost";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiscoveredCiRunner<Watch> {
    pub unit: ScopedSystemdUnit,
    pub label: String,
    pub watch: Watch,
}

fn runner_label(command: &ServiceCommand, program_name: &str) -> String {
    let unit_stem = command.unit_stem();
    unit_stem
        .strip_prefix(program_name)
        .and_then(|suffix| suffix.strip_prefix('-'))
        .filter(|instance_name| !instance_name.is_empty())
        .unwrap_or(unit_stem)
        .to_owned()
}

fn engine_socket(command: &ServiceCommand) -> Option<PathBuf> {
    command.variable(DOCKER_HOST_VARIABLE).map_or_else(
        || Some(PathBuf::from(DOCKER_DEFAULT_SOCKET)),
        unix_socket_of_uri,
    )
}

fn varz_url_of_healthcheck_address(listen_address: &str) -> Option<String> {
    let (host, port) = listen_address.rsplit_once(':')?;
    let reachable_host = if UNSPECIFIED_HOSTS.contains(&host) {
        LOOPBACK_HOST
    } else {
        host
    };
    Some(format!("http://{reachable_host}:{port}/varz"))
}

fn discovered_runner<Watch>(
    scope: SystemdUnitScope,
    command: &ServiceCommand,
    program_name: &str,
    watch: Watch,
) -> DiscoveredCiRunner<Watch> {
    DiscoveredCiRunner {
        unit: ScopedSystemdUnit {
            scope,
            unit_name: command.unit.clone(),
        },
        label: runner_label(command, program_name),
        watch,
    }
}

impl WoodpeckerAgentWatch {
    #[must_use]
    pub fn discovered(
        scope: SystemdUnitScope,
        command: &ServiceCommand,
    ) -> Option<DiscoveredCiRunner<Self>> {
        if command.program_name() != Some(WOODPECKER_AGENT_PROGRAM) {
            return None;
        }
        let watch = WoodpeckerAgentWatch {
            varz_url: varz_url_of_healthcheck_address(
                command
                    .variable(HEALTHCHECK_ADDRESS_VARIABLE)
                    .unwrap_or(HEALTHCHECK_DEFAULT_ADDRESS),
            )?,
            engine_socket: engine_socket(command)?,
        };
        Some(discovered_runner(
            scope,
            command,
            WOODPECKER_AGENT_PROGRAM,
            watch,
        ))
    }
}

impl ForgejoRunnerWatch {
    #[must_use]
    pub fn discovered(
        scope: SystemdUnitScope,
        command: &ServiceCommand,
    ) -> Option<DiscoveredCiRunner<Self>> {
        let runs_daemon = (command.program_name() == Some(FORGEJO_RUNNER_PROGRAM))
            && command
                .arguments_after(&FORGEJO_DAEMON_SUBCOMMAND)
                .is_some();
        if !runs_daemon {
            return None;
        }
        let watch = ForgejoRunnerWatch {
            unit_name: command.unit.clone(),
            engine_socket: engine_socket(command)?,
        };
        Some(discovered_runner(
            scope,
            command,
            FORGEJO_RUNNER_PROGRAM,
            watch,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::systemd_units_journal::service_commands::ServiceAccount;

    fn runner_command(
        unit: &str,
        program_name: &str,
        arguments: &[&str],
        environment: &[(&str, &str)],
    ) -> ServiceCommand {
        ServiceCommand {
            unit: unit.to_owned(),
            program: PathBuf::from("/nix/store/runner/bin").join(program_name),
            arguments: arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect(),
            environment: environment
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect::<BTreeMap<_, _>>(),
            account: ServiceAccount::NamedUser("ci".to_owned()),
        }
    }

    #[test]
    fn a_forgejo_daemon_uses_the_engine_named_in_its_docker_host() {
        let runner = ForgejoRunnerWatch::discovered(
            SystemdUnitScope::SystemWide,
            &runner_command(
                "forgejo-runner-codeberg.service",
                FORGEJO_RUNNER_PROGRAM,
                &["daemon", "--config", "/etc/forgejo-runner.yaml"],
                &[(
                    DOCKER_HOST_VARIABLE,
                    "unix:///run/forgejo-runner-codeberg/podman.sock",
                )],
            ),
        )
        .expect("a forgejo daemon is a runner");
        assert_eq!(runner.label, "codeberg");
        assert_eq!(
            runner.watch.engine_socket,
            PathBuf::from("/run/forgejo-runner-codeberg/podman.sock")
        );
        assert_eq!(runner.watch.unit_name, "forgejo-runner-codeberg.service");
    }

    #[test]
    fn registering_a_forgejo_runner_is_not_a_runner() {
        assert!(
            ForgejoRunnerWatch::discovered(
                SystemdUnitScope::SystemWide,
                &runner_command(
                    "forgejo-runner-register.service",
                    FORGEJO_RUNNER_PROGRAM,
                    &["register"],
                    &[],
                ),
            )
            .is_none()
        );
    }

    #[test]
    fn a_woodpecker_agent_without_settings_uses_its_defaults() {
        let agent = WoodpeckerAgentWatch::discovered(
            SystemdUnitScope::SystemWide,
            &runner_command(
                "woodpecker-agent.service",
                WOODPECKER_AGENT_PROGRAM,
                &[],
                &[],
            ),
        )
        .expect("a woodpecker agent is a runner");
        assert_eq!(agent.label, "woodpecker-agent");
        assert_eq!(agent.watch.varz_url, "http://localhost:3000/varz");
        assert_eq!(
            agent.watch.engine_socket,
            PathBuf::from(DOCKER_DEFAULT_SOCKET)
        );
    }

    #[test]
    fn a_healthcheck_on_a_named_host_is_reached_there() {
        assert_eq!(
            varz_url_of_healthcheck_address("127.0.0.1:3001").as_deref(),
            Some("http://127.0.0.1:3001/varz")
        );
    }

    #[test]
    fn a_runner_on_a_tcp_engine_is_not_watched() {
        assert!(
            WoodpeckerAgentWatch::discovered(
                SystemdUnitScope::SystemWide,
                &runner_command(
                    "woodpecker-agent-remote.service",
                    WOODPECKER_AGENT_PROGRAM,
                    &[],
                    &[(DOCKER_HOST_VARIABLE, "tcp://10.0.0.2:2375")],
                ),
            )
            .is_none()
        );
    }
}
