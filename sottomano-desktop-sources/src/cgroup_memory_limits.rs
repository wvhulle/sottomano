use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use futures::Stream;

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::PeriodicPolling,
    system_protocol_clients::{
        sysfs_attribute_reads::Error,
        sysfs_cgroup_memory::{CGROUP_ROOT, CgroupMemoryReading, read_cgroup_memory},
    },
};

const CGROUP_SEARCH_DEPTH: usize = 3;
const ALWAYS_SHOWN_CGROUPS: [&str; 1] = ["machine.slice"];

#[derive(Debug, Clone, PartialEq)]
pub struct LimitedCgroup {
    pub unit_name: String,
    pub cgroup_path: String,
    pub memory: CgroupMemoryReading,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LimitedCgroups {
    pub cgroups: Vec<LimitedCgroup>,
}

impl SnapshotSource for LimitedCgroups {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Memory limits";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(2))
            .blocking_poll_deliveries(|| limited_cgroups_under(Path::new(CGROUP_ROOT)))
    }
}

fn child_directories(cgroup_directory: &Path) -> Vec<PathBuf> {
    fs::read_dir(cgroup_directory)
        .into_iter()
        .flatten()
        .filter_map(|listed_entry| {
            listed_entry
                .inspect_err(|list_failure| {
                    tracing::debug!(
                        "memory limits: listing {} failed: {list_failure}",
                        cgroup_directory.display()
                    );
                })
                .ok()
        })
        .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_dir()))
        .map(|entry| entry.path())
        .collect()
}

fn collect_limited_cgroups(
    cgroup_root: &Path,
    cgroup_directory: &Path,
    remaining_depth: usize,
    limited_cgroups: &mut Vec<LimitedCgroup>,
) {
    if remaining_depth == 0 {
        return;
    }
    for child_directory in child_directories(cgroup_directory) {
        let Ok(relative_path) = child_directory.strip_prefix(cgroup_root) else {
            continue;
        };
        let cgroup_path = format!("/{}", relative_path.display());
        if let Some(memory) = read_cgroup_memory(&child_directory) {
            let always_shown = ALWAYS_SHOWN_CGROUPS
                .iter()
                .any(|shown_path| relative_path == Path::new(shown_path));
            if memory.is_limited() || always_shown {
                limited_cgroups.push(LimitedCgroup {
                    unit_name: child_directory.file_name().map_or_else(
                        || cgroup_path.clone(),
                        |unit_name| unit_name.to_string_lossy().into_owned(),
                    ),
                    cgroup_path,
                    memory,
                });
            }
        }
        collect_limited_cgroups(
            cgroup_root,
            &child_directory,
            remaining_depth - 1,
            limited_cgroups,
        );
    }
}

fn limited_cgroups_under(cgroup_root: &Path) -> Result<LimitedCgroups, Error> {
    if !cgroup_root.join("cgroup.controllers").exists() {
        return Err(Error::Unreadable(cgroup_root.display().to_string()));
    }
    let mut cgroups = Vec::new();
    collect_limited_cgroups(cgroup_root, cgroup_root, CGROUP_SEARCH_DEPTH, &mut cgroups);
    cgroups.sort_by(|first, second| {
        second
            .memory
            .current
            .cmp(&first.memory.current)
            .then_with(|| first.cgroup_path.cmp(&second.cgroup_path))
    });
    Ok(LimitedCgroups { cgroups })
}

#[cfg(test)]
mod tests {
    use std::{env, fs, path::Path, process};

    use super::limited_cgroups_under;

    fn write_cgroup(cgroup_root: &Path, relative_path: &str, current: &str, max: &str) {
        let cgroup_directory = cgroup_root.join(relative_path);
        fs::create_dir_all(&cgroup_directory).unwrap();
        fs::write(cgroup_directory.join("memory.current"), current).unwrap();
        fs::write(cgroup_directory.join("memory.max"), max).unwrap();
    }

    #[test]
    fn only_capped_cgroups_and_the_machine_slice_are_listed() {
        let cgroup_root =
            env::temp_dir().join(format!("sottomano-limited-cgroups-{}", process::id()));
        fs::create_dir_all(&cgroup_root).unwrap();
        fs::write(cgroup_root.join("cgroup.controllers"), "memory\n").unwrap();
        write_cgroup(&cgroup_root, "ci-runners.slice", "1000\n", "12000\n");
        write_cgroup(
            &cgroup_root,
            "ci-runners.slice/woodpecker-agent-codeberg-podman.service",
            "900\n",
            "max\n",
        );
        write_cgroup(&cgroup_root, "system.slice", "5000\n", "max\n");
        write_cgroup(
            &cgroup_root,
            "system.slice/nix-daemon.service",
            "3000\n",
            "14000\n",
        );
        write_cgroup(&cgroup_root, "machine.slice", "200\n", "max\n");
        let listed = limited_cgroups_under(&cgroup_root).unwrap();
        fs::remove_dir_all(&cgroup_root).unwrap();
        let listed_paths: Vec<&str> = listed
            .cgroups
            .iter()
            .map(|cgroup| cgroup.cgroup_path.as_str())
            .collect();
        assert_eq!(
            listed_paths,
            [
                "/system.slice/nix-daemon.service",
                "/ci-runners.slice",
                "/machine.slice"
            ]
        );
    }

    #[test]
    fn a_missing_cgroup_hierarchy_is_an_error() {
        assert!(limited_cgroups_under(Path::new("/nonexistent/cgroup")).is_err());
    }
}
