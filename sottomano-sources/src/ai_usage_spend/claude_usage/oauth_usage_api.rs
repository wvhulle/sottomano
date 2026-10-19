use std::path::Path;

use serde::de::DeserializeOwned;
use serde_json::Value;

use super::{Error, oauth_retry_backoff, plan, read_credentials};
use crate::system_protocol_clients::http_json_client;

pub const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
pub const PROFILE_URL: &str = "https://api.anthropic.com/api/oauth/profile";
const OAUTH_BETA: &str = "oauth-2025-04-20";
const CLAUDE_CODE_USER_AGENT: &str = "claude-code/2.1.233";

pub struct PlanReply<Reply> {
    pub plan: Option<String>,
    pub reply: Reply,
}

pub async fn fetch<Reply: DeserializeOwned>(
    credentials_path: &Path,
    url: &str,
) -> Result<PlanReply<Reply>, Error> {
    let credentials = read_credentials(credentials_path)?;
    if let Some(waiting) = oauth_retry_backoff::still_waiting().await {
        return Err(waiting);
    }
    let oauth = credentials.oauth.as_ref().ok_or(Error::NoToken)?;
    let token = oauth.access_token.as_ref().ok_or(Error::NoToken)?;

    let auth = format!("Bearer {token}");
    let mut resp = http_json_client::get(
        url,
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
        return Err(oauth_retry_backoff::schedule_retry(&resp, body).await);
    }
    if resp.status != 200 {
        let failure = http_json_client::Error::status(resp.status, Some(body));
        tracing::warn!("claude usage: {failure}");
        return Err(failure.into());
    }
    oauth_retry_backoff::reset().await;

    Ok(PlanReply {
        plan: plan(oauth),
        reply: http_json_client::decode(Some(body))?,
    })
}
