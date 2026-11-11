use std::{
    env, fs,
    future::ready,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use bytesize::ByteSize;
use futures::{Stream, StreamExt, stream, stream::BoxStream};
use serde::Deserialize;

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::PeriodicPolling,
    stream_operators::read_once_then_per_burst,
    system_protocol_clients::{
        inotify_directory_changes::{WatchTarget, changes},
        sysfs_attribute_reads::Error,
        sysfs_cgroup_memory::unit_memory_of_process,
    },
};

const SANDBOX_POLL_PERIOD: Duration = Duration::from_secs(5);
const RUNTIME_DIRECTORY_VARIABLE: &str = "XDG_RUNTIME_DIR";
const INSTANCES_DIRECTORY: &str = ".flatpak";
const INFO_FILE_NAME: &str = "info";
const PID_FILE_NAME: &str = "pid";
const BWRAP_INFO_FILE_NAME: &str = "bwrapinfo.json";
const PROC_ROOT: &str = "/proc";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlatpakProbe {
    pub running_instance_count: usize,
}

impl SnapshotSource for FlatpakProbe {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Flatpak instance presence";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let watch_target: BoxStream<'static, ()> = match flatpak_instances_directory() {
            Some(instances_directory) => {
                changes(vec![WatchTarget::EntriesOf(instances_directory)]).boxed()
            }
            None => stream::empty::<()>().boxed(),
        };
        SnapshotDelivery::live_push_deliveries(ready(Ok(read_once_then_per_burst(
            watch_target,
            || ready(counted_running_instances()),
        ))))
    }
}

fn flatpak_instances_directory() -> Option<PathBuf> {
    env::var_os(RUNTIME_DIRECTORY_VARIABLE)
        .map(|runtime_directory| PathBuf::from(runtime_directory).join(INSTANCES_DIRECTORY))
}

