use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use chrono::DateTime;
use serde::Deserialize;

use crate::system_protocol_clients::http_json_client;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct AgentVarz {
    #[serde(default)]
    pub running: HashMap<String, RunningWorkflow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct RunningWorkflow {
    pub repository: String,
    pub pipeline_number: String,
    pipeline_started: String,
    pipeline_timeout: u64,
}

impl RunningWorkflow {
    pub fn started_at(&self) -> SystemTime {
        DateTime::parse_from_rfc3339(&self.pipeline_started)
            .map_or_else(|_| SystemTime::now(), SystemTime::from)
    }

    pub fn deadline(&self) -> SystemTime {
        self.started_at() + Duration::from_nanos(self.pipeline_timeout)
    }
}

pub(super) async fn fetch_agent_varz(varz_url: &str) -> Result<AgentVarz, http_json_client::Error> {
    http_json_client::get_json::<AgentVarz>(varz_url, &[]).await
}

#[cfg(test)]
pub(super) fn captured_varz() -> AgentVarz {
    serde_json::from_value(serde_json::json!({
        "polling_count": 0,
        "running_count": 2,
        "running": {
            "1054802": {
                "id": "1054802",
                "repository": "wvhulle/sottomano",
                "pipeline_number": "185",
                "pipeline_started": "2026-11-05T17:43:26.998262214Z",
                "pipeline_timeout": 3_600_000_000_000_u64,
            },
            "1054822": {
                "id": "1054822",
                "repository": "wvhulle/nu-lint",
                "pipeline_number": "201",
                "pipeline_started": "2026-11-05T17:53:52.649418563Z",
                "pipeline_timeout": 3_600_000_000_000_u64,
            },
        },
    }))
    .expect("captured varz decodes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_deadline_follows_the_start_by_the_timeout() {
        let varz = captured_varz();
        let workflow = &varz.running["1054802"];
        assert_eq!(
            workflow.deadline(),
            workflow.started_at() + Duration::from_secs(3600)
        );
    }
}
