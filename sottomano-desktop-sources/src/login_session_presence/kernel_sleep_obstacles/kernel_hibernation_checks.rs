use std::{ffi::OsStr, fs, io, os::unix::ffi::OsStrExt};

use sottomano_compositors::compositor_connection::compositor_window_pids;
use tokio::{process::Command, task::spawn_blocking};

use super::{KernelHibernation, SecretMemoryHolder, WindowOwnership};
use crate::{
    desktop_windows_apps::process_tree_inspection::{CgroupScope, launch},
    login_session_presence::cgroup_inhibitor_app_names::InhibitorAppNames,
    non_empty_text::non_empty,
};

const HIBERNATION_MODES_PATH: &str = "/sys/power/disk";

const KERNEL_COMMAND_LINE_PATH: &str = "/proc/cmdline";

const KERNEL_LOCKDOWN_PATH: &str = "/sys/kernel/security/lockdown";

const SECRET_MEMORY_HELPER_PATH: &str = "/run/wrappers/bin/sottomano-secret-memory-holders";

pub(super) fn read_kernel_text(path: &str) -> String {
    match fs::read_to_string(path) {
        Ok(kernel_text) => kernel_text,
        Err(read_failure) if read_failure.kind() == io::ErrorKind::NotFound => String::new(),
        Err(read_failure) => {
            tracing::warn!("sleep obstacles: {path} is unreadable: {read_failure}");
            String::new()
        }
    }
}

pub(super) async fn read_kernel_hibernation(app_names: &InhibitorAppNames) -> KernelHibernation {
    if HibernationModes::from(read_kernel_text(HIBERNATION_MODES_PATH).as_str())
        == HibernationModes::Offered
    {
        return KernelHibernation::Allowed;
    }
    if let Some(flag) = disabling_boot_flag(&read_kernel_text(KERNEL_COMMAND_LINE_PATH)) {
        return KernelHibernation::DisabledByBootFlag(flag);
    }
    if let Some(level) = active_lockdown_level(&read_kernel_text(KERNEL_LOCKDOWN_PATH)) {
        return KernelHibernation::DisabledByLockdown(level);
    }
    match secret_memory_scan().await {
        SecretMemoryScan::Unavailable => KernelHibernation::DisabledWithoutInspection,
        SecretMemoryScan::Scanned(pids) if pids.is_empty() => {
            KernelHibernation::DisabledForUnidentifiedCause
        }
        SecretMemoryScan::Scanned(pids) => KernelHibernation::DisabledBySecretMemory(
            secret_memory_holders(&pids, app_names, &open_window_owners().await),
        ),
    }
}

enum SecretMemoryScan {
    Scanned(Vec<u32>),
    Unavailable,
}

async fn secret_memory_scan() -> SecretMemoryScan {
    match Command::new(SECRET_MEMORY_HELPER_PATH).output().await {
        Ok(output) if output.status.success() => match String::from_utf8(output.stdout) {
            Ok(helper_output) => SecretMemoryScan::Scanned(pids_in_helper_output(&helper_output)),
            Err(encoding_failure) => {
                tracing::warn!(
                    "sleep obstacles: secret memory helper output is not utf-8: {encoding_failure}"
                );
                SecretMemoryScan::Unavailable
            }
        },
        Ok(failed_output) => {
            tracing::warn!(
                "sleep obstacles: secret memory helper failed with {}: {}",
                failed_output.status,
                OsStr::from_bytes(&failed_output.stderr)
                    .display()
                    .to_string()
                    .trim()
            );
            SecretMemoryScan::Unavailable
        }
        Err(launch_failure) if launch_failure.kind() == io::ErrorKind::NotFound => {
            tracing::debug!("sleep obstacles: secret memory helper is not installed");
            SecretMemoryScan::Unavailable
        }
        Err(launch_failure) => {
            tracing::warn!("sleep obstacles: secret memory helper did not start: {launch_failure}");
            SecretMemoryScan::Unavailable
        }
    }
}

