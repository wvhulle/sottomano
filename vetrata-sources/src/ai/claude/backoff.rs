use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use super::Error;
use crate::{
    http,
    store::rate_limit::{self, Backoff},
};

const SOURCE: &str = "claude-usage-oauth";
const MIN_DELAY: Duration = Duration::from_mins(1);
const BASE_DELAY: Duration = Duration::from_mins(5);
const MAX_DELAY: Duration = Duration::from_mins(60);

pub async fn still_waiting() -> Option<Error> {
    let stored = rate_limit::load(SOURCE).await?;
    let Some(remaining) = stored.remaining() else {
        tracing::info!("claude usage: backoff expired, resuming polling");
        rate_limit::clear(SOURCE).await;
        return None;
    };
    tracing::debug!(
        "claude usage: backing off for another {}",
        humantime::format_duration(remaining)
    );
    Some(Error::Backoff {
        remaining,
        cause: http::Error::Status {
            status: stored.status,
            message: stored.message,
        },
    })
}

pub async fn schedule_retry(resp: &http::Response, body: Value) -> Error {
    let message = http::failure_message(Some(body));
    let attempts = rate_limit::load(SOURCE)
        .await
        .map_or(0, |stored| stored.attempts);
    let server_delay = retry_delay_from_headers(resp);
    let delay = server_delay
        .unwrap_or_else(|| with_jitter(escalating_delay(attempts)))
        .clamp(MIN_DELAY, MAX_DELAY);
    tracing::warn!(
        "claude usage: HTTP 429 ({message:?}); retry-after {:?}, attempt {}, waiting {}",
        server_delay,
        attempts + 1,
        humantime::format_duration(delay)
    );
    rate_limit::save(
        SOURCE,
        &Backoff {
            retry_after_unix: rate_limit::now_unix()
                + i64::try_from(delay.as_secs()).unwrap_or(i64::MAX),
            attempts: attempts.saturating_add(1),
            status: 429,
            message: message.clone(),
        },
    )
    .await;
    Error::RateLimited(http::Error::Status {
        status: 429,
        message,
    })
}

pub async fn reset() {
    rate_limit::clear(SOURCE).await;
}

fn retry_delay_from_headers(resp: &http::Response) -> Option<Duration> {
    let seconds = seconds_from_header(resp.header("retry-after")).or_else(|| {
        seconds_from_header(resp.header("anthropic-ratelimit-unified-reset"))
            .map(|reset| reset - rate_limit::now_unix())
    })?;
    u64::try_from(seconds).ok().map(Duration::from_secs)
}

fn seconds_from_header(header: Option<&str>) -> Option<i64> {
    header?.trim().parse().ok()
}

fn escalating_delay(attempts: u32) -> Duration {
    (BASE_DELAY * 2u32.pow(attempts.min(4))).min(MAX_DELAY)
}

fn with_jitter(delay: Duration) -> Duration {
    let span = delay.as_secs() / 10;
    if span == 0 {
        return delay;
    }
    let offset = u64::from(nanos_of_second()) % (span * 2 + 1);
    Duration::from_secs(delay.as_secs() + offset - span)
}

fn nanos_of_second() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.subsec_nanos())
}

#[cfg(test)]
mod tests {
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

    use super::*;

    fn response(headers: &[(&str, &str)]) -> http::Response {
        let mut map = HeaderMap::new();
        for (name, value) in headers {
            map.insert(
                HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        http::Response {
            status: 429,
            headers: map,
            body: None,
        }
    }

    #[test]
    fn retry_after_header_wins_over_the_reset_header() {
        let resp = response(&[
            ("retry-after", "90"),
            (
                "anthropic-ratelimit-unified-reset",
                &(rate_limit::now_unix() + 3600).to_string(),
            ),
        ]);
        assert_eq!(
            retry_delay_from_headers(&resp),
            Some(Duration::from_secs(90))
        );
    }

    #[test]
    fn reset_header_is_converted_to_a_relative_delay() {
        let resp = response(&[(
            "anthropic-ratelimit-unified-reset",
            &(rate_limit::now_unix() + 120).to_string(),
        )]);
        let delay = retry_delay_from_headers(&resp).unwrap();
        assert!(delay <= Duration::from_secs(120) && delay >= Duration::from_secs(118));
    }

    #[test]
    fn missing_and_unparsable_headers_yield_no_server_delay() {
        assert_eq!(retry_delay_from_headers(&response(&[])), None);
        assert_eq!(
            retry_delay_from_headers(&response(&[(
                "retry-after",
                "Wed, 21 Oct 2026 07:28:00 GMT"
            )])),
            None
        );
    }
}
