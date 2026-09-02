use std::{path::PathBuf, time::Duration};

use serde::Deserialize;
use serde_json::Value;

use super::{ApiFailure, Error, backoff, percent, plan, read_credentials, time_until_reset};
use crate::data::source::{
    BarWidget, Glyph, Level, Occasional, Row, Source, TooltipBody, Unit, Value as GaugeValue,
    ai::level_from_used, http, row, simple_icons::SimpleIcon,
};

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const OAUTH_BETA: &str = "oauth-2025-04-20";
const CLAUDE_CODE_USER_AGENT: &str = "claude-code/2.1.233";

#[derive(Deserialize)]
struct Usage {
    five_hour: Window,
    seven_day: Window,
    seven_day_sonnet: Option<Window>,
    #[serde(rename = "extra_usage")]
    overage: Option<Overage>,
}

#[derive(Deserialize)]
struct Window {
    utilization: f64,
    resets_at: Option<String>,
}

#[derive(Deserialize)]
struct Overage {
    #[serde(default)]
    is_enabled: bool,
    used_credits: Option<f64>,
    monthly_limit: Option<f64>,
    utilization: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeOauth {
    session_used: i64,
    tooltip: Vec<Row>,
}

impl BarWidget for ClaudeOauth {
    fn icon(&self) -> Glyph {
        SimpleIcon::Claude.into()
    }

    fn level(&self) -> Level {
        level_from_used(self.session_used)
    }

    fn value(&self) -> Option<GaugeValue> {
        Some(GaugeValue::Gauge {
            percent: self.session_used,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(self.tooltip.clone())
    }
}

impl Source for ClaudeOauth {
    type Error = Error;
    type Key = PathBuf;

    const NAME: &'static str = "claude-usage-oauth";
    const INTERVAL: Duration = Duration::from_mins(1);

    async fn poll(key: PathBuf) -> Result<Self, Self::Error> {
        let credentials = read_credentials(&key)?;
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

        let usage: Usage = serde_json::from_value(body)?;
        let session_used = percent(usage.five_hour.utilization);
        let mut tooltip = vec![
            row("Plan", plan(oauth)),
            row("Session", format!("{session_used}%")),
            row(
                "Session resets",
                time_until_reset(usage.five_hour.resets_at.as_ref()),
            ),
            row(
                "Weekly",
                format!("{}%", percent(usage.seven_day.utilization)),
            ),
            row(
                "Weekly resets",
                time_until_reset(usage.seven_day.resets_at.as_ref()),
            ),
        ];
        if let Some(sonnet) = &usage.seven_day_sonnet {
            tooltip.push(row("Sonnet", format!("{}%", percent(sonnet.utilization))));
            tooltip.push(row(
                "Sonnet resets",
                time_until_reset(sonnet.resets_at.as_ref()),
            ));
        }
        push_overage_rows(usage.overage.as_ref(), &mut tooltip);

        Ok(ClaudeOauth {
            session_used,
            tooltip,
        })
    }
}

impl Occasional for ClaudeOauth {
    fn present(&self) -> bool {
        self.session_used > 0
    }
}

fn push_overage_rows(extra: Option<&Overage>, tooltip: &mut Vec<Row>) {
    let Some(extra) = extra.filter(|extra| extra.is_enabled) else {
        return;
    };
    if let Some(spent) = extra.used_credits {
        tooltip.push(row("Extra spent", format!("€{:.2}", spent / 100.0)));
    }
    if let Some(limit) = extra.monthly_limit {
        tooltip.push(row("Extra limit", format!("€{:.0}", limit / 100.0)));
    }
    if let Some(utilization) = extra.utilization {
        tooltip.push(row("Extra", format!("{}%", percent(utilization))));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const USAGE: &str = r#"{
        "five_hour": {
            "utilization": 42.0, "resets_at": "2026-08-27T12:00:00Z",
            "limit_dollars": null, "used_dollars": null
        },
        "seven_day": { "utilization": 49.0, "resets_at": "2026-09-01T00:00:00Z" },
        "seven_day_sonnet": null,
        "extra_usage": {
            "is_enabled": false, "monthly_limit": null,
            "used_credits": null, "utilization": null
        },
        "limits": {}
    }"#;

    #[test]
    fn usage_windows_decode_with_null_sonnet_and_disabled_overage() {
        let usage: Usage = serde_json::from_str(USAGE).unwrap();
        assert_eq!(percent(usage.five_hour.utilization), 42);
        assert_eq!(percent(usage.seven_day.utilization), 49);
        assert!(usage.seven_day_sonnet.is_none());
        let mut tooltip = Vec::new();
        push_overage_rows(usage.overage.as_ref(), &mut tooltip);
        assert!(tooltip.is_empty());
    }
}