fn secret_memory_holders(
    pids: &[u32],
    app_names: &InhibitorAppNames,
    window_owners: &CompositorWindows,
) -> Vec<SecretMemoryHolder> {
    let mut holders: Vec<SecretMemoryHolder> = pids
        .iter()
        .map(|pid| SecretMemoryHolder {
            name: app_names
                .process_app_name(*pid)
                .unwrap_or_else(|| format!("Process {pid}")),
            pid: *pid,
            window_ownership: window_owners.window_ownership(*pid),
        })
        .collect();
    holders.sort_by(|earlier, later| {
        (earlier.name.as_str(), earlier.pid).cmp(&(later.name.as_str(), later.pid))
    });
    holders
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CompositorWindows {
    Listed(WindowOwners),
    Unavailable,
}

impl CompositorWindows {
    fn window_ownership(&self, pid: u32) -> WindowOwnership {
        match self {
            Self::Listed(owners) => owners.window_ownership(pid, process_scope(pid).as_deref()),
            Self::Unavailable => WindowOwnership::OwnsWindow,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct WindowOwners {
    window_pids: Vec<u32>,
    window_scopes: Vec<String>,
}

impl WindowOwners {
    fn window_ownership(&self, pid: u32, scope: Option<&str>) -> WindowOwnership {
        let owns = self.window_pids.contains(&pid)
            || scope.is_some_and(|scope| self.window_scopes.iter().any(|owned| owned == scope));
        if owns {
            WindowOwnership::OwnsWindow
        } else {
            WindowOwnership::Windowless
        }
    }
}

fn process_scope(pid: u32) -> Option<String> {
    let process_id = i32::try_from(pid)
        .inspect_err(|range_failure| {
            tracing::warn!("sleep obstacles: pid {pid} out of range: {range_failure}");
        })
        .ok()?;
    match launch(process_id)?.scope {
        CgroupScope::Unit(unit) => Some(unit),
        CgroupScope::Unreported => None,
    }
}

async fn open_window_owners() -> CompositorWindows {
    let Some(compositor_pids) = compositor_window_pids().await else {
        return CompositorWindows::Unavailable;
    };
    spawn_blocking(move || listed_window_owners(compositor_pids))
        .await
        .unwrap_or_else(|join_failure| {
            tracing::warn!("sleep obstacles: window owner lookup did not finish: {join_failure}");
            CompositorWindows::Unavailable
        })
}

fn listed_window_owners(compositor_pids: Vec<i32>) -> CompositorWindows {
    let window_pids: Vec<u32> = compositor_pids
        .into_iter()
        .filter_map(|pid| {
            u32::try_from(pid)
                .inspect_err(|range_failure| {
                    tracing::warn!(
                        "sleep obstacles: compositor window pid {pid} is negative: {range_failure}"
                    );
                })
                .ok()
        })
        .collect();
    let window_scopes = window_pids
        .iter()
        .filter_map(|pid| process_scope(*pid))
        .filter(|scope| scope.starts_with("app-"))
        .collect();
    CompositorWindows::Listed(WindowOwners {
        window_pids,
        window_scopes,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HibernationModes {
    Disabled,
    Offered,
}

impl From<&str> for HibernationModes {
    fn from(hibernation_modes: &str) -> Self {
        if hibernation_modes.contains("[disabled]") {
            HibernationModes::Disabled
        } else {
            HibernationModes::Offered
        }
    }
}

fn disabling_boot_flag(command_line: &str) -> Option<String> {
    command_line
        .split_whitespace()
        .find(|argument| matches!(*argument, "nohibernate" | "hibernate=no"))
        .map(str::to_owned)
}

fn active_lockdown_level(lockdown_levels: &str) -> Option<String> {
    lockdown_levels
        .split_whitespace()
        .find_map(|level| level.strip_prefix('[')?.strip_suffix(']'))
        .filter(|level| *level != "none")
        .and_then(|level| non_empty(level.to_owned()))
}

fn pids_in_helper_output(helper_output: &str) -> Vec<u32> {
    helper_output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            line.parse()
                .inspect_err(|parse_failure| {
                    tracing::warn!(
                        "sleep obstacles: secret memory helper printed invalid pid {line:?}: \
                         {parse_failure}"
                    );
                })
                .ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_holder_owns_a_window_through_its_own_pid_or_its_app_scope() {
        let owners = WindowOwners {
            window_pids: vec![6627],
            window_scopes: vec!["app-bitwarden-6370.scope".to_owned()],
        };
        assert_eq!(
            owners.window_ownership(6627, None),
            WindowOwnership::OwnsWindow
        );
        assert_eq!(
            owners.window_ownership(7365, Some("app-bitwarden-6370.scope")),
            WindowOwnership::OwnsWindow
        );
        assert_eq!(
            owners.window_ownership(9000, Some("app-signal-12.scope")),
            WindowOwnership::Windowless
        );
        assert_eq!(
            owners.window_ownership(9001, None),
            WindowOwnership::Windowless
        );
    }

    #[test]
    fn only_the_disabled_marker_counts_as_disabled_hibernation() {
        assert_eq!(
            HibernationModes::from("[disabled]\n"),
            HibernationModes::Disabled
        );
        assert_eq!(
            HibernationModes::from("[platform] shutdown reboot suspend test_resume\n"),
            HibernationModes::Offered
        );
    }

    #[test]
    fn both_boot_flags_that_disable_hibernation_are_recognised() {
        assert_eq!(
            disabling_boot_flag("quiet nohibernate splash").as_deref(),
            Some("nohibernate")
        );
        assert_eq!(
            disabling_boot_flag("hibernate=no").as_deref(),
            Some("hibernate=no")
        );
        assert_eq!(
            disabling_boot_flag("resume=/dev/nvme0n1p2 hibernate=nocompress"),
            None
        );
    }

    #[test]
    fn only_a_selected_level_other_than_none_is_a_lockdown() {
        assert_eq!(
            active_lockdown_level("[none] integrity confidentiality"),
            None
        );
        assert_eq!(
            active_lockdown_level("none [integrity] confidentiality").as_deref(),
            Some("integrity")
        );
    }

    #[test]
    fn helper_output_is_read_as_one_pid_per_line() {
        assert_eq!(pids_in_helper_output("6370\n7001\n"), vec![6370, 7001]);
        assert_eq!(pids_in_helper_output(""), Vec::<u32>::new());
    }
}
