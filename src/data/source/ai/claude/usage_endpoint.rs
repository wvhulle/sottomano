use std::path::Path;

use serde::de::DeserializeOwned;
use serde_json::Value;

use super::{ApiFailure, Error, backoff, plan, read_credentials};
use crate::data::source::http;

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const OAUTH_BETA: &str = "oauth-2025-04-20";
const CLAUDE_CODE_USER_AGENT: &str = "claude-code/2.1.233";

pub struct PlanUsage<Usage> {
    pub plan: String,
    pub usage: Usage,
}

pub async fn fetch<Usage: DeserializeOwned>(
    credentials_path: &Path,
) -> Result<PlanUsage<Usage>, Error> {
    let credentials = read_credentials(credentials_path)?;
    if let Some(waiting) = backoff::still_waiting().await {
        return Err(waiting);
    }
    let oauth = credentials.oauth.as_ref().ok_or(Error::NoToken)?;
    let token = oauth.access_token.as_ref().ok_or(Error::NoToken)?;

    let auth = format!("Bearer {token}");
    let mut resp = http::get(
        USAGE_URL,
        &[
            ("Authorization", &auth),
            ("anthropic-beta", OAUTH_BETA),
            ("User-Agent", CLAUDE_CODE_USER_AGENT),
        ],
        10,
    )
    .await?;
    let body = resp.body.take().unwrap_or(Value::Null);
    if resp.status == 429 {
        return Err(backoff::schedule_retry(&resp, body).await);
    }
    if resp.status != 200 {
        let failure = ApiFailure::from_body(body, resp.status);
        tracing::warn!("claude usage: HTTP {}: {failure}", resp.status);
        return Err(Error::Api(failure));
    }
    backoff::reset().await;

    Ok(PlanUsage {
        plan: plan(oauth),
        usage: serde_json::from_value(body)?,
    })
}
