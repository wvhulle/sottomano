use std::{
    collections::HashMap,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;

use super::{
    ContainerPhase, ContainerSelection, EngineReadError, LIBPOD_API_ROOT, StoppedContainer,
    WOODPECKER_STEP_LABEL, classified_engine_failure, created_by_ci_runner, engine_unix_time,
    parsed_engine_timestamp,
};
use crate::system_protocol_clients::http_json_client;

const RUNNING_STATE: &str = "running";
const STOPPED_STATES: [&str; 4] = ["created", "configured", "exited", "stopped"];

#[derive(Deserialize)]
pub(super) struct ListedContainer {
    #[serde(rename = "Id")]
    pub(super) id: String,
    #[serde(rename = "Names", default)]
    names: Vec<String>,
    #[serde(rename = "Image", default)]
    pub(super) image: String,
    #[serde(rename = "State", default)]
    state: String,
    #[serde(rename = "Pid", default)]
    pub(super) pid: i32,
    #[serde(rename = "StartedAt", default)]
    started_at_unix_seconds: i64,
    #[serde(rename = "ExitedAt", default)]
    exited_at_unix_seconds: i64,
    #[serde(rename = "Created", default)]
    created: String,
    #[serde(rename = "Labels", default)]
    labels: Option<HashMap<String, String>>,
}

impl ListedContainer {
    pub(super) fn step_label(&self) -> Option<&str> {
        self.labels
            .as_ref()?
            .get(WOODPECKER_STEP_LABEL)
            .map(String::as_str)
    }

    pub(super) fn container_name(&self) -> String {
        self.names
            .first()
            .cloned()
            .unwrap_or_else(|| self.id.clone())
    }

    pub(super) fn phase(&self) -> ContainerPhase {
        if self.state == RUNNING_STATE && self.pid > 0 {
            ContainerPhase::Running
        } else if STOPPED_STATES.contains(&self.state.as_str()) {
            if created_by_ci_runner(
                self.labels
                    .iter()
                    .flatten()
                    .map(|(label_key, _)| label_key.as_str()),
                &self.container_name(),
            ) {
                ContainerPhase::CiLeftover
            } else {
                ContainerPhase::Reusable
            }
        } else {
            ContainerPhase::Transitional
        }
    }

    pub(super) fn shown_name(&self) -> String {
        self.step_label()
            .map_or_else(|| self.container_name(), str::to_owned)
    }

    pub(super) fn started_at(&self) -> SystemTime {
        engine_unix_time(self.started_at_unix_seconds).unwrap_or(UNIX_EPOCH)
    }

    pub(super) fn stopped_container(&self) -> StoppedContainer {
        let stopped_at = engine_unix_time(self.exited_at_unix_seconds)
            .into_iter()
            .chain(parsed_engine_timestamp(&self.created))
            .max()
            .unwrap_or(UNIX_EPOCH);
        StoppedContainer {
            id: self.id.clone(),
            container_name: self.container_name(),
            shown_name: self.shown_name(),
            image: self.image.clone(),
            stopped_at,
        }
    }
}

async fn listed_containers(
    socket_path: &Path,
    selection: ContainerSelection,
) -> Result<Vec<ListedContainer>, http_json_client::Error> {
    http_json_client::get_json_over_unix_socket(
        socket_path,
        &format!(
            "{LIBPOD_API_ROOT}/containers/json{}",
            selection.list_query()
        ),
    )
    .await
}

pub(super) async fn listed_containers_with_one_retry(
    socket_path: &Path,
    selection: ContainerSelection,
) -> Result<Vec<ListedContainer>, EngineReadError> {
    let attempt = match listed_containers(socket_path, selection).await {
        Err(http_json_client::Error::Status { status, .. })
            if status == http_json_client::TOO_MANY_REQUESTS_STATUS
                || status >= http_json_client::SERVER_ERROR_STATUS =>
        {
            listed_containers(socket_path, selection).await
        }
        Err(
            http_json_client::Error::HostNotResolved { .. }
            | http_json_client::Error::TimedOut { .. }
            | http_json_client::Error::Unreachable { .. },
        ) => listed_containers(socket_path, selection).await,
        first_attempt => first_attempt,
    };
    match attempt {
        Ok(listed) => Ok(listed),
        Err(failure) => Err(classified_engine_failure(socket_path, failure).await),
    }
}
