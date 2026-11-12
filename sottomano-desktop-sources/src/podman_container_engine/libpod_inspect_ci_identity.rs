use std::path::Path;

use serde::Deserialize;

use crate::{
    ci_runner_tasks::TaskWebLog,
    podman_container_engine::libpod_running_containers::LIBPOD_API_ROOT,
    system_protocol_clients::http_json_client,
};

const REPOSITORY_KEY: &str = "CI_REPO";
const PIPELINE_NUMBER_KEY: &str = "CI_PIPELINE_NUMBER";
const WORKFLOW_NAME_KEY: &str = "CI_WORKFLOW_NAME";
const STEP_NAME_KEY: &str = "CI_STEP_NAME";
const PIPELINE_URL_KEY: &str = "CI_PIPELINE_URL";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WoodpeckerStepIdentity {
    pub repository: String,
    pub pipeline_number: String,
    pub workflow_name: String,
    pub step_name: String,
    pub pipeline_web_log: TaskWebLog,
}

#[derive(Deserialize)]
struct InspectedContainer {
    #[serde(rename = "Config")]
    config: InspectedContainerConfig,
}

#[derive(Deserialize)]
struct InspectedContainerConfig {
    #[serde(rename = "Env", default)]
    environment_entries: Vec<String>,
}

fn allow_listed_value<'entries>(entries: &'entries [String], key: &str) -> Option<&'entries str> {
    entries.iter().find_map(|entry| {
        let (entry_key, value) = entry.split_once('=')?;
        (entry_key == key).then_some(value)
    })
}

impl InspectedContainer {
    fn woodpecker_step_identity(&self) -> Option<WoodpeckerStepIdentity> {
        let entries = &self.config.environment_entries;
        Some(WoodpeckerStepIdentity {
            repository: allow_listed_value(entries, REPOSITORY_KEY)?.to_owned(),
            pipeline_number: allow_listed_value(entries, PIPELINE_NUMBER_KEY)?.to_owned(),
            workflow_name: allow_listed_value(entries, WORKFLOW_NAME_KEY)?.to_owned(),
            step_name: allow_listed_value(entries, STEP_NAME_KEY)?.to_owned(),
            pipeline_web_log: match allow_listed_value(entries, PIPELINE_URL_KEY) {
                Some(pipeline_url) => TaskWebLog::Linked(pipeline_url.to_owned()),
                None => TaskWebLog::NotKnown,
            },
        })
    }
}

pub(super) async fn fetch_woodpecker_step_identity(
    socket_path: &Path,
    container_id: &str,
) -> Option<WoodpeckerStepIdentity> {
    let url = format!("{LIBPOD_API_ROOT}/containers/{container_id}/json");
    let inspected: InspectedContainer =
        http_json_client::get_json_over_unix_socket(socket_path, &url)
            .await
            .inspect_err(|inspect_failure| {
                tracing::debug!(
                    "podman: inspecting container {container_id} failed: {inspect_failure}"
                );
            })
            .ok()?;
    inspected.woodpecker_step_identity()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn decoded(entries: &[&str]) -> InspectedContainer {
        serde_json::from_value(json!({"Config": {"Env": entries}})).expect("inspect body decodes")
    }

    #[test]
    fn identity_comes_from_the_allow_listed_keys() {
        let inspected = decoded(&[
            "CI_REPO=wvhulle/sottomano",
            "CI_PIPELINE_NUMBER=185",
            "CI_WORKFLOW_NAME=checks",
            "CI_STEP_NAME=clone",
            "CI_PIPELINE_URL=https://ci.codeberg.org/repos/1/pipeline/185",
        ]);
        assert_eq!(
            inspected.woodpecker_step_identity(),
            Some(WoodpeckerStepIdentity {
                repository: "wvhulle/sottomano".to_owned(),
                pipeline_number: "185".to_owned(),
                workflow_name: "checks".to_owned(),
                step_name: "clone".to_owned(),
                pipeline_web_log: TaskWebLog::Linked(
                    "https://ci.codeberg.org/repos/1/pipeline/185".to_owned()
                ),
            })
        );
    }

    #[test]
    fn a_step_without_a_pipeline_url_keeps_its_identity() {
        let inspected = decoded(&[
            "CI_REPO=wvhulle/sottomano",
            "CI_PIPELINE_NUMBER=185",
            "CI_WORKFLOW_NAME=checks",
            "CI_STEP_NAME=clone",
        ]);
        assert_eq!(
            inspected
                .woodpecker_step_identity()
                .map(|identity| identity.pipeline_web_log),
            Some(TaskWebLog::NotKnown)
        );
    }

    #[test]
    fn secrets_and_scripts_never_reach_the_identity() {
        let inspected = decoded(&[
            "CI_REPO=wvhulle/sottomano",
            "CI_PIPELINE_NUMBER=185",
            "CI_WORKFLOW_NAME=checks",
            "CI_STEP_NAME=clone",
            "CI_SCRIPT=c2VjcmV0LXNjcmlwdA==",
            "CACHIX_AUTH_TOKEN=hunter2",
        ]);
        let shown = format!(
            "{:?}",
            inspected
                .woodpecker_step_identity()
                .expect("identity is complete")
        );
        assert!(!shown.contains("hunter2"));
        assert!(!shown.contains("c2VjcmV0LXNjcmlwdA"));
    }

    #[test]
    fn a_container_missing_any_key_has_no_identity() {
        let inspected = decoded(&["CI_REPO=wvhulle/sottomano", "CI_STEP_NAME=clone"]);
        assert_eq!(inspected.woodpecker_step_identity(), None);
    }

    #[test]
    fn a_value_containing_equals_signs_stays_whole() {
        let entries = ["CI_STEP_NAME=build=release".to_owned()];
        assert_eq!(
            allow_listed_value(&entries, STEP_NAME_KEY),
            Some("build=release")
        );
    }
}