fn counted_running_instances() -> Result<FlatpakProbe, Error> {
    let instances_directory = flatpak_instances_directory()
        .ok_or_else(|| Error::Unreadable(RUNTIME_DIRECTORY_VARIABLE.to_owned()))?;
    Ok(FlatpakProbe {
        running_instance_count: running_sandboxes_in(&instances_directory, Path::new(PROC_ROOT))
            .sandboxes
            .len(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatpakSandboxListing {
    pub instance_id: String,
    pub application_id: String,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatpakSandboxListings {
    pub listings: Vec<FlatpakSandboxListing>,
}

impl SnapshotSource for FlatpakSandboxListings {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Flatpak app ids";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(SANDBOX_POLL_PERIOD).blocking_poll_deliveries(|| {
            let instances_directory = flatpak_instances_directory()
                .ok_or_else(|| Error::Unreadable(RUNTIME_DIRECTORY_VARIABLE.to_owned()))?;
            Ok(listed_sandboxes_in(
                &instances_directory,
                Path::new(PROC_ROOT),
            ))
        })
    }
}

fn listed_sandboxes_in(instances_directory: &Path, proc_root: &Path) -> FlatpakSandboxListings {
    let mut listings: Vec<FlatpakSandboxListing> = fs::read_dir(instances_directory)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let info_text = fs::read_to_string(entry.path().join(INFO_FILE_NAME)).ok()?;
            let instance_info = parsed_instance_info(&info_text);
            let sandbox_pid = sandbox_pid_in(&entry.path())?;
            let process_directory = proc_root.join(sandbox_pid.to_string());
            process_directory.exists().then_some(FlatpakSandboxListing {
                instance_id: entry.file_name().to_string_lossy().into_owned(),
                application_id: instance_info.application_id?,
                running: true,
            })
        })
        .collect();
    listings.sort_by(|first, second| {
        first
            .application_id
            .cmp(&second.application_id)
            .then_with(|| first.instance_id.cmp(&second.instance_id))
    });
    FlatpakSandboxListings { listings }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatpakSandbox {
    pub instance_id: String,
    pub application_id: String,
    pub runtime_reference: Option<String>,
    pub branch: Option<String>,
    pub sandbox_pid: u32,
    pub started_at: Option<SystemTime>,
    pub memory_in_use: Option<ByteSize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatpakSandboxes {
    pub sandboxes: Vec<FlatpakSandbox>,
}

impl SnapshotSource for FlatpakSandboxes {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Flatpak sandboxes";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(SANDBOX_POLL_PERIOD).blocking_poll_deliveries(|| {
            let instances_directory = flatpak_instances_directory()
                .ok_or_else(|| Error::Unreadable(RUNTIME_DIRECTORY_VARIABLE.to_owned()))?;
            Ok(running_sandboxes_in(
                &instances_directory,
                Path::new(PROC_ROOT),
            ))
        })
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct InstanceInfo {
    application_id: Option<String>,
    runtime_reference: Option<String>,
    branch: Option<String>,
}

fn parsed_instance_info(info_text: &str) -> InstanceInfo {
    let mut instance_info = InstanceInfo::default();
    let mut current_section = String::new();
    for info_line in info_text.lines().map(str::trim) {
        if let Some(section_name) = info_line
            .strip_prefix('[')
            .and_then(|bracketed| bracketed.strip_suffix(']'))
        {
            section_name.clone_into(&mut current_section);
            continue;
        }
        let Some((info_key, info_value)) = info_line.split_once('=') else {
            continue;
        };
        let info_value = Some(info_value.trim().to_owned());
        match (current_section.as_str(), info_key.trim()) {
            ("Application", "name") => instance_info.application_id = info_value,
            ("Application", "runtime") => instance_info.runtime_reference = info_value,
            ("Instance", "branch") => instance_info.branch = info_value,
            _ => {}
        }
    }
    instance_info
}

#[derive(Deserialize)]
struct BubblewrapInfo {
    #[serde(rename = "child-pid")]
    child_pid: u32,
}

fn sandbox_pid_in(instance_directory: &Path) -> Option<u32> {
    fs::read_to_string(instance_directory.join(PID_FILE_NAME))
        .ok()
        .and_then(|pid_text| pid_text.trim().parse().ok())
        .or_else(|| {
            let bwrap_info_text =
                fs::read_to_string(instance_directory.join(BWRAP_INFO_FILE_NAME)).ok()?;
            serde_json::from_str::<BubblewrapInfo>(&bwrap_info_text)
                .ok()
                .map(|bwrap_info| bwrap_info.child_pid)
        })
}

fn running_sandboxes_in(instances_directory: &Path, proc_root: &Path) -> FlatpakSandboxes {
    let mut sandboxes: Vec<FlatpakSandbox> = fs::read_dir(instances_directory)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let sandbox_directory = entry.path();
            let info_text = fs::read_to_string(sandbox_directory.join(INFO_FILE_NAME)).ok()?;
            let instance_info = parsed_instance_info(&info_text);
            let sandbox_pid = sandbox_pid_in(&sandbox_directory)?;
            let process_directory = proc_root.join(sandbox_pid.to_string());
            if !process_directory.exists() {
                return None;
            }
            Some(FlatpakSandbox {
                instance_id: entry.file_name().to_string_lossy().into_owned(),
                application_id: instance_info.application_id?,
                runtime_reference: instance_info.runtime_reference,
                branch: instance_info.branch,
                sandbox_pid,
                started_at: fs::metadata(&sandbox_directory)
                    .and_then(|metadata| metadata.modified())
                    .ok(),
                memory_in_use: unit_memory_of_process(sandbox_pid),
            })
        })
        .collect();
    sandboxes.sort_by(|first, second| {
        first
            .application_id
            .cmp(&second.application_id)
            .then_with(|| first.instance_id.cmp(&second.instance_id))
    });
    FlatpakSandboxes { sandboxes }
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process};

    use super::{InstanceInfo, parsed_instance_info, running_sandboxes_in};

    #[test]
    fn the_info_file_names_the_app_runtime_and_branch() {
        let info_text = [
            "[Application]",
            "name=org.mozilla.firefox",
            "runtime=runtime/org.freedesktop.Platform/x86_64/24.08",
            "",
            "[Instance]",
            "instance-id=1849302754",
            "branch=stable",
            "arch=x86_64",
        ]
        .join("\n");
        let info_text = info_text.as_str();
        assert_eq!(
            parsed_instance_info(info_text),
            InstanceInfo {
                application_id: Some("org.mozilla.firefox".to_owned()),
                runtime_reference: Some("runtime/org.freedesktop.Platform/x86_64/24.08".to_owned()),
                branch: Some("stable".to_owned()),
            }
        );
    }

    #[test]
    fn an_instance_whose_process_has_exited_is_left_out() {
        let scratch = env::temp_dir().join(format!("sottomano-flatpak-{}", process::id()));
        let proc_root = scratch.join("proc");
        let own_pid = process::id();
        fs::create_dir_all(proc_root.join(own_pid.to_string())).unwrap();
        for (instance_id, sandbox_pid) in [("1849302754", own_pid), ("2210043981", 4_000_000)] {
            let instance_directory = scratch.join("instances").join(instance_id);
            fs::create_dir_all(&instance_directory).unwrap();
            fs::write(
                instance_directory.join("info"),
                "[Application]\nname=org.gnome.Calculator\n",
            )
            .unwrap();
            fs::write(instance_directory.join("pid"), sandbox_pid.to_string()).unwrap();
        }
        let listed = running_sandboxes_in(&scratch.join("instances"), &proc_root);
        fs::remove_dir_all(&scratch).unwrap();
        assert_eq!(listed.sandboxes.len(), 1);
        assert_eq!(listed.sandboxes[0].instance_id, "1849302754");
    }
}
