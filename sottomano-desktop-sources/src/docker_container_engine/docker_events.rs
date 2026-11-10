use std::{future::ready, path::PathBuf, time::Duration};

use futures::{Stream, StreamExt, stream};
use reqwest::ClientBuilder;
use serde::Deserialize;

use super::DOCKER_API_ROOT;
use crate::{
    stream_operators::{StreamOperators, resubscribed},
    system_protocol_clients::http_json_client::{CONNECT_TIMEOUT, Error},
};

const TRIGGER_BURST_GAP: Duration = Duration::from_secs(1);
const RETRY_GAP: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize)]
struct DockerEventRecord {
    #[serde(rename = "Type", default)]
    #[allow(dead_code)]
    event_type: Option<String>,
    #[serde(rename = "Action", default)]
    #[allow(dead_code)]
    action: Option<String>,
}

pub(crate) fn docker_container_event_triggers(
    socket_path: PathBuf,
) -> impl Stream<Item = ()> + Send {
    resubscribed(RETRY_GAP, move || {
        let trigger_socket = socket_path.clone();
        async move { opened_event_records(trigger_socket).await }
    })
    .map(|delivered| {
        delivered.map_or_else(
            |failure| tracing::warn!("docker events: {failure}, resubscribing"),
            |_| (),
        );
    })
    .at_most_once_per(TRIGGER_BURST_GAP)
}

async fn opened_event_records(
    socket_path: PathBuf,
) -> Result<impl Stream<Item = Result<DockerEventRecord, Error>> + Send, Error> {
    let socket_label = socket_path.display().to_string();
    let client = ClientBuilder::new()
        .connect_timeout(CONNECT_TIMEOUT)
        .unix_socket(socket_path)
        .build()
        .map_err(|failure| Error::Client(failure.to_string()))?;
    let response = client
        .get(format!("{DOCKER_API_ROOT}/events"))
        .send()
        .await
        .map_err(|failure| Error::Unreachable {
            host: socket_label,
            detail: failure.to_string(),
        })?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(Error::status(status, None));
    }
    let event_records = response
        .bytes_stream()
        .scan(String::new(), |buffer, chunk| {
            let Ok(chunk) = chunk else {
                return ready(None);
            };
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            let decoded: Vec<DockerEventRecord> = buffer
                .lines()
                .filter(|event_line| !event_line.trim().is_empty())
                .filter_map(|event_line| serde_json::from_str(event_line).ok())
                .collect();
            *buffer = buffer
                .lines()
                .last()
                .filter(|_| !buffer.ends_with('\n'))
                .map(str::to_owned)
                .unwrap_or_default();
            ready(Some(decoded))
        });
    Ok(event_records
        .flat_map(stream::iter)
        .map(Ok::<DockerEventRecord, Error>))
}

#[cfg(test)]
mod tests {
    use super::DockerEventRecord;

    #[test]
    fn an_event_line_decodes_with_optional_fields() {
        let record: DockerEventRecord =
            serde_json::from_str(r#"{"Type":"container","Action":"start","Actor":{"ID":"abc"}}"#)
                .expect("event record decodes");
        assert_eq!(record.event_type.as_deref(), Some("container"));
    }
}
