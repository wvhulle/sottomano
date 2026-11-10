use std::{
    fs,
    path::{Path, PathBuf},
};

const NIX_BUILD_DIRECTORY_PREFIX: &str = "nix-build-";
const DERIVATION_SUFFIX_MARKER: &str = ".drv-";
const NIXOS_TEST_RUN_PREFIX: &str = "vm-test-run-";
const CI_CHECKOUT_MOUNT_PREFIXES: [&str; 2] = ["/workspace/", "/woodpecker/src/"];
const ANCESTOR_SEARCH_DEPTH: usize = 8;
const NIX_DAEMON_PROGRAM: &str = "nix-daemon";
const NIX_CLIENT_PROGRAM: &str = "nix";
const UNINFORMATIVE_DIRECTORIES: [&str; 4] = ["/", "/tmp", "/nix", "/build"];

fn derivation_name_in(mount_field: &str) -> Option<&str> {
    let after_prefix = mount_field
        .split('/')
        .find_map(|path_component| path_component.strip_prefix(NIX_BUILD_DIRECTORY_PREFIX))?;
    let derivation_name = &after_prefix[..after_prefix.find(DERIVATION_SUFFIX_MARKER)?];
    Some(
        derivation_name
            .strip_prefix(NIXOS_TEST_RUN_PREFIX)
            .unwrap_or(derivation_name),
    )
}

fn checkout_repository_in(mount_point: &str) -> Option<&str> {
    CI_CHECKOUT_MOUNT_PREFIXES
        .iter()
        .find_map(|checkout_prefix| mount_point.strip_prefix(checkout_prefix))
        .and_then(|checkout_path| checkout_path.trim_end_matches('/').rsplit('/').next())
        .filter(|repository_name| !repository_name.is_empty())
}

#[must_use]
pub(crate) fn context_in_mountinfo(mountinfo_text: &str) -> Option<String> {
    let mount_entries: Vec<Vec<&str>> = mountinfo_text
        .lines()
        .map(|mount_line| mount_line.split_whitespace().collect())
        .collect();
    let built_test = mount_entries
        .iter()
        .flat_map(|mount_fields| mount_fields.iter().take(5))
        .find_map(|mount_field| derivation_name_in(mount_field));
    let checked_out_repository = || {
        mount_entries
            .iter()
            .filter_map(|mount_fields| mount_fields.get(4))
            .find_map(|mount_point| checkout_repository_in(mount_point))
    };
    built_test
        .or_else(checked_out_repository)
        .map(str::to_owned)
}

fn parent_pid_of(pid: i32) -> Option<i32> {
    let stat_text = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_command = &stat_text[stat_text.rfind(')')? + 2..];
    after_command.split_whitespace().nth(1)?.parse().ok()
}

fn informative_directory_name(working_directory: &Path) -> Option<String> {
    let is_uninformative = UNINFORMATIVE_DIRECTORIES
        .iter()
        .any(|uninformative| working_directory == Path::new(uninformative))
        || working_directory.starts_with("/tmp")
        || working_directory.starts_with("/nix/store");
    if is_uninformative {
        return None;
    }
    working_directory
        .file_name()
        .map(|directory_name| directory_name.to_string_lossy().into_owned())
}

fn program_file_name(command_line: &[String]) -> Option<&str> {
    Path::new(command_line.first()?).file_name()?.to_str()
}

fn daemon_client_pid(command_line: &[String]) -> Option<i32> {
    (program_file_name(command_line)? == NIX_DAEMON_PROGRAM)
        .then(|| command_line.get(1)?.parse().ok())
        .flatten()
}

fn installable_attribute(command_line: &[String]) -> Option<String> {
    if program_file_name(command_line)? != NIX_CLIENT_PROGRAM {
        return None;
    }
    command_line
        .iter()
        .skip(1)
        .filter(|argument| !argument.starts_with('-'))
        .find_map(|argument| argument.split_once('#'))
        .and_then(|(_, attribute_path)| attribute_path.rsplit('.').next())
        .filter(|attribute_name| !attribute_name.is_empty())
        .map(str::to_owned)
}

