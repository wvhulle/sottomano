use std::iter;

use procfs::process::Process;

const DEEPEST_ANCESTRY: usize = 64;
const INIT_PROCESS: u32 = 1;

#[must_use]
pub fn process_ancestry(pid: u32) -> Vec<u32> {
    ancestry_through(pid, parent_process)
}

fn parent_process(child_pid: u32) -> Option<u32> {
    let parent_pid = Process::new(child_pid.cast_signed())
        .and_then(|process| process.stat())
        .map(|stat| stat.ppid)
        .inspect_err(|stat_failure| {
            tracing::debug!("process {child_pid}: no parent in procfs: {stat_failure}");
        })
        .ok()?;
    u32::try_from(parent_pid)
        .inspect_err(|_| tracing::debug!("process {child_pid}: negative parent {parent_pid}"))
        .ok()
}

fn ancestry_through(pid: u32, parent_of: impl Fn(u32) -> Option<u32>) -> Vec<u32> {
    iter::successors(Some(pid), |&child_pid| {
        parent_of(child_pid)
            .filter(|&parent_pid| parent_pid > INIT_PROCESS && parent_pid != child_pid)
    })
    .take(DEEPEST_ANCESTRY)
    .collect()
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, process};

    use super::*;

    fn parents(edges: &[(u32, u32)]) -> impl Fn(u32) -> Option<u32> {
        let parent_by_child: HashMap<u32, u32> = edges.iter().copied().collect();
        move |child_pid| parent_by_child.get(&child_pid).copied()
    }

    #[test]
    fn ancestry_runs_from_the_process_up_to_below_init() {
        let tree = parents(&[(400, 300), (300, 200), (200, 1)]);
        assert_eq!(ancestry_through(400, tree), vec![400, 300, 200]);
    }

    #[test]
    fn ancestry_stops_where_the_parent_is_unknown() {
        let tree = parents(&[(400, 300)]);
        assert_eq!(ancestry_through(400, tree), vec![400, 300]);
    }

    #[test]
    fn a_self_parented_process_does_not_loop() {
        let tree = parents(&[(400, 400)]);
        assert_eq!(ancestry_through(400, tree), vec![400]);
    }

    #[test]
    fn the_running_test_process_has_itself_first_in_its_ancestry() {
        let own_pid = process::id();
        assert_eq!(process_ancestry(own_pid).first(), Some(&own_pid));
    }
}
