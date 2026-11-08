use std::collections::HashMap;

use procfs::process::Process;

use crate::system_protocol_clients::procfs_process_tree::ProcessTable;

const CONMON_COMMAND_NAME: &str = "conmon";
const CONTAINER_ID_FLAGS: [&str; 2] = ["-c", "--cid"];

#[derive(Debug, Default)]
pub(crate) struct ConmonSessions {
    conmon_pids_by_container_id: HashMap<String, Vec<i32>>,
}

fn attached_container_id(arguments: &[String]) -> Option<&str> {
    arguments
        .windows(2)
        .find(|flag_and_value| CONTAINER_ID_FLAGS.contains(&flag_and_value[0].as_str()))
        .map(|flag_and_value| flag_and_value[1].as_str())
}

impl ConmonSessions {
    pub(crate) fn read_current(table: &ProcessTable) -> Self {
        Self::from_command_lines(table.pids_named(CONMON_COMMAND_NAME).iter().filter_map(
            |&conmon_pid| {
                let arguments = Process::new(conmon_pid).ok()?.cmdline().ok()?;
                Some((conmon_pid, arguments))
            },
        ))
    }

    pub(crate) fn from_command_lines(
        command_lines: impl IntoIterator<Item = (i32, Vec<String>)>,
    ) -> Self {
        let conmon_pids_by_container_id = command_lines.into_iter().fold(
            HashMap::<String, Vec<i32>>::new(),
            |mut conmon_pids_by_container_id, (conmon_pid, arguments)| {
                if let Some(container_id) = attached_container_id(&arguments) {
                    conmon_pids_by_container_id
                        .entry(container_id.to_owned())
                        .or_default()
                        .push(conmon_pid);
                }
                conmon_pids_by_container_id
            },
        );
        Self {
            conmon_pids_by_container_id,
        }
    }

    pub(crate) fn measured_roots(&self, container_id: &str, init_process_pid: i32) -> Vec<i32> {
        self.conmon_pids_by_container_id
            .get(container_id)
            .map_or_else(Vec::new, Clone::clone)
            .into_iter()
            .chain([init_process_pid])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn exec_sessions_join_the_container_they_attach_to() {
        let sessions = ConmonSessions::from_command_lines([
            (5, arguments(&["conmon", "--api-version", "1", "-c", "abc"])),
            (6, arguments(&["conmon", "-c", "abc", "--exec"])),
            (7, arguments(&["conmon", "--cid", "other"])),
            (8, arguments(&["conmon", "--log-level", "debug"])),
        ]);
        assert_eq!(sessions.measured_roots("abc", 10), vec![5, 6, 10]);
        assert_eq!(sessions.measured_roots("other", 20), vec![7, 20]);
    }

    #[test]
    fn a_container_without_conmon_is_measured_from_its_own_process() {
        assert_eq!(
            ConmonSessions::default().measured_roots("abc", 10),
            vec![10]
        );
    }
}
