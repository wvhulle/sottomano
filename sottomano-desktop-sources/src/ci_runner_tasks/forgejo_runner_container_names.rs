const TASK_PREFIX: &str = "FORGEJO-ACTIONS-TASK-";
const JOB_PREFIX: &str = "JOB-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ForgejoJobName {
    Named(String),
    NotInContainerName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ForgejoContainerName {
    pub task_id: u64,
    pub job_name: ForgejoJobName,
}

pub(super) fn parse_forgejo_container_name(container_name: &str) -> Option<ForgejoContainerName> {
    let mut parts = container_name.strip_prefix(TASK_PREFIX)?.split('_');
    let task_id = parts.next()?.parse().ok()?;
    let job_name = parts
        .find_map(|part| part.strip_prefix(JOB_PREFIX))
        .map_or(ForgejoJobName::NotInContainerName, |job_name| {
            ForgejoJobName::Named(job_name.to_owned())
        });
    Some(ForgejoContainerName { task_id, job_name })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_and_job_come_from_the_runner_name_format() {
        assert_eq!(
            parse_forgejo_container_name(
                "FORGEJO-ACTIONS-TASK-11282424_WORKFLOW-0a1b2c_JOB-build-release"
            ),
            Some(ForgejoContainerName {
                task_id: 11_282_424,
                job_name: ForgejoJobName::Named("build-release".to_owned()),
            })
        );
    }

    #[test]
    fn a_name_without_a_job_part_keeps_its_task() {
        assert_eq!(
            parse_forgejo_container_name("FORGEJO-ACTIONS-TASK-7-deadbeef_service"),
            None
        );
        assert_eq!(
            parse_forgejo_container_name("FORGEJO-ACTIONS-TASK-7_WORKFLOW-0a1b2c"),
            Some(ForgejoContainerName {
                task_id: 7,
                job_name: ForgejoJobName::NotInContainerName,
            })
        );
    }

    #[test]
    fn unrelated_containers_are_not_tasks() {
        assert_eq!(parse_forgejo_container_name("postgres"), None);
        assert_eq!(parse_forgejo_container_name("wp_01M6"), None);
    }
}