fn command_line_of(pid: i32) -> Vec<String> {
    fs::read(format!("/proc/{pid}/cmdline"))
        .map(|command_line_bytes| {
            command_line_bytes
                .split(|byte| *byte == 0)
                .filter(|argument| !argument.is_empty())
                .map(|argument| String::from_utf8_lossy(argument).into_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn ancestor_context(pid: i32) -> Option<String> {
    let mut current_pid = pid;
    for _ in 0..ANCESTOR_SEARCH_DEPTH {
        let command_line = command_line_of(current_pid);
        if let Some(attribute_name) = installable_attribute(&command_line) {
            return Some(attribute_name);
        }
        let working_directory: Option<PathBuf> =
            fs::read_link(format!("/proc/{current_pid}/cwd")).ok();
        if let Some(directory_name) = working_directory
            .as_deref()
            .and_then(informative_directory_name)
        {
            return Some(directory_name);
        }
        current_pid = daemon_client_pid(&command_line)
            .or_else(|| parent_pid_of(current_pid))
            .filter(|next_pid| *next_pid > 1)?;
    }
    None
}

#[must_use]
pub(crate) fn launch_context_of(pid: i32) -> Option<String> {
    fs::read_to_string(format!("/proc/{pid}/mountinfo"))
        .ok()
        .and_then(|mountinfo_text| context_in_mountinfo(&mountinfo_text))
        .or_else(|| ancestor_context(pid))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        context_in_mountinfo, daemon_client_pid, informative_directory_name, installable_attribute,
    };

    fn arguments(command_line: &[&str]) -> Vec<String> {
        command_line
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect()
    }

    #[test]
    fn a_daemon_worker_leads_to_the_client_that_asked_for_the_build() {
        assert_eq!(
            daemon_client_pid(&arguments(&[
                "/nix/store/abc-determinate-nix-3.24.1/bin/nix-daemon",
                "1213111",
                "json-log-path",
            ])),
            Some(1_213_111)
        );
        assert_eq!(
            daemon_client_pid(&arguments(&["nix-daemon", "--option", "json-log-path"])),
            None
        );
    }

    #[test]
    fn a_nix_build_names_the_check_it_builds() {
        assert_eq!(
            installable_attribute(&arguments(&[
                "nix",
                "build",
                "--no-link",
                ".#checks.x86_64-linux.ci-runners-private-network-isolation",
            ]))
            .as_deref(),
            Some("ci-runners-private-network-isolation")
        );
        assert_eq!(
            installable_attribute(&arguments(&["nix", "flake", "check"])),
            None
        );
    }

    #[test]
    fn a_vm_in_a_nix_build_is_named_after_its_test() {
        let mountinfo_text = "\
            612 598 0:31 / / rw,relatime - tmpfs none rw\n640 612 259:2 \
                              /nix/var/nix/builds/nix-build-vm-test-run-bindrune-pairing.drv-0/\
                              build /build rw - ext4 /dev/nvme0n1p2 rw\n";
        assert_eq!(
            context_in_mountinfo(mountinfo_text).as_deref(),
            Some("bindrune-pairing")
        );
    }

    #[test]
    fn a_vm_in_a_ci_job_is_named_after_the_checked_out_repository() {
        let forgejo_mountinfo = "1201 1188 0:94 /forgejo/workspace /workspace/wvhulle/bindrune rw \
                                 - overlay overlay rw\n";
        assert_eq!(
            context_in_mountinfo(forgejo_mountinfo).as_deref(),
            Some("bindrune")
        );
        let woodpecker_mountinfo = "1301 1288 0:95 / \
                                    /woodpecker/src/codeberg.org/wvhulle/bootvine rw - overlay \
                                    overlay rw\n";
        assert_eq!(
            context_in_mountinfo(woodpecker_mountinfo).as_deref(),
            Some("bootvine")
        );
    }

    #[test]
    fn a_plain_host_mount_table_gives_no_context() {
        let mountinfo_text = "22 1 259:2 / / rw,relatime - ext4 /dev/nvme0n1p2 rw\n";
        assert_eq!(context_in_mountinfo(mountinfo_text), None);
    }

    #[test]
    fn temporary_and_root_directories_name_no_project() {
        assert_eq!(informative_directory_name(Path::new("/")), None);
        assert_eq!(
            informative_directory_name(Path::new("/tmp/nix-shell.x8")),
            None
        );
        assert_eq!(
            informative_directory_name(Path::new("/home/wvhulle/Code/bindrune")).as_deref(),
            Some("bindrune")
        );
    }
}
