use std::{future::ready, path::PathBuf, time::Duration};

use futures::{Stream, StreamExt, stream};
use reqwest::ClientBuilder;
use serde::Deserialize;

use super::DOCKER_API_ROOT;
use crate::system_protocol_clients::{
    http_json_client::{CONNECT_TIMEOUT, Error},
    protocol_peer_reconnects::subscribed,
};

pub(crate) const TRIGGER_BURST_GAP: Duration = Duration::from_secs(1);

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
) -> impl Stream<Item = Result<(), Error>> + Send {
    subscribed(socket_path, opened_event_records).map(|delivered| delivered.map(drop))
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
            let decoded: Vec<Result<DockerEventRecord, Error>> = match chunk {
                Ok(chunk) => {
                    buffer.push_str(&String::from_utf8_lossy(&chunk));
                    let complete_length = buffer.rfind('\n').map_or(0, |newline| newline + 1);
                    let complete_lines: String = buffer.drain(..complete_length).collect();
                    complete_lines
                        .lines()
                        .filter(|event_line| !event_line.trim().is_empty())
                        .map(|event_line| {
                            serde_json::from_str(event_line).map_err(|decode_failure| {
                                Error::Decode(format!(
                                    "docker event {event_line}: {decode_failure}"
                                ))
                            })
                        })
                        .collect()
                }
                Err(body_failure) => vec![Err(Error::Body(body_failure.to_string()))],
            };
            ready(Some(decoded))
        });
    Ok(event_records.flat_map(stream::iter))
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
